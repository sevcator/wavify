//! Session-cookie auth: paste Cookie header (or oauth_token) from a real browser.

use crate::auth::gen_pkce;
use crate::config::*;
use anyhow::{bail, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CookieJar {
    pub raw: String,
}

pub fn normalize_cookie_header(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("Cookie:") {
        s = rest.trim();
    } else if let Some(rest) = s.strip_prefix("cookie:") {
        s = rest.trim();
    } else if let Some(rest) = s.strip_prefix("cookie") {
        s = rest.trim();
    }
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string()
}

/// Cookie-jar writes one at a time: two responses updating datadome at once
/// each read the jar, and the later write dropped the other's change.
static JAR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn save_cookies(raw: &str) -> Result<()> {
    let _jar = JAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    write_jar(raw)
}

/// Through a temp file renamed over the jar, so a reader never finds it half
/// written (an empty read was taken for "no jar", and the session replaced
/// by a lone datadome cookie).
fn write_jar(raw: &str) -> Result<()> {
    let jar = CookieJar {
        raw: normalize_cookie_header(raw),
    };
    let path = cookies_path();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(&jar)?)?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

pub fn load_cookies() -> Result<String> {
    let j = std::fs::read_to_string(cookies_path())?;
    let jar: CookieJar = serde_json::from_str(&j)?;
    let raw = normalize_cookie_header(&jar.raw);
    if raw.is_empty() {
        bail!("cookie jar is empty");
    }
    Ok(raw)
}

pub fn update_datadome_cookie(set_cookie_header: &str) {
    let Some(pos) = set_cookie_header.find("datadome=") else {
        return;
    };
    let rest = &set_cookie_header[pos + 9..];
    let val = rest.split(';').next().unwrap_or_default().trim();
    if val.is_empty() {
        return;
    }
    let _jar = JAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut raw = match std::fs::read_to_string(cookies_path()) {
        Ok(j) => match serde_json::from_str::<CookieJar>(&j) {
            Ok(jar) => normalize_cookie_header(&jar.raw),
            // unreadable: left for the next sign-in to replace, not taken
            // for a missing jar and overwritten
            Err(_) => return,
        },
        // No jar (token-only or WebView login): start one. The api
        // prepends oauth_token when the jar lacks it.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return,
    };
    if let Some(old_pos) = raw.find("datadome=") {
        let old_end = raw[old_pos..]
            .find(';')
            .map(|p| old_pos + p)
            .unwrap_or(raw.len());
        raw.replace_range(old_pos..old_end, &format!("datadome={val}"));
    } else if raw.is_empty() {
        raw = format!("datadome={val}");
    } else {
        raw.push_str(&format!("; datadome={val}"));
    }
    let _ = write_jar(&raw);
}

#[derive(Deserialize, Debug)]
struct AuthorizeResponse {
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

pub fn open_social_login() -> String {
    let url = "https://soundcloud.com/signin".to_string();
    let _ = open::that(&url);
    url
}

fn cookie_value(raw: &str, name: &str) -> Option<String> {
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k.trim().eq_ignore_ascii_case(name) {
                let v = v.trim();
                if !v.is_empty() && !v.eq_ignore_ascii_case("deleted") {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn csrf_from_cookies(raw: &str) -> String {
    for name in [
        "oauth_token",
        "csrf_token",
        "XSRF-TOKEN",
        "sc_a_cookie",
        "_csrf",
    ] {
        if let Some(v) = cookie_value(raw, name) {
            return v;
        }
    }
    String::new()
}

fn basic_auth_header() -> String {
    base64::engine::general_purpose::STANDARD
        .encode(format!("{CLIENT_ID}:{CLIENT_SECRET}").as_bytes())
}

fn browser_http() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
        )
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

pub fn save_access_token(
    access: &str,
    refresh: Option<String>,
    expires_in: Option<u64>,
) -> Result<()> {
    let token = Token {
        access_token: access.to_string(),
        refresh_token: refresh.unwrap_or_default(),
        scope: "non-expiring".into(),
        expires_at_ms: now_ms() + expires_in.unwrap_or(365 * 24 * 3600) as u64 * 1000,
    };
    std::fs::write(token_path(), serde_json::to_string(&token)?)?;
    Ok(())
}

/// Verify token works against /me (mobile envelope or flat).
fn verify_token(access: &str) -> Result<String> {
    let http = browser_http()?;
    let tries = [
        format!("{MOBILE_API}/me?treating=1"),
        format!("{V2_API}/me?client_id={CLIENT_ID}"),
    ];
    let mut last = String::new();
    for url in tries {
        let resp = http
            .get(&url)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION)
            .send();
        match resp {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json()?;
                let name = v
                    .get("user")
                    .and_then(|u| u.get("username"))
                    .or_else(|| v.get("username"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("ok")
                    .to_string();
                return Ok(name);
            }
            Ok(r) => {
                last = format!(
                    "{} {}",
                    r.status(),
                    r.text()
                        .unwrap_or_default()
                        .chars()
                        .take(120)
                        .collect::<String>()
                )
            }
            Err(e) => last = e.to_string(),
        }
    }
    bail!("token rejected: {last}")
}

/// A token issued to the Android client for the browser session in the
/// jar: OAuth authorize with the session cookies -> code -> token.
///
/// This is the token to hold. The browser's own `oauth_token` cookie belongs
/// to the web client: the mobile API refuses it for writes (401), and the web
/// API that accepts it sits behind DataDome, which blocks writes from outside
/// a browser. Returns (access, refresh, expires_in).
pub fn android_token_from_cookies(cookies: &str) -> Result<(String, Option<String>, Option<u64>)> {
    let pkce = gen_pkce();
    let csrf = csrf_from_cookies(cookies);
    let http = browser_http()?;

    let payload = serde_json::json!({
        "client_id": CLIENT_ID,
        "redirect_uri": REDIRECT_URI,
        "response_type": "code",
        "scope": "non-expiring",
        "code_challenge": pkce.challenge,
        "code_challenge_method": "S256",
    });

    let mut req = http
        .post(format!("{AUTH_API}/oauth/authorize?client_id={CLIENT_ID}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Cookie", cookies)
        .header("Origin", "https://soundcloud.com")
        .header("Referer", "https://soundcloud.com/");
    if !csrf.is_empty() {
        req = req.header("X-Csrf-Token", &csrf);
    }
    let raw_resp = req.json(&payload).send()?;
    let status = raw_resp.status();
    let body = raw_resp.text()?;
    if !status.is_success() {
        bail!(
            "authorize HTTP {status}: {}",
            body.chars().take(300).collect::<String>()
        );
    }
    let resp: AuthorizeResponse = serde_json::from_str(&body).map_err(|e| {
        anyhow::anyhow!(
            "authorize json: {e}; body={}",
            body.chars().take(200).collect::<String>()
        )
    })?;

    let redirect = match resp.redirect_uri {
        Some(r) => r,
        None => bail!(
            "authorize failed (need valid session Cookie with oauth_token). state={:?} error={:?}",
            resp.state,
            resp.error
        ),
    };
    let code = extract_code(&redirect)
        .ok_or_else(|| anyhow::anyhow!("no code in redirect: {redirect}"))?;
    token_for_code(&code, &pkce.verifier)
}

/// The Android-client token for an authorization code issued to
/// REDIRECT_URI (see android_token_from_cookies and the login window).
/// Returns (access, refresh, expires_in).
pub fn token_for_code(code: &str, verifier: &str) -> Result<(String, Option<String>, Option<u64>)> {
    let http = browser_http()?;
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", verifier),
    ];
    let tok: OAuthTokenResponse = http
        .post(format!("{AUTH_API}/oauth/token"))
        .header("Accept", "application/json")
        .header("Accept-Charset", "UTF-8")
        .header("Authorization", format!("Basic {}", basic_auth_header()))
        .form(&params)
        .send()?
        .error_for_status()?
        .json()?;

    if tok.access_token.is_empty() {
        bail!("token response missing access_token");
    }
    Ok((tok.access_token, tok.refresh_token, tok.expires_in))
}

/// Sign in from the cookie jar: an Android-client token first (it can
/// write), else the browser's `oauth_token` as it is (read-only in practice).
pub fn authorize_with_cookies() -> Result<()> {
    let cookies = load_cookies()?;
    let exchange_err = match android_token_from_cookies(&cookies) {
        Ok((access, refresh, expires_in)) => {
            save_access_token(&access, refresh, expires_in)?;
            let _ = verify_token(&access);
            return Ok(());
        }
        Err(e) => e,
    };
    if let Some(tok) = cookie_value(&cookies, "oauth_token") {
        if verify_token(&tok).is_ok() {
            crate::log!(
                "cookie login: android token exchange failed ({exchange_err}); using the web token"
            );
            save_access_token(&tok, None, Some(365 * 24 * 3600))?;
            return Ok(());
        }
    }
    Err(exchange_err)
}

/// Import raw paste: full Cookie header OR bare oauth_token value.
pub fn authorize_from_paste(raw: &str) -> Result<()> {
    let raw = normalize_cookie_header(raw);
    if raw.is_empty() {
        bail!("empty paste");
    }
    // bare token (no semicolons / cookie pairs)
    if !raw.contains('=') && !raw.contains(';') && raw.len() > 16 {
        verify_token(&raw)?;
        save_access_token(&raw, None, Some(365 * 24 * 3600))?;
        return Ok(());
    }
    save_cookies(&raw)?;
    authorize_with_cookies()
}

pub fn extract_code(redirect_uri: &str) -> Option<String> {
    let pos = redirect_uri.find("code=")?;
    let rest = &redirect_uri[pos + 5..];
    let code = rest.split('&').next()?.split('#').next()?;
    let code = percent_decode(code);
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = |c: u8| -> Option<u8> {
                match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    b'A'..=b'F' => Some(c - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(a), Some(b)) = (h(bytes[i + 1]), h(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
