//! Wavify's side of the YouTube Music player (see yt_web.rs): starts the
//! hidden player process on demand, sends it commands, and hands its events
//! to the UI. While a track plays there, the audio player's pause, seek,
//! volume and speed commands are forwarded to it (see PlayerHandle::send).

use crate::player::PlayerCommand;
use crate::yt_web::PageEvent;
use iced::futures::channel::mpsc;
use std::io::{BufRead, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// The playing track comes from YouTube Music.
static ACTIVE: AtomicBool = AtomicBool::new(false);

struct Proc {
    child: Child,
    stdin: ChildStdin,
}

static EVENTS: Mutex<Option<mpsc::UnboundedSender<PageEvent>>> = Mutex::new(None);
/// The song last started (video id, volume, speed), for a restart.
static LAST: Mutex<Option<(String, f32, f32)>> = Mutex::new(None);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

/// Signed in to YouTube Music in Wavify's YouTube profile.
pub fn signed_in() -> bool {
    crate::yt_web::signed_in_marker().exists()
}

/// Player events, for as long as the subscription runs.
pub fn subscription() -> iced::Subscription<PageEvent> {
    iced::Subscription::run(listen)
}

fn listen() -> mpsc::UnboundedReceiver<PageEvent> {
    let (tx, rx) = mpsc::unbounded();
    if let Ok(mut e) = EVENTS.lock() {
        *e = Some(tx);
    }
    rx
}

/// What the player thread does next.
enum Job {
    /// A command for the page.
    Line(serde_json::Value),
    /// End the process; `done` hears once it's gone.
    Stop(Option<std::sync::mpsc::Sender<()>>),
}

/// The one thread that owns the player process: it starts it when a
/// command needs it (that can wait seconds for the browser profile, see
/// wait_for_profile), writes the commands in order, and stops it. The UI
/// thread only queues jobs, so it never waits on any of that.
fn worker() -> &'static std::sync::mpsc::Sender<Job> {
    static TX: std::sync::OnceLock<std::sync::mpsc::Sender<Job>> = std::sync::OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("yt-player".into())
            .spawn(move || {
                let mut proc: Option<Proc> = None;
                let mut last_job = std::time::Instant::now();
                loop {
                    let job = match rx.recv_timeout(std::time::Duration::from_secs(60)) {
                        Ok(job) => job,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            // Not played on YouTube for 5 minutes: its player
                            // (a WebView2, a couple of hundred MB) goes, and
                            // the next YouTube track starts it again.
                            let idle = last_job.elapsed() >= std::time::Duration::from_secs(300);
                            if idle && !is_active() {
                                if let Some(mut p) = proc.take() {
                                    let _ = p.child.kill();
                                    let _ = p.child.wait();
                                    crate::log!("youtube: player idle, shut down");
                                }
                            }
                            continue;
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    last_job = std::time::Instant::now();
                    match job {
                        Job::Line(cmd) => write_line(&mut proc, &cmd),
                        Job::Stop(done) => {
                            if let Some(mut p) = proc.take() {
                                let _ = p.child.kill();
                                let _ = p.child.wait();
                            }
                            if let Some(done) = done {
                                let _ = done.send(());
                            }
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            crate::log!("youtube: player thread failed to start: {e}");
        }
        tx
    })
}

/// Send one command (queued for the player thread).
fn send_line(cmd: serde_json::Value) {
    let _ = worker().send(Job::Line(cmd));
}

/// Write one command, starting the player process if it isn't running.
fn write_line(proc: &mut Option<Proc>, cmd: &serde_json::Value) {
    let alive = proc
        .as_mut()
        .is_some_and(|p| matches!(p.child.try_wait(), Ok(None)));
    if !alive {
        *proc = spawn();
    }
    let ok = proc.as_mut().is_some_and(|p| {
        writeln!(p.stdin, "{cmd}")
            .and_then(|_| p.stdin.flush())
            .is_ok()
    });
    if !ok {
        crate::log!("youtube: player process unavailable");
        *proc = None;
    }
}

/// WebView2's browser outlives the process that ran it by a few seconds,
/// and a new process on the same profile would share it as it shuts down:
/// wait for the profile's lock to go first (up to 5 s).
fn wait_for_profile() {
    let lock = crate::yt_web::profile_dir()
        .join("EBWebView")
        .join("lockfile");
    for _ in 0..25 {
        match std::fs::remove_file(&lock) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                std::thread::sleep(std::time::Duration::from_millis(200))
            }
            _ => return,
        }
    }
    crate::log!("youtube: browser profile still in use");
}

fn spawn() -> Option<Proc> {
    let exe = std::env::current_exe().ok()?;
    wait_for_profile();
    let mut child = Command::new(exe)
        .arg("--yt-player")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(ev) = serde_json::from_str::<PageEvent>(&line) else {
                continue;
            };
            if let Ok(e) = EVENTS.lock() {
                if let Some(tx) = e.as_ref() {
                    let _ = tx.unbounded_send(ev);
                }
            }
        }
    });
    crate::log!("youtube: player process started");
    Some(Proc { child, stdin })
}

/// Play a YouTube Music song at the app's volume (0..1, as the audio player
/// takes it) and speed.
pub fn play(video_id: &str, volume: f32, speed: f32) {
    ACTIVE.store(true, Ordering::SeqCst);
    if let Ok(mut last) = LAST.lock() {
        *last = Some((video_id.to_string(), volume, speed));
    }
    send_line(serde_json::json!({"op": "load", "vid": video_id}));
    send_line(serde_json::json!({"op": "volume", "v": perceptual(volume)}));
    send_line(serde_json::json!({"op": "speed", "v": speed}));
}

/// The same curve as the audio player, so the level doesn't jump between
/// sources.
fn perceptual(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v
}

/// Mirror an audio-player command while YouTube Music is the source; a new
/// track or a stop sends the page to idle.
pub fn forward(cmd: &PlayerCommand) {
    match cmd {
        PlayerCommand::Stop
        | PlayerCommand::FadeOut(_)
        | PlayerCommand::PlaySingle { .. }
        | PlayerCommand::PlayChunks { .. }
        | PlayerCommand::PlayCached { .. } => {
            if ACTIVE.swap(false, Ordering::SeqCst) {
                send_line(serde_json::json!({"op": "stop"}));
            }
        }
        _ if !is_active() => {}
        PlayerCommand::Pause => send_line(serde_json::json!({"op": "pause"})),
        PlayerCommand::Resume => send_line(serde_json::json!({"op": "play"})),
        PlayerCommand::SeekMs(ms) => {
            send_line(serde_json::json!({"op": "seek", "sec": *ms as f64 / 1000.0}))
        }
        PlayerCommand::SetVolume(v) => {
            if let Ok(Some(last)) = LAST.lock().as_deref_mut() {
                last.1 = *v;
            }
            send_line(serde_json::json!({"op": "volume", "v": perceptual(*v)}))
        }
        PlayerCommand::SetSpeed(s) => {
            if let Ok(Some(last)) = LAST.lock().as_deref_mut() {
                last.2 = *s;
            }
            send_line(serde_json::json!({"op": "speed", "v": s}))
        }
    }
}

/// A player that went silent: a new process, and the song again (the
/// player thread does it in order; the new process waits for the old
/// browser to go).
pub fn restart() {
    let last = LAST.lock().ok().and_then(|l| l.clone());
    ACTIVE.store(false, Ordering::SeqCst);
    let _ = worker().send(Job::Stop(None));
    if let Some((video_id, volume, speed)) = last {
        play(&video_id, volume, speed);
    }
}

/// Stop the player process and wait until it's gone: sign-in and sign-out
/// need the browser profile it holds. Blocks, so never on the UI thread.
fn stop_process_and_wait() {
    ACTIVE.store(false, Ordering::SeqCst);
    let (done, gone) = std::sync::mpsc::channel();
    if worker().send(Job::Stop(Some(done))).is_ok() {
        let _ = gone.recv_timeout(std::time::Duration::from_secs(10));
    }
}

/// Open the Google sign-in window and wait for it. The player process shares
/// the browser profile, so it's stopped first.
pub async fn sign_in() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = tokio::task::spawn_blocking(move || {
        stop_process_and_wait();
        wait_for_profile();
        Command::new(exe).arg("--yt-login").status()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if status.success() && signed_in() {
        Ok(())
    } else {
        Err("sign-in window closed before signing in".into())
    }
}

/// Forget the Google session: the whole YouTube browser profile goes.
pub fn sign_out() {
    stop_process_and_wait();
    let _ = std::fs::remove_file(crate::yt_web::signed_in_marker());
    // the browser process may hold the folder for a moment after exit
    for _ in 0..10 {
        if std::fs::remove_dir_all(crate::yt_web::profile_dir()).is_ok()
            || !crate::yt_web::profile_dir().exists()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
