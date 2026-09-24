//! Embedded SoundCloud login using WebView2 (wry): a native window that
//! hosts the real Edge engine, so DataDome/captcha works. The person signs
//! in on soundcloud.com itself, as in a browser; once the site is signed
//! in, the page asks SoundCloud (from inside that session) to authorize
//! Wavify's app client, and the code it gets is exchanged for the app token
//! that can save (the site's own token is read-only for the app API). The
//! `sc://auth?code=...` redirect of SoundCloud's app sign-in page is still
//! caught, should it come up.

use crate::auth::{b64_sha256, gen_pkce, save_pending};
use crate::config::*;
use anyhow::{bail, Result};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};

pub fn login_window() -> Result<Token> {
    let (tx, rx) = std_mpsc::channel::<Result<Token>>();

    std::thread::spawn(move || {
        let _ = tx.send(run_login_window());
    });

    match rx.recv() {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(e)) => Err(e),
        Err(_) => bail!("login thread died"),
    }
}

fn extract_code(url: &str) -> Option<String> {
    if url.contains("sc://auth") {
        let pos = url.find("code=")?;
        let rest = &url[pos + 5..];
        let code = rest.split('&').next()?;
        if code.is_empty() {
            None
        } else {
            Some(code.to_string())
        }
    } else {
        None
    }
}

fn exchange_headless(code: &str, verifier: &str) -> Result<Token> {
    let http = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36")
        .build()?;
    let auth_header = b64_sha256(format!("{CLIENT_ID}:{CLIENT_SECRET}").as_bytes());
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("code", code),
        ("redirect_uri", "sc://auth"),
        ("code_verifier", verifier),
    ];
    let resp: OAuthTokenResponse = http
        .post(format!("{AUTH_API}/oauth/token"))
        .header("Accept-Charset", "UTF-8")
        .header("Authorization", format!("Basic {auth_header}"))
        .form(&params)
        .send()?
        .error_for_status()?
        .json()?;
    let token = Token::from_oauth(&resp);
    if let Ok(j) = serde_json::to_string(&token) {
        let _ = std::fs::write(token_path(), j);
    }
    Ok(token)
}

/// Instead of a blank window when SoundCloud's sign-in page can't load: a
/// note on top of it. SoundCloud's bot protection (DataDome) refusing the
/// page's own requests, or blocking this network outright, leaves the page
/// without its form; so does a page that never finishes loading.
const STATUS_SCRIPT: &str = r#"
(() => {
  if (window.__wavifyStatus) return;
  window.__wavifyStatus = true;
  const note = (msg) => {
    const show = () => {
      if (!document.body || document.getElementById('wavify-note')) return;
      const d = document.createElement('div');
      d.id = 'wavify-note';
      d.textContent = msg;
      d.style.cssText = 'position:fixed;left:16px;right:16px;top:16px;z-index:2147483647;padding:14px 16px;'
        + 'border-radius:8px;background:#2a2a2a;color:#fff;font:14px/1.4 "Segoe UI",sans-serif;'
        + 'box-shadow:0 8px 24px rgba(0,0,0,.5)';
      document.body.appendChild(d);
    };
    if (document.body) show(); else document.addEventListener('DOMContentLoaded', show);
  };
  const BLOCKED = "SoundCloud's bot protection is blocking this network for now, so its sign-in page "
    + "can't load. Close this window and try again later.";
  // a refused request of the page itself: blocked outright ("t=bv"), or a
  // captcha that DataDome's own script on the page will show
  const check = (status, body) => {
    if (status === 403 && /captcha-delivery/.test(body || '') && /[?&]t=bv/.test(body)) note(BLOCKED);
  };
  const send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function (...args) {
    this.addEventListener('load', () => { try { check(this.status, this.responseText); } catch (e) {} });
    return send.apply(this, args);
  };
  const fetch0 = window.fetch;
  if (fetch0) {
    window.fetch = (...args) => fetch0(...args).then((r) => {
      if (r.status === 403) { try { r.clone().text().then((t) => check(403, t), () => {}); } catch (e) {} }
      return r;
    });
  }
  // DataDome's block page, shown as the page itself
  const blockPage = () => /you have been blocked/i.test((document.body && document.body.innerText) || '');
  document.addEventListener('DOMContentLoaded', () => { if (blockPage()) note(BLOCKED); });
  // SoundCloud's sign-in page with no form after 15 s
  if (location.hostname === 'secure.soundcloud.com') {
    setTimeout(() => {
      if (!document.querySelector('input, button, iframe')) {
        note("SoundCloud's sign-in page didn't load. If its bot protection is blocking this network, "
          + "it lifts after a while: close this window and try again later.");
      }
    }, 15000);
  }
})();
"#;

/// Runs on soundcloud.com: once the site is signed in, authorizes Wavify's
/// client from the page (the request carries the session like any request
/// of the site, including cookies scripts can't read) and hands the code to
/// Wavify. "Signed in" is the `oauth_token` cookie when scripts can see it,
/// or the signed-in user in the page's own data (`__sc_hydration`); as
/// neither may show, it also simply tries now and then. If SoundCloud
/// refuses while signed in, Wavify gets the site's token (read-only for
/// saves) when the page can read it, else why it was refused.
const SITE_SCRIPT: &str = r#"
(() => {
  if (window.__wavifySite || window !== window.top || location.hostname !== 'soundcloud.com') return;
  window.__wavifySite = true;
  const cookie = (name) => {
    const m = document.cookie.match(new RegExp('(?:^|;\\s*)' + name + '=([^;]+)'));
    return m ? decodeURIComponent(m[1]) : null;
  };
  const signedIn = () => !!cookie('oauth_token')
    || (window.__sc_hydration || []).some((h) => h && h.hydratable === 'meUser' && h.data);
  const authorize = async (csrf) => {
    const headers = { 'Content-Type': 'application/json', 'Accept': 'application/json' };
    if (csrf) headers['X-Csrf-Token'] = csrf;
    const r = await fetch('__AUTH__/oauth/authorize?client_id=__CLIENT__', {
      method: 'POST',
      credentials: 'include',
      headers,
      body: JSON.stringify({
        client_id: '__CLIENT__',
        redirect_uri: '__REDIRECT__',
        response_type: 'code',
        scope: 'non-expiring',
        code_challenge: '__CHALLENGE__',
        code_challenge_method: 'S256',
      }),
    });
    const j = await r.json().catch(() => ({}));
    const m = /[?&]code=([^&#]+)/.exec(j.redirect_uri || '');
    return { code: m ? decodeURIComponent(m[1]) : null, status: r.status };
  };
  let busy = false, done = false, tries = 0;
  const attempt = async () => {
    if (busy || done) return;
    busy = true;
    tries++;
    const token = cookie('oauth_token');
    let got = { code: null, status: 0 };
    for (const csrf of token ? [token, null] : [null]) {
      try { got = await authorize(csrf); } catch (e) { got = { code: null, status: -1 }; }
      if (got.code) break;
    }
    busy = false;
    if (got.code) { done = true; window.ipc.postMessage('code=' + got.code); return; }
    if (signedIn()) {
      done = true;
      const t = cookie('oauth_token');
      window.ipc.postMessage(t ? 'webtoken=' + t : 'refused=' + got.status);
    }
  };
  const loaded = Date.now();
  setInterval(() => { if (signedIn()) attempt(); }, 1000);
  setInterval(() => { if (!signedIn() && tries < 40 && Date.now() - loaded > 8000) attempt(); }, 10000);
})();
"#;

/// What the window got to sign in with.
#[derive(Clone)]
enum Captured {
    /// From SoundCloud's app sign-in page (redirect sc://auth).
    AppCode(String),
    /// Authorized from the signed-in site (redirect REDIRECT_URI).
    SiteCode(String),
    /// The site's own token: read-only for the app API.
    WebToken(String),
    /// Signed in, but SoundCloud refused to authorize Wavify (HTTP status,
    /// -1 when the request didn't go through).
    Refused(String),
}

fn run_login_window() -> Result<Token> {
    use wry::application::{
        event::{Event, WindowEvent},
        event_loop::ControlFlow,
        window::WindowBuilder,
    };

    let pkce = gen_pkce();
    let _ = save_pending(&pkce.verifier);

    // soundcloud.com's own sign-in, as in a browser (see SITE_SCRIPT)
    let start_url = "https://soundcloud.com/signin";
    let site_script = SITE_SCRIPT
        .replace("__AUTH__", AUTH_API)
        .replace("__CLIENT__", CLIENT_ID)
        .replace("__REDIRECT__", REDIRECT_URI)
        .replace("__CHALLENGE__", &pkce.challenge);

    let event_loop: wry::application::event_loop::EventLoop<()> =
        wry::application::platform::windows::EventLoopExtWindows::new_any_thread();
    let window = WindowBuilder::new()
        .with_title("Sign in to SoundCloud — Wavify")
        .with_inner_size(wry::application::dpi::LogicalSize::new(520.0, 720.0))
        .with_window_icon(crate::ui::webview_window_icon())
        .build(&event_loop)?;

    let code_cell: Arc<Mutex<Option<Captured>>> = Arc::new(Mutex::new(None));
    let verifier = Arc::new(pkce.verifier);

    let cc = code_cell.clone();
    let from_site = code_cell.clone();
    let profile = wavify::sc_web::sc_profile_dir();
    let mut context = wry::webview::WebContext::new(Some(profile));
    let _webview = wry::webview::WebViewBuilder::new(window)?
        .with_web_context(&mut context)
        .with_devtools(crate::webview_debug::enabled())
        .with_initialization_script(crate::webview_debug::script())
        .with_initialization_script(STATUS_SCRIPT)
        .with_initialization_script(&site_script)
        .with_ipc_handler(move |_, msg| {
            if crate::webview_debug::handle("login", &msg) {
                return;
            }
            crate::dlog!("[login] page says {}", msg.split('=').next().unwrap_or(""));
            let token_like = |v: &str| {
                !v.is_empty()
                    && v.len() < 512
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.~".contains(c))
            };
            let got = if let Some(code) = msg.strip_prefix("code=") {
                token_like(code).then(|| Captured::SiteCode(code.to_string()))
            } else if let Some(tok) = msg.strip_prefix("webtoken=") {
                token_like(tok).then(|| Captured::WebToken(tok.to_string()))
            } else if let Some(status) = msg.strip_prefix("refused=") {
                let status: String = status
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '-')
                    .take(4)
                    .collect();
                Some(Captured::Refused(status))
            } else {
                None
            };
            if let (Some(got), Ok(mut g)) = (got, from_site.lock()) {
                g.get_or_insert(got);
            }
        })
        .with_url(start_url)?
        .with_navigation_handler(move |url| {
            if let Some(code) = extract_code(&url) {
                let mut g = cc.lock().unwrap();
                if g.is_none() {
                    *g = Some(Captured::AppCode(code));
                }
                crate::webview_debug::navigation("login", &url, false);
                return false; // swallow the sc:// redirect
            }
            // allow SoundCloud + SSO + DataDome hosts
            let allowed = url.starts_with("https://secure.soundcloud.com/")
                || url.starts_with("https://soundcloud.com/")
                || url.starts_with("https://accounts.google.com/")
                || url.starts_with("https://www.facebook.com/")
                || url.starts_with("https://appleid.apple.com/")
                || url.starts_with("https://geo.captcha-delivery.com/")
                || url.starts_with("https://ct.captcha-delivery.com/")
                || url.starts_with("https://api-auth.soundcloud.com/")
                || url.starts_with("https://cdn.captcha-delivery.com/")
                || url.starts_with("about:")
                || url.starts_with("data:");
            crate::webview_debug::navigation("login", &url, allowed);
            allowed
        })
        .build()?;

    let (token_tx, token_rx) = std_mpsc::channel::<Result<()>>();
    let token_tx = Arc::new(Mutex::new(Some(token_tx)));
    let mut exchange_started = false;
    let mut closed = false;

    // Note: this runs in a dedicated child process (wavify.exe --login),
    // so `run()`'s process::exit() on loop end is fine here. It also means
    // login_window() never returns, so failures are written to
    // auth_error.json right here (see `fail`) for the polling parent.
    // `run` never returns (it exits the process), so it is the tail.
    event_loop.run(move |event, _, control_flow| {
        // Poll: the captured code and the exchange result come from other
        // callbacks/threads, which don't wake this loop.
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        );

        if !exchange_started {
            let captured = code_cell.lock().ok().and_then(|g| g.clone());
            if let Some(captured) = captured {
                exchange_started = true;
                let verifier = verifier.to_string();
                let tx_slot = token_tx.clone();
                std::thread::spawn(move || {
                    let tx = tx_slot.lock().unwrap().take();
                    if let Some(tx) = tx {
                        let _ = tx.send(finish_sign_in(captured, &verifier));
                    }
                });
            }
        }
        if !closed {
            let res = match token_rx.try_recv() {
                Ok(res) => Some(res),
                // the exchange thread died without an answer
                Err(std_mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("token exchange failed")))
                }
                Err(std_mpsc::TryRecvError::Empty) => None,
            };
            if let Some(res) = res {
                closed = true;
                match res {
                    // exchange_headless has written token.json
                    Ok(_) => *control_flow = ControlFlow::Exit,
                    Err(e) => fail(&e.to_string(), control_flow),
                }
            }
        }

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            // (if `closed`, it is already exiting with the exchange result)
            if !closed && exchange_started {
                // Already signed in: hide, and let the exchange finish (the
                // branch above exits) rather than kill it mid-request.
                _webview.window().set_visible(false);
            } else if !closed {
                closed = true;
                fail("login window closed", control_flow);
            }
        }
    })
}

/// Turn what the window captured into token.json (the parent polls for it).
fn finish_sign_in(captured: Captured, verifier: &str) -> Result<()> {
    crate::dlog!(
        "[login] signing in with {}",
        match &captured {
            Captured::AppCode(_) => "the app sign-in page's code",
            Captured::SiteCode(_) => "a code authorized from the site",
            Captured::WebToken(_) => "the site's token",
            Captured::Refused(_) => "nothing: authorization refused",
        }
    );
    match captured {
        Captured::AppCode(code) => exchange_headless(&code, verifier).map(|_| ()),
        Captured::SiteCode(code) => {
            let (access, refresh, expires_in) =
                crate::cookie_auth::token_for_code(&code, verifier)?;
            crate::cookie_auth::save_access_token(&access, refresh, expires_in)?;
            crate::log!("login: app token from the signed-in site");
            Ok(())
        }
        Captured::WebToken(tok) => {
            // SoundCloud refused to authorize the app client: the site's
            // token still signs in, read-only for saves
            crate::cookie_auth::save_access_token(&tok, None, Some(365 * 24 * 3600))?;
            crate::log!("login: app authorization refused; using the site's token (read-only)");
            Ok(())
        }
        Captured::Refused(status) => {
            bail!("signed in on soundcloud.com, but SoundCloud refused to authorize Wavify (HTTP {status})")
        }
    }
}

/// Tell the parent (it polls for token.json or auth_error.json) that the
/// login failed, and exit.
fn fail(msg: &str, control_flow: &mut wry::application::event_loop::ControlFlow) {
    let _ = std::fs::write(auth_error_path(), msg);
    *control_flow = wry::application::event_loop::ControlFlow::ExitWithCode(1);
}
