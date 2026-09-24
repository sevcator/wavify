use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DISCORD_CLIENT_ID: &str = "1550042978599440435";
const SOUNDCLOUD_FALLBACK_COVER: &str =
    "https://a-v2.sndcdn.com/assets/images/sc-custom-image-ambience-9669528f.jpg";

static RPC_CONNECTED: AtomicBool = AtomicBool::new(false);

pub fn is_connected() -> bool {
    RPC_CONNECTED.load(Ordering::Relaxed)
}

fn rpc_log_file() -> PathBuf {
    crate::config::config_dir().join("discord_rpc.log")
}

pub fn log_rpc(msg: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{ts}] {msg}\n");
    crate::log!("discord-rpc: {msg}");

    // kept small: once a run, a log past 512 KB is set aside (the one
    // before stays as .old) and a new one starts
    static ROTATED: std::sync::Once = std::sync::Once::new();
    ROTATED.call_once(|| {
        let path = rpc_log_file();
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 512 * 1024) {
            let _ = std::fs::rename(&path, path.with_extension("log.old"));
        }
    });

    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(rpc_log_file())
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityData {
    pub title: String,
    pub artist: String,
    pub artwork_url: Option<String>,
    pub permalink_url: Option<String>,
    pub pos_ms: u64,
    pub dur_ms: u64,
}

#[derive(Debug)]
pub enum RpcCommand {
    /// Activity plus the playback speed it plays at.
    Update(ActivityData, f32),
    Clear,
    Shutdown,
}

/// The latest activity from the UI and when it arrived: `pos_ms` is only
/// true at that moment, so a later send (the rate-limit wait, a reconnect)
/// advances it by the time since.
struct CurrentActivity {
    data: ActivityData,
    speed: f32,
    at: Instant,
}

pub struct DiscordRpcHandle {
    tx: Sender<RpcCommand>,
}

impl DiscordRpcHandle {
    pub fn start() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || {
                run_rpc_worker(rx);
            })
            .expect("failed to spawn discord-rpc thread");

        Self { tx }
    }

    pub fn update(&self, data: ActivityData) {
        self.update_with_speed(data, 1.0);
    }

    /// Like `update`, for playback at `speed`x: Discord's bar runs at wall
    /// clock, so the timestamps are scaled to match.
    pub fn update_with_speed(&self, data: ActivityData, speed: f32) {
        let _ = self.tx.send(RpcCommand::Update(data, speed));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(RpcCommand::Clear);
    }
}

impl Drop for DiscordRpcHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(RpcCommand::Shutdown);
    }
}

fn get_pipe_paths() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        (0..10)
            .map(|i| PathBuf::from(format!(r"\\.\pipe\discord-ipc-{}", i)))
            .collect()
    }
    #[cfg(not(windows))]
    {
        let mut paths = Vec::new();
        let dirs = [
            std::env::var("XDG_RUNTIME_DIR").ok().map(PathBuf::from),
            std::env::var("TMPDIR").ok().map(PathBuf::from),
            std::env::var("TMP").ok().map(PathBuf::from),
            std::env::var("TEMP").ok().map(PathBuf::from),
            Some(PathBuf::from("/tmp")),
        ];
        for dir in dirs.into_iter().flatten() {
            for i in 0..10 {
                paths.push(dir.join(format!("discord-ipc-{}", i)));
            }
        }
        paths
    }
}

struct IpcConnection {
    pipe: std::fs::File,
}

impl IpcConnection {
    fn connect() -> Option<Self> {
        for path in get_pipe_paths() {
            if let Ok(mut pipe) = OpenOptions::new().read(true).write(true).open(&path) {
                // Handshake Opcode 0
                let handshake = serde_json::json!({
                    "v": 1,
                    "client_id": DISCORD_CLIENT_ID
                })
                .to_string();

                if Self::send_packet(&mut pipe, 0, &handshake).is_err() {
                    continue;
                }

                // Read Handshake response (opcode 1, ready)
                match Self::read_packet(&mut pipe) {
                    Ok((op, payload)) => {
                        log_rpc(&format!(
                            "connected to {:?} (op={}, len={})",
                            path,
                            op,
                            payload.len()
                        ));
                        RPC_CONNECTED.store(true, Ordering::Relaxed);
                        return Some(Self { pipe });
                    }
                    Err(e) => {
                        log_rpc(&format!("handshake read error on {:?}: {e}", path));
                        continue;
                    }
                }
            }
        }
        None
    }

    fn send_packet(
        pipe: &mut std::fs::File,
        opcode: u32,
        json_payload: &str,
    ) -> std::io::Result<()> {
        let payload_bytes = json_payload.as_bytes();
        let mut packet = Vec::with_capacity(8 + payload_bytes.len());
        packet.extend_from_slice(&opcode.to_le_bytes());
        packet.extend_from_slice(&(payload_bytes.len() as u32).to_le_bytes());
        packet.extend_from_slice(payload_bytes);

        pipe.write_all(&packet)?;
        pipe.flush()?;
        Ok(())
    }

    fn read_packet(pipe: &mut std::fs::File) -> std::io::Result<(u32, String)> {
        let mut hdr = [0u8; 8];
        pipe.read_exact(&mut hdr)?;
        let opcode = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;

        // Discord payload sanity check (max 64KB)
        if len > 65536 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "packet too large",
            ));
        }

        let mut buf = vec![0u8; len];
        pipe.read_exact(&mut buf)?;
        let payload = String::from_utf8_lossy(&buf).to_string();
        Ok((opcode, payload))
    }

    fn set_activity(&mut self, activity: Option<serde_json::Value>) -> std::io::Result<()> {
        let payload = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity
            },
            "nonce": format!("{}", std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis())
        }).to_string();

        Self::send_packet(&mut self.pipe, 1, &payload)?;
        match Self::read_packet(&mut self.pipe) {
            Ok((_op, resp)) => {
                if resp.contains("\"evt\":\"ERROR\"") || resp.contains("\"code\":") {
                    log_rpc(&format!("SET_ACTIVITY error response: {resp}"));
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Discord returned error",
                    ))
                } else {
                    log_rpc("SET_ACTIVITY acknowledged by Discord");
                    Ok(())
                }
            }
            Err(e) => {
                log_rpc(&format!("read error after SET_ACTIVITY: {e}"));
                Err(e)
            }
        }
    }

    fn close(&mut self) {
        let _ = Self::send_packet(&mut self.pipe, 2, "{}");
        RPC_CONNECTED.store(false, Ordering::Relaxed);
    }

    /// Is Discord still at the other end? Looked at without waiting: a
    /// restarted or closed Discord leaves a broken pipe, which is only
    /// noticed otherwise at the next update (never, during a long track).
    /// Anything Discord sent meanwhile is read: a CLOSE (opcode 2) ends
    /// the connection too.
    fn alive(&mut self) -> bool {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Pipes::PeekNamedPipe;
            let mut avail: u32 = 0;
            let ok = unsafe {
                PeekNamedPipe(
                    self.pipe.as_raw_handle() as _,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut avail,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return false;
            }
            if avail >= 8 {
                match Self::read_packet(&mut self.pipe) {
                    Ok((2, payload)) => {
                        log_rpc(&format!("Discord closed the connection: {payload}"));
                        return false;
                    }
                    Ok(_) => {}
                    Err(_) => return false,
                }
            }
            true
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    &s[..end]
}

fn sanitize_string(s: &str, fallback: &str, max_len: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() < 2 {
        fallback.to_string()
    } else {
        truncate_str(trimmed, max_len).to_string()
    }
}

/// Matches BetterSoundCloud activity format:
/// - type: 2 (Listening) -> Discord displays "Listening to SoundCloud"
/// - details: track title
/// - state: artist name (or "⏸ Paused • <artist>" when paused)
/// - timestamps: start and end in milliseconds
/// - assets: large_image, and large_text only for a speed other than 1x
///   ("x1.25"; no small_image)
/// - NO buttons (omitted to prevent Discord hiding activity on user's own profile)
fn format_activity(cur: &CurrentActivity) -> serde_json::Value {
    let data = &cur.data;
    let title = sanitize_string(&data.title, "SoundCloud Track", 128);
    let artist = sanitize_string(&data.artist, "SoundCloud Artist", 100);

    // Only playing tracks are shown: the UI clears the presence on pause.
    let state = sanitize_string(&artist, "SoundCloud", 128);

    let mut activity = serde_json::json!({
        "type": 2, // ActivityType.Listening
        "details": title,
        "state": state,
        "instance": false,
    });

    // Timestamps in milliseconds. Discord runs the bar at
    // wall-clock speed, so at `speed`x the track's timeline is scaled by
    // 1/speed; the position also advances by the time since the UI sent it.
    if data.dur_ms > 0 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let speed = if cur.speed.is_finite() && cur.speed > 0.0 {
            cur.speed as f64
        } else {
            1.0
        };
        let dur_ms = (data.dur_ms as f64 / speed) as u64;
        let pos_ms =
            ((data.pos_ms as f64 / speed) as u64 + cur.at.elapsed().as_millis() as u64).min(dur_ms);
        let start_ms = now_ms.saturating_sub(pos_ms);
        let end_ms = start_ms + dur_ms;

        activity["timestamps"] = serde_json::json!({
            "start": start_ms,
            "end": end_ms,
        });
    }

    // Cover art: only large_image (no large_text per user instruction)
    let cover_url = match &data.artwork_url {
        Some(url) if !url.trim().is_empty() => url.as_str(),
        _ => SOUNDCLOUD_FALLBACK_COVER,
    };

    // the cover, and (hovering it) a playback speed that isn't 1x: "x1.25"
    let mut assets = serde_json::json!({
        "large_image": cover_url,
    });
    if cur.speed.is_finite() && (cur.speed - 1.0).abs() > 0.005 {
        let speed = format!("{:.2}", cur.speed);
        let speed = speed.trim_end_matches('0').trim_end_matches('.');
        assets["large_text"] = serde_json::json!(format!("x{speed}"));
    }
    activity["assets"] = assets;

    activity
}

/// Discord takes 5 activity updates per 20 s and silently drops the rest:
/// updates are spent from a bucket of 5 that refills one every 4 s. Changes
/// in between are merged, and the latest goes out as soon as it may.
const RATE_BURST: f64 = 5.0;
const RATE_REFILL: Duration = Duration::from_secs(4);

fn run_rpc_worker(rx: Receiver<RpcCommand>) {
    log_rpc("worker started");
    let mut conn: Option<IpcConnection> = None;
    let mut current_activity: Option<CurrentActivity> = None;
    // the activity (or its clearing) Discord hasn't been told yet
    let mut dirty = false;
    let mut tokens = RATE_BURST;
    let mut refilled = Instant::now();
    let mut last_connect_attempt = Instant::now() - Duration::from_secs(10);
    let mut last_health_check = Instant::now();

    let drop_conn = |conn: &mut Option<IpcConnection>, why: &str| {
        log_rpc(why);
        *conn = None;
        RPC_CONNECTED.store(false, Ordering::Relaxed);
    };

    loop {
        // (re)connect: Discord started, restarted, or the pipe broke; what
        // was showing is sent again
        if conn.is_none() && last_connect_attempt.elapsed() >= Duration::from_secs(3) {
            last_connect_attempt = Instant::now();
            conn = IpcConnection::connect();
            if conn.is_some() {
                dirty = current_activity.is_some();
                last_health_check = Instant::now();
            } else {
                RPC_CONNECTED.store(false, Ordering::Relaxed);
            }
        }

        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(RpcCommand::Update(data, speed)) => {
                let changed = current_activity
                    .as_ref()
                    .map_or(true, |cur| cur.data != data || cur.speed != speed);
                if changed {
                    current_activity = Some(CurrentActivity {
                        data,
                        speed,
                        at: Instant::now(),
                    });
                    dirty = true;
                }
            }
            Ok(RpcCommand::Clear) => {
                if current_activity.take().is_some() {
                    dirty = true;
                }
            }
            Ok(RpcCommand::Shutdown) => {
                log_rpc("worker received shutdown command");
                if let Some(mut c) = conn {
                    let _ = c.set_activity(None);
                    c.close();
                }
                RPC_CONNECTED.store(false, Ordering::Relaxed);
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log_rpc("worker channel disconnected");
                if let Some(mut c) = conn {
                    let _ = c.set_activity(None);
                    c.close();
                }
                RPC_CONNECTED.store(false, Ordering::Relaxed);
                break;
            }
        }

        // refill the rate-limit bucket
        let since = refilled.elapsed();
        if since >= RATE_REFILL {
            let steps = since.as_secs_f64() / RATE_REFILL.as_secs_f64();
            tokens = (tokens + steps.floor()).min(RATE_BURST);
            refilled = Instant::now();
        }

        let Some(c) = conn.as_mut() else { continue };

        // Discord gone (restarted, closed)? Checked every second without
        // blocking; the reconnect above picks it up again.
        if last_health_check.elapsed() >= Duration::from_secs(1) {
            last_health_check = Instant::now();
            if !c.alive() {
                drop_conn(&mut conn, "Discord connection lost, reconnecting");
                dirty = current_activity.is_some();
                continue;
            }
        }

        // Nothing is resent while nothing changed: Discord runs the bar
        // itself, and a resend would restart it.
        if dirty && tokens >= 1.0 {
            // the position moves on by the time it waited (see format_activity)
            let act = current_activity.as_ref().map(format_activity);
            match c.set_activity(act) {
                Ok(()) => {
                    tokens -= 1.0;
                    dirty = false;
                }
                Err(e) => {
                    drop_conn(&mut conn, &format!("update error: {e}; reconnecting"));
                }
            }
        }
    }
    log_rpc("worker exited");
}
