//! SoundCloud's DataDome sometimes answers a write with a captcha (or its
//! block page) instead of doing it. The captcha is shown in the SoundCloud
//! bridge's window (the browser DataDome trusts once it's passed there, see
//! sc_web), and the write goes again; meanwhile saves are held for a moment
//! so the network doesn't stay flagged.

use std::sync::Mutex;
use std::time::Instant;

/// The captcha page in a DataDome answer ({"url":"https://geo.captcha-
/// delivery.com/captcha/?..."}); only DataDome's own address is taken.
pub fn url_in(body: &str) -> Option<String> {
    const HOST: &str = "https://geo.captcha-delivery.com/captcha/";
    let url = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(str::to_string))
        .or_else(|| {
            // quoted inside some other text, maybe JSON-escaped
            let body = body.replace("\\/", "/").replace("\\u0026", "&");
            let start = body.find(HOST)?;
            let rest = &body[start..];
            let end = rest
                .find(|c: char| c == '"' || c == '\'' || c.is_whitespace())
                .unwrap_or(rest.len());
            Some(rest[..end].to_string())
        })?;
    url.starts_with(HOST).then_some(url)
}

/// DataDome's answer is a block, not a captcha (`t=bv`): its page only
/// says "You have been blocked", with nothing to solve.
pub fn is_block(url: &str) -> bool {
    url.split(['?', '&']).any(|p| p == "t=bv")
}

/// When SoundCloud last answered a save with DataDome's captcha.
static CHALLENGED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// How long saves wait after one: every refused request keeps the
/// network's flag fresh, so Wavify stops asking for a while instead.
const BACK_OFF: std::time::Duration = std::time::Duration::from_secs(5);

/// A save was answered with DataDome's captcha (or block).
pub fn note_challenge() {
    if let Ok(mut c) = CHALLENGED_AT.lock() {
        *c = Some(Instant::now());
    }
    crate::log!(
        "DataDome challenge noted: holding saves briefly ({}s)",
        BACK_OFF.as_secs()
    );
}

/// When a check was last passed.
static PASSED_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// The check was passed: saves go again at once.
pub fn note_passed() {
    if let Ok(mut c) = CHALLENGED_AT.lock() {
        *c = None;
    }
    if let Ok(mut p) = PASSED_AT.lock() {
        *p = Some(Instant::now());
    }
}

/// A check was passed in the last 30 s: a write refused again now isn't
/// shown another one (it would only repeat), it fails.
pub fn passed_recently() -> bool {
    PASSED_AT
        .lock()
        .ok()
        .and_then(|p| *p)
        .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(30))
}

/// Saves are on hold after a recent DataDome refusal.
pub fn backing_off() -> bool {
    CHALLENGED_AT
        .lock()
        .ok()
        .and_then(|c| *c)
        .is_some_and(|t| t.elapsed() < BACK_OFF)
}

#[cfg(test)]
mod tests {
    use super::url_in;

    #[test]
    fn captcha_url_from_datadome_answers() {
        let body = r#"{"url":"https://geo.captcha-delivery.com/captcha/?initialCid=AHrl&cid=p0h4&referer=https%3A%2F%2Fapi-v2.soundcloud.com&t=fe&b=1051735"}"#;
        assert_eq!(
            url_in(body).as_deref(),
            Some("https://geo.captcha-delivery.com/captcha/?initialCid=AHrl&cid=p0h4&referer=https%3A%2F%2Fapi-v2.soundcloud.com&t=fe&b=1051735")
        );
        let escaped =
            r#"blocked: {"url":"https:\/\/geo.captcha-delivery.com\/captcha\/?cid=x&t=fe"} (403)"#;
        assert_eq!(
            url_in(escaped).as_deref(),
            Some("https://geo.captcha-delivery.com/captcha/?cid=x&t=fe")
        );
        assert_eq!(url_in(r#"{"url":"https://evil.example/captcha/"}"#), None);
        assert_eq!(url_in("Forbidden"), None);
    }

    #[test]
    fn blocks_are_told_from_captchas() {
        assert!(super::is_block(
            "https://geo.captcha-delivery.com/captcha/?initialCid=a&t=bv&s=1"
        ));
        assert!(!super::is_block(
            "https://geo.captcha-delivery.com/captcha/?initialCid=a&t=fe&s=1"
        ));
    }
}
