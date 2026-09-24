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
    const KEYS: [&str; 14] = [
        "oauth_token=",
        "access_token=",
        "refresh_token=",
        "\"access_token\":\"",
        "\"refresh_token\":\"",
        "code=",
        "code_verifier=",
        "datadome=",
        "webtoken=",
        "cid=",
        "initialCid=",
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
    use super::redact;

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
            redact(r#"{"access_token":"tok","expires_in":3}"#),
            r#"{"access_token":"[redacted]","expires_in":3}"#
        );
        assert_eq!(redact("nothing here"), "nothing here");
    }
}
