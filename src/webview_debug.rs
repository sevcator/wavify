//! `--debug` for the WebView windows (SoundCloud sign-in, YouTube Music):
//! what each page does goes into that window process's debug log
//! (console.rs; secrets masked there): where it navigates, what it loads
//! (XHR / fetch with their status), its console, errors, and the text it
//! shows once loaded and settled. Devtools open with F12 / right click.

/// Every frame. Frames pass their lines to the top one, which hands them to
/// the window over IPC, marked so the window's own handler tells them apart.
const SCRIPT: &str = r#"
(() => {
  if (window.__wavifyDebug) return;
  window.__wavifyDebug = true;
  const top = window === window.top;
  const where = top ? '' : '(frame ' + location.host + ') ';
  const send = (kind, data) => {
    let text;
    try { text = typeof data === 'string' ? data : JSON.stringify(data); } catch (e) { text = String(data); }
    const line = ('__wavify_debug ' + where + kind + ' ' + text).slice(0, 20000);
    if (top) { try { window.ipc.postMessage(line); } catch (e) {} }
    else { try { window.top.postMessage({ wavifyDebugLine: line }, '*'); } catch (e) {} }
  };
  if (top) {
    window.addEventListener('message', (e) => {
      if (e.data && typeof e.data.wavifyDebugLine === 'string') { try { window.ipc.postMessage(e.data.wavifyDebugLine); } catch (x) {} }
    }, true);
  }
  send('document', location.href);
  for (const level of ['log', 'info', 'warn', 'error']) {
    const orig = console[level];
    console[level] = function (...args) {
      try {
        send('console.' + level, args.map((a) => { try { return typeof a === 'string' ? a : JSON.stringify(a); } catch (e) { return String(a); } }).join(' '));
      } catch (e) {}
      return orig.apply(this, args);
    };
  }
  window.addEventListener('error', (e) => {
    if (e.message) send('error', e.message + ' at ' + (e.filename || '?') + ':' + (e.lineno || 0));
    else if (e.target && (e.target.src || e.target.href)) send('resource-error', e.target.src || e.target.href);
  }, true);
  window.addEventListener('unhandledrejection', (e) => send('unhandledrejection', String(e.reason)));
  const open = XMLHttpRequest.prototype.open;
  const xsend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, u, ...rest) { this.__wavifyReq = m + ' ' + u; return open.call(this, m, u, ...rest); };
  XMLHttpRequest.prototype.send = function (...args) {
    this.addEventListener('loadend', () => send('xhr', this.__wavifyReq + ' -> ' + this.status));
    return xsend.apply(this, args);
  };
  const fetch0 = window.fetch;
  if (fetch0) {
    window.fetch = function (input, init) {
      const url = (input && input.url) || String(input);
      const method = (init && init.method) || (input && input.method) || 'GET';
      return fetch0.apply(this, arguments).then(
        (r) => { send('fetch', method + ' ' + url + ' -> ' + r.status); return r; },
        (e) => { send('fetch', method + ' ' + url + ' failed: ' + e); throw e; }
      );
    };
  }
  const snapshot = (why) => {
    try {
      send('snapshot', {
        why,
        url: location.href,
        title: document.title,
        text: ((document.body && document.body.innerText) || '').slice(0, 8000),
      });
    } catch (e) {}
  };
  document.addEventListener('DOMContentLoaded', () => snapshot('loaded'));
  window.addEventListener('load', () => setTimeout(() => snapshot('settled'), 3000));
})();
"#;

pub fn enabled() -> bool {
    crate::console::debug_enabled()
}

/// The script to add to a window's pages (none outside debug mode).
pub fn script() -> &'static str {
    if enabled() {
        SCRIPT
    } else {
        ""
    }
}

/// A debug line from the page: logged, true. Anything else is the window's
/// own message: false.
pub fn handle(window: &str, msg: &str) -> bool {
    match msg.strip_prefix("__wavify_debug ") {
        Some(line) => {
            crate::console::detail(&format!("WEBVIEW {window}"), line);
            true
        }
        None => false,
    }
}

/// A navigation the window allowed or stopped.
pub fn navigation(window: &str, url: &str, allowed: bool) {
    crate::console::detail(
        &format!("WEBVIEW {window} NAVIGATION"),
        &format!(
            "{}{}",
            crate::console::redact(url),
            if allowed { "" } else { " (stopped)" }
        ),
    );
}
