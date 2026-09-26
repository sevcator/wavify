use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Attach to the parent console if launched from a terminal.
/// If launched from Explorer (no console), all log! calls are silently dropped.
pub fn attach() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// Show a console for the GUI app when persistent Debug mode was enabled.
pub fn show() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::AllocConsole;
        let _ = AllocConsole();
    }
}

/// `--debug`: every log! line also goes to a file of this process's own
/// (<config>/debug/<time>_<role>_<pid>.log), secrets masked. Child processes
/// (sign-in, captcha, YouTube Music windows) inherit it through WAVIFY_DEBUG.
static DEBUG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static STARTED: OnceLock<std::time::Instant> = OnceLock::new();

/// The environment variable that passes debug mode to child processes.
pub const DEBUG_ENV: &str = "WAVIFY_DEBUG";

pub fn debug_enabled() -> bool {
    DEBUG_FILE.get().is_some()
}

/// Start debug logging for this process (`role`: "app", "login", ...).
/// Returns the log file's path.
pub fn start_debug(role: &str) -> Option<std::path::PathBuf> {
    let dir = crate::config::config_dir().join("debug");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!(
        "{}_{role}_{}.log",
        file_stamp(),
        std::process::id()
    ));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    let _ = STARTED.set(std::time::Instant::now());
    DEBUG_FILE.set(Mutex::new(file)).ok()?;
    // children spawned from here on log too
    std::env::set_var(DEBUG_ENV, "1");
    // iced's and wgpu's own messages (renderer, adapter, warnings) too
    static LOGGER: Logger = Logger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    // a panic ends up in the file as well
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(&format!("PANIC: {info}"));
        default_hook(info);
    }));
    Some(path)
}

/// The `log` crate's messages in debug mode: iced's at Info and up (the
/// renderer and adapter it picked), everyone else's warnings and errors.
struct Logger;

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn || m.target().starts_with("iced")
    }

    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            log(&format!("{} {}: {}", r.level(), r.target(), r.args()));
        }
    }

    fn flush(&self) {}
}

/// Write a line to stderr (visible only when launched from a terminal), and
/// to the debug file in debug mode.
pub fn log(msg: &str) {
    // masked everywhere: terminal output gets copied and shared too
    let line = shorten(&redact(msg));
    {
        let mut e = std::io::stderr().lock();
        let _ = writeln!(e, "{line}");
        let _ = e.flush();
    }
    if let Some(f) = DEBUG_FILE.get() {
        let at = STARTED
            .get()
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "[{at:>9.3}] {line}");
            let _ = f.flush();
        }
    }
}

/// Persist an untruncated, credential-redacted diagnostic block. Full network
/// bodies are written to the debug file, not stderr, to keep the live console
/// usable while still retaining the payload for diagnosis.
pub fn detail(label: &str, payload: &str) {
    if !debug_enabled() {
        return;
    }
    let payload = sanitize_payload(payload);
    let Some(file) = DEBUG_FILE.get() else { return };
    let at = STARTED
        .get()
        .map(|started| started.elapsed().as_secs_f64())
        .unwrap_or(0.0);
    if let Ok(mut file) = file.lock() {
        let _ = writeln!(file, "[{at:>9.3}] --- {label} ---");
        for line in payload.lines() {
            let _ = writeln!(file, "[{at:>9.3}] {}", redact(line));
        }
        let _ = file.flush();
    }
}

pub fn http_request(method: &str, url: &str, body: Option<&serde_json::Value>) {
    if !debug_enabled() {
        return;
    }
    let mut block = format!("{method} {}", redact(url));
    if let Some(body) = body {
        block.push_str("\nPayload:\n");
        block.push_str(
            &serde_json::to_string_pretty(&sanitize_json(body.clone()))
                .unwrap_or_else(|_| "<could not serialize payload>".into()),
        );
    }
    detail("HTTP REQUEST", &block);
}

pub fn http_request_raw(method: &str, url: &str, body: Option<&[u8]>) {
    if !debug_enabled() {
        return;
    }
    let mut block = format!("{method} {}", redact(url));
    if let Some(body) = body {
        block.push_str("\nPayload:\n");
        block.push_str(&sanitize_payload(&String::from_utf8_lossy(body)));
    }
    detail("HTTP REQUEST", &block);
}

pub fn http_response(method: &str, url: &str, status: impl std::fmt::Display, body: &[u8]) {
    if !debug_enabled() {
        return;
    }
    let body = sanitize_payload(&String::from_utf8_lossy(body));
    detail(
        "HTTP RESPONSE",
        &format!("{method} {} -> {status}\nBody:\n{body}", redact(url)),
    );
}

pub fn http_binary_response(
    method: &str,
    url: &str,
    status: impl std::fmt::Display,
    content_type: Option<&str>,
    byte_count: usize,
) {
    detail(
        "HTTP BINARY RESPONSE",
        &format!(
            "{method} {} -> {status}; content-type={}; bytes={byte_count} (body omitted)",
            redact(url),
            content_type.unwrap_or("unknown")
        ),
    );
}

fn sanitize_payload(payload: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
        serde_json::to_string_pretty(&sanitize_json(value)).unwrap_or_else(|_| redact(payload))
    } else {
        redact(payload)
    }
}

fn sanitize_json(mut value: serde_json::Value) -> serde_json::Value {
    fn visit(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, value) in map.iter_mut() {
                    let key_lower = key.to_ascii_lowercase();
                    if key_lower.contains("token")
                        || key_lower.contains("secret")
                        || key_lower.contains("password")
                        || key_lower.contains("cookie")
                        || key_lower.contains("authorization")
                        || key_lower.contains("captcha")
                        || key_lower == "policy"
                        || key_lower == "signature"
                        || key_lower == "key-pair-id"
                        || key_lower.starts_with("x-amz-")
                        || key_lower == "code"
                    {
                        *value = serde_json::Value::String("[redacted]".into());
                    } else {
                        visit(value);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(visit),
            serde_json::Value::String(text) => *text = redact(text),
            _ => {}
        }
    }
    visit(&mut value);
    value
}

/// Very long words (a captcha's multi-kilobyte query strings) cut to their
/// first 300 characters; the rest of the line stays.
fn shorten(line: &str) -> String {
    const MAX: usize = 300;
    if line.len() <= MAX {
        return line.to_string();
    }
    line.split(' ')
        .map(|w| {
            if w.chars().count() > MAX {
                let cut: String = w.chars().take(MAX).collect();
                format!("{cut}…[{} more]", w.chars().count() - MAX)
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Secrets out of a log line: tokens, codes, cookies and DataDome's client
/// ids become "[redacted]", so a debug log can be shared.
pub fn redact(s: &str) -> String {
    // (marker, the value runs until one of these)
    const KEYS: [&str; 26] = [
        "oauth_token=",
        "access_token=",
        "refresh_token=",
        "token=",
        "session_token=",
        "password=",
        "client_secret=",
        "authorization=",
        "\"access_token\":\"",
        "\"refresh_token\":\"",
        "code=",
        "code_verifier=",
        "datadome=",
        "webtoken=",
        "cid=",
        "initialCid=",
        "Policy=",
        "Signature=",
        "Key-Pair-Id=",
        "X-Amz-Credential=",
        "X-Amz-Signature=",
        "X-Amz-Security-Token=",
        "jwt=",
        "OAuth ",
        "Bearer ",
        "\"cookie\":\"",
    ];
    let mut out = s.to_string();
    for key in KEYS {
        let mut from = 0;
        while let Some(i) = out[from..].find(key).map(|i| i + from) {
            let start = i + key.len();
            let len = out[start..]
                .find(|c: char| matches!(c, '&' | ';' | '"' | '\'' | ' ' | ',' | '}' | ')' | '\n'))
                .unwrap_or(out.len() - start);
            from = if len > 0 && &out[start..start + len] != "[redacted]" {
                out.replace_range(start..start + len, "[redacted]");
                start + "[redacted]".len()
            } else {
                start + len
            };
        }
    }
    out
}

/// "2026-09-23_17-05-42" (UTC) for file names; no date crate needed.
fn file_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    )
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::console::log(&format!($($arg)*))
    };
}

/// log!, only in debug mode (for detail too chatty for a normal run).
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        if $crate::console::debug_enabled() {
            $crate::console::log(&format!($($arg)*))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{redact, sanitize_payload};

    #[test]
    fn secrets_are_masked() {
        assert_eq!(
            redact("GET https://x/?client_id=abc&code=S3CR3T&state=1"),
            "GET https://x/?client_id=abc&code=[redacted]&state=1"
        );
        assert_eq!(
            redact("Cookie: oauth_token=2-1-2-x; datadome=ab~c"),
            "Cookie: oauth_token=[redacted]; datadome=[redacted]"
        );
        assert_eq!(
            redact("Authorization: OAuth 2-3-4"),
            "Authorization: OAuth [redacted]"
        );
        assert_eq!(
            redact("https://cdn.example/stream?Policy=signed&Signature=secret&keep=yes"),
            "https://cdn.example/stream?Policy=[redacted]&Signature=[redacted]&keep=yes"
        );
        assert_eq!(
            redact(r#"{"access_token":"tok","expires_in":3}"#),
            r#"{"access_token":"[redacted]","expires_in":3}"#
        );
        assert_eq!(redact("nothing here"), "nothing here");
    }

    #[test]
    fn detailed_payloads_keep_user_content_but_redact_credentials() {
        let safe = sanitize_payload(
            r#"{"comment":{"body":"hello"},"session":{"access_token":"secret","refresh_token":"refresh"},"url":"https://x/?oauth_token=secret"}"#,
        );
        assert!(safe.contains("hello"));
        assert!(!safe.contains("secret"));
        assert!(!safe.contains("refresh\""));
        assert!(safe.contains("[redacted]"));
    }
}
