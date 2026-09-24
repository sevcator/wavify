//! YouTube Music's own web player, in a WebView2 of its own (a child
//! process, like the SoundCloud login): Wavify doesn't fetch YouTube audio
//! itself. The page runs as it would in a browser, signed in with the
//! user's Google account, and Wavify drives it: load a song, play, pause,
//! seek, volume, speed; the page reports position and the song's end.
//!
//!   wavify.exe --yt-login   Google sign-in window (the session is kept in
//!                           Wavify's YouTube profile)
//!   wavify.exe --yt-player  hidden player: JSON commands on stdin, JSON
//!                           events on stdout

use crate::config::config_dir;
use std::io::{BufRead, Write};

/// Wavify's browser profile for YouTube (cookies of the Google sign-in).
pub fn profile_dir() -> std::path::PathBuf {
    config_dir().join("youtube_web")
}

/// Written when the sign-in window reaches YouTube Music signed in.
pub fn signed_in_marker() -> std::path::PathBuf {
    config_dir().join("youtube_signed_in")
}

/// The same for both processes: they share the profile, which WebView2
/// only allows with identical options. Autoplay without a click, because
/// every song is started from Wavify, not on the page.
const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required";

const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?service=youtube&passive=true\
    &continue=https%3A%2F%2Fmusic.youtube.com%2F";

/// Runs on every page. Reports whether the session is signed in (the
/// SAPISID cookie YouTube's own scripts read) and, on a player page, the
/// playback state four times a second; takes Wavify's commands. Ads play
/// muted, and YouTube's own Skip button is pressed once the page shows it.
const PAGE_SCRIPT: &str = r#"
(() => {
  if (window.__wavify) return;
  const post = (o) => { try { window.ipc.postMessage(JSON.stringify(o)); } catch (e) {} };
  const W = window.__wavify = { ended: false, volume: null, speed: null, adMuted: false };
  // Wavify's commands count as user activity, which lets the page ask
  // "Leave site?" on the next song's load, in a window nobody sees: the
  // player would hang. Registered before the page's own handlers.
  window.addEventListener('beforeunload', (e) => e.stopImmediatePropagation(), true);
  const player = () => document.getElementById('movie_player');
  const video = () => document.querySelector('video');
  const signedIn = () => /(^|;\s*)(__Secure-3PAPISID|SAPISID)=/.test(document.cookie);
  const isAd = () => { const p = player(); return !!(p && p.classList && p.classList.contains('ad-showing')); };
  const skipButton = () => {
    for (const b of document.querySelectorAll('.ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-ad-skip-button-modern')) {
      if (b.offsetParent !== null) return b;
    }
    return null;
  };
  const applyPrefs = () => {
    const p = player(), v = video();
    if (isAd()) {
      W.adMuted = true;
      if (p && p.mute) p.mute();
      if (v) v.muted = true;
      return;
    }
    if (W.adMuted) {
      W.adMuted = false;
      if (p && p.unMute) p.unMute();
      if (v) v.muted = false;
    }
    if (W.volume !== null) {
      if (p && p.setVolume) { if (p.isMuted && p.isMuted()) p.unMute(); p.setVolume(Math.round(W.volume * 100)); }
      else if (v) { v.muted = false; v.volume = W.volume; }
    }
    if (W.speed !== null) {
      if (p && p.setPlaybackRate) p.setPlaybackRate(W.speed); else if (v) v.playbackRate = W.speed;
    }
  };
  W.cmd = (c) => {
    const p = player(), v = video();
    switch (c.op) {
      case 'play': W.ended = false; if (p && p.playVideo) p.playVideo(); else if (v) v.play(); break;
      case 'pause': if (p && p.pauseVideo) p.pauseVideo(); else if (v) v.pause(); break;
      case 'seek': W.ended = false; if (p && p.seekTo) p.seekTo(c.sec, true); else if (v) v.currentTime = c.sec; break;
      case 'volume': W.volume = c.v; applyPrefs(); break;
      case 'speed': W.speed = c.v; applyPrefs(); break;
    }
  };
  // The song's end: reported once, and YouTube Music's autoplay of the next
  // song is held (Wavify's queue decides what comes next).
  document.addEventListener('ended', (e) => {
    if (e.target.tagName !== 'VIDEO' || isAd()) return;
    W.ended = true;
    post({ ev: 'ended', vid: new URL(location.href).searchParams.get('v') });
  }, true);
  document.addEventListener('play', (e) => {
    if (e.target.tagName !== 'VIDEO') return;
    if (isAd()) applyPrefs();
    else if (W.ended) e.target.pause();
  }, true);
  let lastPrefs = 0;
  setInterval(() => {
    const v = video();
    if (location.hostname === 'music.youtube.com') post({ ev: 'account', signed_in: signedIn() });
    if (!v || W.ended) return;
    const ad = isAd();
    // an ad starting or ending changes the sound at once
    if (ad !== W.adMuted || Date.now() - lastPrefs > 2000) { lastPrefs = Date.now(); applyPrefs(); }
    if (ad) { const b = skipButton(); if (b) b.click(); }
    post({
      ev: 'status',
      vid: new URL(location.href).searchParams.get('v'),
      pos: v.currentTime || 0,
      dur: isFinite(v.duration) ? v.duration : 0,
      playing: !v.paused,
      ad,
    });
  }, 250);
})();
"#;

/// Events of the hidden player, one JSON object per stdout line.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum PageEvent {
    Status {
        vid: Option<String>,
        pos: f64,
        dur: f64,
        playing: bool,
        ad: bool,
    },
    Ended {
        vid: Option<String>,
    },
    Account {
        signed_in: bool,
    },
}

enum Cmd {
    Load(String),
    Js(String),
    Stop,
}

/// `wavify.exe --yt-login`: the Google sign-in window. Exits 0 once YouTube
/// Music is signed in, 1 if the window is closed before.
pub fn run_login() -> ! {
    use wry::application::event::{Event, WindowEvent};
    use wry::application::event_loop::{ControlFlow, EventLoop};
    use wry::application::platform::windows::EventLoopExtWindows;
    use wry::application::window::WindowBuilder;
    use wry::webview::{WebContext, WebViewBuilder, WebViewBuilderExtWindows};

    let event_loop: EventLoop<()> = EventLoop::new_any_thread();
    let window = WindowBuilder::new()
        .with_title("Sign in to YouTube Music — Wavify")
        .with_window_icon(crate::ui::webview_window_icon())
        .with_inner_size(wry::application::dpi::LogicalSize::new(480.0, 720.0))
        .build(&event_loop)
        .expect("window");
    let mut context = WebContext::new(Some(profile_dir()));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done2 = done.clone();
    let webview = WebViewBuilder::new(window)
        .expect("webview")
        .with_web_context(&mut context)
        .with_additional_browser_args(BROWSER_ARGS)
        .with_devtools(crate::webview_debug::enabled())
        .with_initialization_script(crate::webview_debug::script())
        .with_initialization_script(PAGE_SCRIPT)
        .with_navigation_handler(|url| {
            crate::webview_debug::navigation("yt-login", &url, true);
            true
        })
        .with_ipc_handler(move |_, msg| {
            if crate::webview_debug::handle("yt-login", &msg) {
                return;
            }
            if let Ok(PageEvent::Account { signed_in: true }) =
                serde_json::from_str::<PageEvent>(&msg)
            {
                done2.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        })
        .with_url(SIGN_IN_URL)
        .expect("url")
        .build()
        .expect("webview");
    event_loop.run(move |event, _, control_flow| {
        let _ = &webview;
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(200),
        );
        if done.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = std::fs::write(signed_in_marker(), "1");
            *control_flow = ControlFlow::ExitWithCode(0);
        }
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::ExitWithCode(1);
        }
    })
}

/// `wavify.exe --yt-player`: the hidden player. Commands on stdin, one
/// JSON object per line: {"op":"load","vid":..} / "play" / "pause" /
/// {"op":"seek","sec":..} / {"op":"volume","v":0..1} / {"op":"speed","v":..}
/// / "stop". Exits when stdin closes (Wavify quit).
pub fn run_player() -> ! {
    use wry::application::event::Event;
    use wry::application::event_loop::{ControlFlow, EventLoop};
    use wry::application::platform::windows::EventLoopExtWindows;
    use wry::application::window::WindowBuilder;
    use wry::webview::{WebContext, WebViewBuilder, WebViewBuilderExtWindows};

    let event_loop: EventLoop<Cmd> = EventLoop::new_any_thread();
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let cmd = match v.get("op").and_then(|o| o.as_str()) {
                Some("load") => v
                    .get("vid")
                    .and_then(|x| x.as_str())
                    .map(|id| Cmd::Load(id.to_string())),
                Some("stop") => Some(Cmd::Stop),
                Some(_) => Some(Cmd::Js(format!(
                    "window.__wavify && window.__wavify.cmd({line});"
                ))),
                None => None,
            };
            if let Some(cmd) = cmd {
                if proxy.send_event(cmd).is_err() {
                    break;
                }
            }
        }
        // Wavify is gone: so is its player
        std::process::exit(0);
    });

    let window = WindowBuilder::new()
        .with_title("Wavify YouTube Music player")
        .with_visible(false)
        .build(&event_loop)
        .expect("window");
    let mut context = WebContext::new(Some(profile_dir()));
    let webview = WebViewBuilder::new(window)
        .expect("webview")
        .with_web_context(&mut context)
        .with_additional_browser_args(BROWSER_ARGS)
        .with_devtools(crate::webview_debug::enabled())
        .with_initialization_script(crate::webview_debug::script())
        .with_initialization_script(PAGE_SCRIPT)
        .with_navigation_handler(|url| {
            crate::webview_debug::navigation("yt-player", &url, true);
            true
        })
        .with_ipc_handler(|_, msg| {
            if crate::webview_debug::handle("yt-player", &msg) {
                return;
            }
            // one event per line, for Wavify's reader
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{msg}");
            let _ = out.flush();
        })
        .with_url("about:blank")
        .expect("url")
        .build()
        .expect("webview");
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::UserEvent(cmd) = event {
            match cmd {
                Cmd::Load(id) => {
                    let id: String = id
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    webview.load_url(&format!("https://music.youtube.com/watch?v={id}"));
                }
                Cmd::Js(js) => {
                    let _ = webview.evaluate_script(&js);
                }
                Cmd::Stop => webview.load_url("about:blank"),
            }
        }
    })
}
