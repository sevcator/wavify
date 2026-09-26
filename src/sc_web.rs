//! Embedded SoundCloud Web Bridge using WebView2 (wry): a hidden Microsoft
//! Edge Chromium instance running on soundcloud.com.
//!
//! SoundCloud protects its mutating APIs (likes, follows, playlists, comments)
//! behind DataDome bot protection. Direct HTTP mutation requests from non-browser
//! TLS stacks (such as Windows SChannel) are flagged as bots (HTTP 403 / t=fe).
//!
//! This bridge executes write operations directly inside Edge Chromium's genuine
//! browser runtime with authentic TLS handshakes (BoringSSL), browser headers,
//! and DataDome session state.
//!
//! When DataDome still wants a check (its captcha), the bridge's window shows
//! it: DataDome trusts only the browser that passed it, and this is the one
//! that sends the writes. Once passed, the write goes again.
//!
//! The bridge runs only while needed: it starts with the first write and
//! quits after a few idle minutes (a WebView2 costs a couple of hundred MB).

use crate::config::{config_dir, CLIENT_ID};
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// The persistent profile directory used for SoundCloud web session data.
pub fn sc_profile_dir() -> PathBuf {
    let dir = config_dir().join("sc_web");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BridgeRequest {
    pub id: u64,
    pub url: String,
    pub method: String,
    pub body: Option<serde_json::Value>,
    pub form: bool,
    pub access: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct BridgeResponse {
    pub id: u64,
    pub status: u16,
    pub ok: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

enum BridgeCmd {
    Req(String),
    /// Show DataDome's check (its captcha page URL).
    Check(String),
}

/// A line in config/bridge.log (and the console): what refused writes got
/// back, and the checks shown. Kept small: past 256 KB it starts over.
pub fn blog(msg: &str) {
    crate::log!("sc_bridge: {msg}");
    let path = config_dir().join("bridge.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

/// How long the bridge may sit idle before it's shut down.
const IDLE_QUIT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// How long the user gets for a check.
const CHECK_WAIT: std::time::Duration = std::time::Duration::from_secs(180);

/// Runs as `wavify.exe --sc-bridge`: a hidden background WebView2 instance.
pub fn run_bridge() -> ! {
    use wry::application::event::Event;
    use wry::application::event_loop::{ControlFlow, EventLoop};
    use wry::application::platform::windows::EventLoopExtWindows;
    use wry::application::window::WindowBuilder;
    use wry::webview::{WebContext, WebViewBuilder};

    let event_loop: EventLoop<BridgeCmd> = EventLoop::new_any_thread();
    let proxy = event_loop.create_proxy();

    // Read commands from stdin and forward to event loop
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let cmd = match serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v.get("check").and_then(|c| c.as_str()).map(str::to_string))
            {
                Some(url) => BridgeCmd::Check(url),
                None => BridgeCmd::Req(line.to_string()),
            };
            if proxy.send_event(cmd).is_err() {
                break;
            }
        }
        std::process::exit(0);
    });

    let icon = image::load_from_memory(include_bytes!("../assets/icon/wavify-64.png"))
        .ok()
        .map(|i| i.into_rgba8())
        .and_then(|i| {
            let (w, h) = i.dimensions();
            wry::application::window::Icon::from_rgba(i.into_raw(), w, h).ok()
        });
    let window = WindowBuilder::new()
        .with_title("SoundCloud check - Wavify")
        .with_window_icon(icon)
        .with_inner_size(wry::application::dpi::LogicalSize::new(460.0, 680.0))
        .with_visible(false)
        .build(&event_loop)
        .expect("window");

    let profile = sc_profile_dir();
    let mut context = WebContext::new(Some(profile));

    const INIT_SCRIPT: &str = r#"
    (() => {
        if (window.__scBridge) return;
        window.__scBridge = {
            ready: false,
            queue: [],
            dispatch(req) {
                if (!this.ready) {
                    this.queue.push(req);
                    return;
                }
                this.execute(req);
            },
            async execute(req) {
                const { id, url, method, body, form, access } = req;
                const headers = { 'Accept': 'application/json' };
                if (access) {
                    headers['Authorization'] = `OAuth ${access}`;
                }
                if (body !== null && body !== undefined) {
                    headers['Content-Type'] = form
                        ? 'application/x-www-form-urlencoded; charset=UTF-8'
                        : 'application/json';
                }
                try {
                    const resp = await fetch(url, {
                        method: method || 'GET',
                        headers,
                        body: (body !== null && body !== undefined)
                            ? (form ? new URLSearchParams(body).toString() : JSON.stringify(body))
                            : undefined,
                        credentials: 'include'
                    });
                    const text = await resp.text();
                    let data = null;
                    try { data = JSON.parse(text); } catch (_) { data = text; }
                    window.ipc.postMessage(JSON.stringify({
                        id,
                        status: resp.status,
                        ok: resp.ok,
                        data,
                        error: null
                    }));
                } catch (err) {
                    window.ipc.postMessage(JSON.stringify({
                        id,
                        status: 0,
                        ok: false,
                        data: null,
                        error: String(err)
                    }));
                }
            }
        };

        const markReady = () => {
            if (window.__scBridge.ready) return;
            window.__scBridge.ready = true;
            while (window.__scBridge.queue.length > 0) {
                const r = window.__scBridge.queue.shift();
                window.__scBridge.execute(r);
            }
        };

        // DataDome's check over the page, as its own tag shows it: its
        // captcha in a frame. Passing it hands this page a new datadome
        // cookie (the frame's message, or the cookie simply changing).
        const ddCookie = () => (document.cookie.match(/(?:^|; )datadome=([^;]*)/) || [])[1] || '';
        window.__scBridge.endCheck = () => {};
        window.__scBridge.check = (url) => {
            window.__scBridge.endCheck();
            const before = ddCookie();
            const wrap = document.createElement('div');
            wrap.style.cssText = 'position:fixed;inset:0;z-index:2147483647;background:#121212;display:flex;flex-direction:column';
            const note = document.createElement('div');
            note.textContent = 'SoundCloud wants to make sure you are not a bot. Complete the check and Wavify will carry on.';
            note.style.cssText = 'color:#fff;font:14px "Segoe UI",sans-serif;padding:14px 16px;line-height:1.4';
            const frame = document.createElement('iframe');
            frame.src = url;
            frame.style.cssText = 'flex:1;border:0;width:100%;background:#fff';
            wrap.append(note, frame);
            document.documentElement.appendChild(wrap);
            let open = true;
            let poll = null;
            const done = (passed) => {
                if (!open) return;
                open = false;
                clearInterval(poll);
                window.removeEventListener('message', onMessage);
                wrap.remove();
                window.__scBridge.endCheck = () => {};
                window.ipc.postMessage(JSON.stringify({ check: passed ? 'passed' : 'closed' }));
            };
            const onMessage = (e) => {
                if (!String(e.origin).includes('captcha-delivery.com')) return;
                let d = e.data;
                try { if (typeof d === 'string') d = JSON.parse(d); } catch (_) {}
                if (d && typeof d.cookie === 'string' && d.cookie.indexOf('datadome=') >= 0) {
                    document.cookie = d.cookie;
                    done(true);
                }
            };
            window.addEventListener('message', onMessage);
            poll = setInterval(() => {
                const now = ddCookie();
                if (now && now !== before) done(true);
            }, 1000);
            window.__scBridge.endCheck = () => done(false);
        };

        if (document.readyState === 'complete') {
            setTimeout(markReady, 1000);
        } else {
            window.addEventListener('load', () => setTimeout(markReady, 1000));
        }
        // Fallback: ready after 3 seconds regardless
        setTimeout(markReady, 3000);
    })();
    "#;

    let webview = WebViewBuilder::new(window)
        .expect("webview builder")
        .with_web_context(&mut context)
        .with_initialization_script(INIT_SCRIPT)
        .with_ipc_handler(|window, msg| {
            // a check that ended: the window goes back out of sight
            if msg.contains("\"check\"") {
                window.set_visible(false);
            }
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{msg}");
            let _ = out.flush();
        })
        .with_url("https://soundcloud.com")
        .expect("url")
        .build()
        .expect("webview build");

    event_loop.run(move |event, _, control_flow| {
        use wry::application::event::WindowEvent;
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(BridgeCmd::Req(line)) => {
                let json_literal =
                    serde_json::to_string(&line).unwrap_or_else(|_| "{}".to_string());
                let js = format!(
                    "window.__scBridge && window.__scBridge.dispatch(JSON.parse({json_literal}));"
                );
                let _ = webview.evaluate_script(&js);
            }
            Event::UserEvent(BridgeCmd::Check(url)) => {
                let url_literal = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".into());
                let _ = webview.evaluate_script(&format!(
                    "window.__scBridge && window.__scBridge.check({url_literal});"
                ));
                let window = webview.window();
                window.set_visible(true);
                window.set_focus();
            }
            // closed: the check is given up, the bridge goes on out of sight
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                webview.window().set_visible(false);
                let _ = webview.evaluate_script(
                    "window.__scBridge && window.__scBridge.endCheck && window.__scBridge.endCheck();",
                );
            }
            _ => {}
        }
    });
}

// -----------------------------------------------------------------------------
// Parent process client
// -----------------------------------------------------------------------------

struct BridgeChild {
    _child: Child,
    stdin: ChildStdin,
}

/// Who waits for the check shown now: told whether it was passed.
type CheckWaiters = Arc<Mutex<Vec<oneshot::Sender<bool>>>>;

pub struct ScBridge {
    child: Mutex<Option<BridgeChild>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<BridgeResponse>>>>,
    next_id: AtomicU64,
    /// A check is on screen.
    checking: Arc<AtomicBool>,
    check_waiters: CheckWaiters,
    /// The last request, for the idle shutdown.
    last_used: Mutex<std::time::Instant>,
    reaper: AtomicBool,
}

impl ScBridge {
    /// The bridge itself starts with the first write (see ensure_started).
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(None),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            checking: Arc::new(AtomicBool::new(false)),
            check_waiters: Arc::new(Mutex::new(Vec::new())),
            last_used: Mutex::new(std::time::Instant::now()),
            reaper: AtomicBool::new(false),
        })
    }

    /// Idle a while (no request, nothing pending, no check on screen): the
    /// bridge and its WebView2 go, and the next write starts them again.
    fn stop_if_idle(&self) {
        let idle = self
            .last_used
            .lock()
            .map(|t| t.elapsed() >= IDLE_QUIT)
            .unwrap_or(false);
        if !idle
            || self.checking.load(Ordering::SeqCst)
            || !self.pending.lock().map(|p| p.is_empty()).unwrap_or(false)
        {
            return;
        }
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut c) = guard.take() {
                // its stdin closing ends it; the kill makes sure
                let _ = c._child.kill();
                let _ = c._child.wait();
                crate::log!("sc_bridge: idle, shut down");
            }
        }
    }

    fn ensure_started(self: &Arc<Self>) {
        if let Ok(mut t) = self.last_used.lock() {
            *t = std::time::Instant::now();
        }
        if !self.reaper.swap(true, Ordering::SeqCst) {
            let weak = Arc::downgrade(self);
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                match weak.upgrade() {
                    Some(bridge) => bridge.stop_if_idle(),
                    None => break,
                }
            });
        }
        let mut guard = self.child.lock().unwrap();
        if let Some(c) = guard.as_mut() {
            if let Ok(Some(_)) = c._child.try_wait() {
                *guard = None;
            } else {
                return;
            }
        }

        let Ok(mut exe) = std::env::current_exe() else {
            crate::log!("sc_bridge: cannot determine current_exe");
            return;
        };
        if exe.file_stem().and_then(|s| s.to_str()) != Some("wavify") {
            let candidate = exe.with_file_name("wavify.exe");
            if candidate.exists() {
                exe = candidate;
            }
        }

        let mut cmd = Command::new(exe);
        cmd.arg("--sc-bridge")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                crate::log!("sc_bridge: failed to spawn --sc-bridge: {e}");
                return;
            }
        };

        let stdin = match child.stdin.take() {
            Some(s) => s,
            None => {
                crate::log!("sc_bridge: failed to take stdin");
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                crate::log!("sc_bridge: failed to take stdout");
                return;
            }
        };

        let pending_map = self.pending.clone();
        let checking = self.checking.clone();
        let waiters = self.check_waiters.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // the end of a check: whoever waits on it hears how it went
                if let Some(result) = serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| v.get("check").and_then(|c| c.as_str()).map(str::to_string))
                {
                    let passed = result == "passed";
                    blog(&format!("check {result}"));
                    checking.store(false, Ordering::SeqCst);
                    for tx in waiters
                        .lock()
                        .map(|mut w| std::mem::take(&mut *w))
                        .unwrap_or_default()
                    {
                        let _ = tx.send(passed);
                    }
                    continue;
                }
                if let Ok(resp) = serde_json::from_str::<BridgeResponse>(line) {
                    let mut p = pending_map.lock().unwrap();
                    if let Some(tx) = p.remove(&resp.id) {
                        let _ = tx.send(resp);
                    }
                }
            }
        });

        *guard = Some(BridgeChild {
            _child: child,
            stdin,
        });
        crate::log!("sc_bridge: background WebView2 worker started");
    }

    /// A write through the bridge. DataDome asking for its check: the check
    /// is shown (see run_bridge), and once passed the write goes again.
    pub async fn request(
        self: &Arc<Self>,
        url: &str,
        method: &str,
        body: Option<serde_json::Value>,
        access: Option<&str>,
    ) -> Result<BridgeResponse> {
        self.request_encoded(url, method, body, false, access).await
    }

    /// A form-encoded write through the browser bridge. Some legacy
    /// SoundCloud endpoints parse bracketed form fields but ignore JSON bodies.
    pub async fn request_form(
        self: &Arc<Self>,
        url: &str,
        method: &str,
        fields: &[(&str, &str)],
        access: Option<&str>,
    ) -> Result<BridgeResponse> {
        let body = fields
            .iter()
            .map(|(key, value)| {
                (
                    (*key).to_string(),
                    serde_json::Value::String((*value).to_string()),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        self.request_encoded(
            url,
            method,
            Some(serde_json::Value::Object(body)),
            true,
            access,
        )
        .await
    }

    async fn request_encoded(
        self: &Arc<Self>,
        url: &str,
        method: &str,
        body: Option<serde_json::Value>,
        form: bool,
        access: Option<&str>,
    ) -> Result<BridgeResponse> {
        let resp = self
            .request_once(url, method, body.clone(), form, access)
            .await?;
        if !resp.ok {
            // what a refused write got, for bridge.log (no tokens in it)
            let path = url.split('?').next().unwrap_or(url);
            let what = resp
                .error
                .clone()
                .or_else(|| resp.data.as_ref().map(|d| d.to_string()))
                .unwrap_or_default();
            blog(&format!(
                "{method} {path} -> {}: {}",
                resp.status,
                what.chars().take(300).collect::<String>()
            ));
        }
        if resp.status != 403 {
            return Ok(resp);
        }
        let text = match &resp.data {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        };
        let Some(check) = crate::captcha::url_in(&text).filter(|u| !crate::captcha::is_block(u))
        else {
            return Ok(resp);
        };
        crate::log!("sc_bridge: DataDome wants a check, showing it");
        if !self.solve_check(&check).await {
            return Ok(resp);
        }
        self.request_once(url, method, body, form, access).await
    }

    /// Show DataDome's check in the bridge's window and wait for it to be
    /// passed (true) or given up (false). Writes refused meanwhile wait on
    /// the same check.
    pub async fn solve_check(self: &Arc<Self>, url: &str) -> bool {
        self.ensure_started();
        blog(&format!(
            "showing SoundCloud's check ({})",
            url.chars().take(80).collect::<String>()
        ));
        let (tx, rx) = oneshot::channel();
        if let Ok(mut w) = self.check_waiters.lock() {
            w.push(tx);
        }
        if !self.checking.swap(true, Ordering::SeqCst) {
            let line = serde_json::json!({ "check": url }).to_string();
            let sent = self
                .child
                .lock()
                .ok()
                .and_then(|mut g| {
                    g.as_mut().map(|c| {
                        writeln!(c.stdin, "{line}")
                            .and_then(|_| c.stdin.flush())
                            .is_ok()
                    })
                })
                .unwrap_or(false);
            if !sent {
                self.checking.store(false, Ordering::SeqCst);
                return false;
            }
        }
        match tokio::time::timeout(CHECK_WAIT, rx).await {
            Ok(Ok(passed)) => passed,
            _ => {
                self.checking.store(false, Ordering::SeqCst);
                false
            }
        }
    }

    async fn request_once(
        self: &Arc<Self>,
        url: &str,
        method: &str,
        body: Option<serde_json::Value>,
        form: bool,
        access: Option<&str>,
    ) -> Result<BridgeResponse> {
        self.ensure_started();

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = BridgeRequest {
            id,
            url: url.to_string(),
            method: method.to_string(),
            body,
            form,
            access: access.map(str::to_string),
        };
        crate::console::http_request(&req.method, &req.url, req.body.as_ref());

        let json_line = serde_json::to_string(&req)?;
        let (tx, rx) = oneshot::channel();

        {
            let mut p = self.pending.lock().unwrap();
            p.insert(id, tx);
        }

        {
            let mut guard = self.child.lock().unwrap();
            let Some(child) = guard.as_mut() else {
                let mut p = self.pending.lock().unwrap();
                p.remove(&id);
                bail!("sc_bridge is not running");
            };
            if writeln!(child.stdin, "{json_line}")
                .and_then(|_| child.stdin.flush())
                .is_err()
            {
                *guard = None;
                let mut p = self.pending.lock().unwrap();
                p.remove(&id);
                bail!("sc_bridge stdin write failed");
            }
        }

        // Wait with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(20), rx).await {
            Ok(Ok(resp)) => {
                let body = serde_json::to_vec(&serde_json::json!({
                    "ok": resp.ok,
                    "data": &resp.data,
                    "error": &resp.error,
                }))
                .unwrap_or_default();
                crate::console::http_response(method, url, resp.status, &body);
                Ok(resp)
            }
            Ok(Err(_)) => bail!("sc_bridge response channel closed"),
            Err(_) => {
                let mut p = self.pending.lock().unwrap();
                p.remove(&id);
                bail!("sc_bridge request timed out");
            }
        }
    }

    // -------------------------------------------------------------------------
    // High-level mutation methods
    // -------------------------------------------------------------------------

    pub async fn like_track(
        self: &Arc<Self>,
        access: &str,
        me_id: i64,
        track_id: i64,
        liked: bool,
    ) -> Result<()> {
        let method = if liked { "DELETE" } else { "PUT" };
        let url = format!("https://api-v2.soundcloud.com/users/{me_id}/track_likes/{track_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, method, None, Some(access)).await?;
        if resp.ok || (liked && resp.status == 404) {
            Ok(())
        } else {
            bail!(
                "like_track failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn follow_user(
        self: &Arc<Self>,
        access: &str,
        user_id: i64,
        follow: bool,
    ) -> Result<()> {
        let method = if follow { "DELETE" } else { "POST" };
        let url =
            format!("https://api-v2.soundcloud.com/me/followings/{user_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, method, None, Some(access)).await?;
        if resp.ok || (follow && resp.status == 404) {
            Ok(())
        } else {
            bail!(
                "follow_user failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn create_playlist(
        self: &Arc<Self>,
        access: &str,
        title: &str,
        sharing: &str,
        track_id: Option<i64>,
    ) -> Result<crate::api::Playlist> {
        let mut pl_map = serde_json::Map::new();
        pl_map.insert("title".to_string(), serde_json::json!(title));
        pl_map.insert("sharing".to_string(), serde_json::json!(sharing));
        if let Some(tid) = track_id {
            pl_map.insert("tracks".to_string(), serde_json::json!([tid]));
        }
        let body = serde_json::json!({ "playlist": pl_map });
        let url = format!("https://api-v2.soundcloud.com/playlists?client_id={CLIENT_ID}");
        let resp = self.request(&url, "POST", Some(body), Some(access)).await?;
        if resp.ok {
            if let Some(data) = resp.data {
                let pl: crate::api::Playlist = serde_json::from_value(data)?;
                return Ok(pl);
            }
        }
        bail!(
            "create_playlist failed (HTTP {}): {:?}",
            resp.status,
            resp.error.or_else(|| resp.data.map(|d| d.to_string()))
        )
    }

    pub async fn delete_playlist(self: &Arc<Self>, access: &str, playlist_id: i64) -> Result<()> {
        let url =
            format!("https://api-v2.soundcloud.com/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, "DELETE", None, Some(access)).await?;
        if resp.ok || resp.status == 404 {
            Ok(())
        } else {
            bail!(
                "delete_playlist failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn add_track_to_playlist(
        self: &Arc<Self>,
        access: &str,
        playlist_id: i64,
        track_id: i64,
        existing_tracks: Vec<i64>,
    ) -> Result<()> {
        let mut tracks = existing_tracks;
        if !tracks.contains(&track_id) {
            tracks.push(track_id);
        }
        let body = serde_json::json!({
            "playlist": {
                "tracks": tracks
            }
        });
        let url =
            format!("https://api-v2.soundcloud.com/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, "PUT", Some(body), Some(access)).await?;
        if resp.ok {
            Ok(())
        } else {
            bail!(
                "add_track_to_playlist failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn remove_track_from_playlist(
        self: &Arc<Self>,
        access: &str,
        playlist_id: i64,
        track_id: i64,
        existing_tracks: Vec<i64>,
    ) -> Result<()> {
        let tracks: Vec<i64> = existing_tracks
            .into_iter()
            .filter(|&id| id != track_id)
            .collect();
        let body = serde_json::json!({
            "playlist": {
                "tracks": tracks
            }
        });
        let url =
            format!("https://api-v2.soundcloud.com/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, "PUT", Some(body), Some(access)).await?;
        if resp.ok {
            Ok(())
        } else {
            bail!(
                "remove_track_from_playlist failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn like_playlist(
        self: &Arc<Self>,
        access: &str,
        me_id: i64,
        playlist_id: i64,
        liked: bool,
    ) -> Result<()> {
        let method = if liked { "DELETE" } else { "PUT" };
        let url = format!("https://api-v2.soundcloud.com/users/{me_id}/playlist_likes/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, method, None, Some(access)).await?;
        if resp.ok || (liked && resp.status == 404) {
            Ok(())
        } else {
            bail!(
                "like_playlist failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    /// Save (or remove) one of SoundCloud's own playlists, a mix or a
    /// station, in Your Library: the web app's system_playlist_likes.
    pub async fn like_system_playlist(
        self: &Arc<Self>,
        access: &str,
        me_id: i64,
        urn: &str,
        liked: bool,
    ) -> Result<()> {
        let method = if liked { "DELETE" } else { "PUT" };
        let url = format!(
            "https://api-v2.soundcloud.com/users/{me_id}/system_playlist_likes/{urn}?client_id={CLIENT_ID}"
        );
        let resp = self.request(&url, method, None, Some(access)).await?;
        if resp.ok || (liked && resp.status == 404) {
            Ok(())
        } else {
            bail!(
                "like_system_playlist failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn repost_track(
        self: &Arc<Self>,
        access: &str,
        track_id: i64,
        reposted: bool,
    ) -> Result<()> {
        let method = if reposted { "DELETE" } else { "PUT" };
        let url = format!(
            "https://api-v2.soundcloud.com/me/track_reposts/{track_id}?client_id={CLIENT_ID}"
        );
        let resp = self.request(&url, method, None, Some(access)).await?;
        if resp.ok || (reposted && resp.status == 404) {
            Ok(())
        } else {
            bail!(
                "repost_track failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn post_comment(
        self: &Arc<Self>,
        access: &str,
        track_id: i64,
        text: &str,
        track_time_ms: u64,
    ) -> Result<crate::api::Comment> {
        let url = format!(
            "https://api-v2.soundcloud.com/tracks/{track_id}/comments?client_id={CLIENT_ID}"
        );
        let timestamp = track_time_ms.to_string();
        let fields = [
            ("comment[body]", text),
            ("comment[timestamp]", timestamp.as_str()),
        ];
        let resp = self
            .request_form(&url, "POST", &fields, Some(access))
            .await?;
        if resp.ok {
            if let Some(data) = resp.data {
                let comment: crate::api::Comment = serde_json::from_value(data)?;
                return Ok(comment);
            }
        }
        bail!(
            "post_comment failed (HTTP {}): {:?}",
            resp.status,
            resp.error.or_else(|| resp.data.map(|d| d.to_string()))
        )
    }

    pub async fn add_track_reaction(
        self: &Arc<Self>,
        access: &str,
        track_id: i64,
        second: u64,
        codepoint: &str,
    ) -> Result<()> {
        const MUTATION: &str = "mutation UpsertInteraction($input: InteractionInput!) { \
            upsertInteraction(input: $input) { targetUrn } }";
        let body = serde_json::json!({
            "query": MUTATION,
            "variables": {
                "input": {
                    "targetUrn": format!("ts:{second}"),
                    "parentUrn": format!("soundcloud:tracks:{track_id}"),
                    "interactionTypeUrn": "sc:interactiontype:trackreaction",
                    "interactionTypeValueUrn": format!("sc:interactiontypevalue:{codepoint}"),
                }
            }
        });
        let url = "https://graph.soundcloud.com/graphql";
        let resp = self.request(url, "POST", Some(body), Some(access)).await?;
        if resp.ok {
            Ok(())
        } else {
            bail!(
                "add_track_reaction failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }

    pub async fn delete_comment(self: &Arc<Self>, access: &str, comment_id: i64) -> Result<()> {
        let url =
            format!("https://api-v2.soundcloud.com/comments/{comment_id}?client_id={CLIENT_ID}");
        let resp = self.request(&url, "DELETE", None, Some(access)).await?;
        if resp.ok || resp.status == 404 {
            Ok(())
        } else {
            bail!(
                "delete_comment failed (HTTP {}): {:?}",
                resp.status,
                resp.error.or_else(|| resp.data.map(|d| d.to_string()))
            )
        }
    }
}
