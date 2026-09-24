use crate::config::*;
use anyhow::{bail, Result};
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

/// One token refresh at a time across every `Auth`: make_api/init_api each
/// build their own from token.json, and refresh tokens are single-use, so
/// parallel refreshes would all but one fail (and read as logged out).
static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn read_token(path: &std::path::Path) -> Option<Token> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub struct Auth {
    http: reqwest::Client,
    pub token: RwLock<Option<Token>>,
    token_path: std::path::PathBuf,
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn gen_pkce() -> Pkce {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rnd = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut rnd);
    let verifier: String = rnd
        .iter()
        .map(|b| CHARSET[(*b as usize) % CHARSET.len()] as char)
        .collect();
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    Pkce {
        verifier,
        challenge,
    }
}

pub fn b64_sha256(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(bytes))
}

#[derive(serde::Serialize, Deserialize, Debug, Clone)]
pub struct PendingAuth {
    pub verifier: String,
    pub started_ms: u64,
}

impl Auth {
    pub fn new(settings: &Settings) -> Result<Self> {
        let http = crate::api::build_http(settings)?;
        let token_path = config_dir().join("token.json");
        let token = read_token(&token_path);
        Ok(Auth {
            http,
            token: RwLock::new(token),
            token_path,
        })
    }

    fn basic_header() -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .encode(format!("{CLIENT_ID}:{CLIENT_SECRET}").as_bytes())
    }

    pub async fn exchange_code(&self, code: &str, verifier: &str) -> Result<Token> {
        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", verifier),
        ];
        let resp: OAuthTokenResponse = self
            .http
            .post(format!("{AUTH_API}/oauth/token"))
            .header("Accept-Charset", "UTF-8")
            .header("Authorization", format!("Basic {}", Self::basic_header()))
            .form(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let token = Token::from_oauth(&resp);
        self.store(token.clone()).await;
        Ok(token)
    }

    pub async fn refresh(&self) -> Result<()> {
        let refresh_token = {
            let guard = self.token.read().await;
            match guard.as_ref() {
                Some(t) if !t.refresh_token.is_empty() => t.refresh_token.clone(),
                _ => bail!("no refresh token"),
            }
        };
        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token.as_str()),
        ];
        let resp: OAuthTokenResponse = self
            .http
            .post(format!("{AUTH_API}/oauth/token"))
            .header("Accept-Charset", "UTF-8")
            .header("Authorization", format!("Basic {}", Self::basic_header()))
            .form(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut token = Token::from_oauth(&resp);
        // The server rotates the refresh token (the one just sent is spent),
        // so keep the old one only when the response carries none.
        if token.refresh_token.is_empty() {
            token.refresh_token = refresh_token;
        }
        self.store(token).await;
        Ok(())
    }

    async fn store(&self, token: Token) {
        if let Ok(j) = serde_json::to_string(&token) {
            // Write-then-rename, so a concurrent Auth::new never reads a
            // half-written token.json.
            let tmp = self.token_path.with_extension("json.tmp");
            let swapped =
                std::fs::write(&tmp, &j).and_then(|_| std::fs::rename(&tmp, &self.token_path));
            if swapped.is_err() {
                let _ = std::fs::remove_file(&tmp);
                let _ = std::fs::write(&self.token_path, j);
            }
        }
        *self.token.write().await = Some(token);
    }

    pub async fn access(&self) -> Result<String> {
        let (expired, has_refresh, token) = {
            let g = self.token.read().await;
            match g.as_ref() {
                Some(t) => (
                    t.expired(),
                    !t.refresh_token.is_empty(),
                    Some(t.access_token.clone()),
                ),
                None => (true, false, None),
            }
        };
        if token.is_none() {
            bail!("unauthenticated");
        }
        if expired && has_refresh {
            let _refresh = REFRESH_LOCK.lock().await;
            // Another task may have refreshed while this one waited: adopt
            // its token instead of spending the old refresh token again.
            if let Some(disk) = read_token(&self.token_path) {
                if token.as_deref() != Some(disk.access_token.as_str()) {
                    *self.token.write().await = Some(disk);
                }
            }
            let needs_refresh = self
                .token
                .read()
                .await
                .as_ref()
                .is_some_and(|t| t.expired() && !t.refresh_token.is_empty());
            if needs_refresh {
                self.refresh().await?;
            }
        }
        match self.token.read().await.as_ref() {
            Some(t) => Ok(t.access_token.clone()),
            None => bail!("unauthenticated"),
        }
    }

    pub async fn access_opt(&self) -> Option<String> {
        self.access().await.ok()
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

/// Extract `code` from a browser-invoked URL argument: sc://auth?code=...
pub fn parse_code_from_arg(arg: &str) -> Option<String> {
    let pos = arg.find("code=")?;
    let rest = &arg[pos + 5..];
    let code = rest.split('&').next()?.trim_matches('"').trim_matches('\'');
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

/// Headless exchange using pending verifier saved at login start.
pub async fn complete_login(code: String) -> Result<()> {
    let pending: PendingAuth =
        serde_json::from_str(&std::fs::read_to_string(auth_pending_path())?)?;
    let _ = std::fs::remove_file(auth_pending_path());
    let settings = Settings::load();
    let auth = Auth::new(&settings)?;
    auth.exchange_code(&code, &pending.verifier).await?;
    Ok(())
}

pub fn save_pending(verifier: &str) -> std::io::Result<()> {
    let p = PendingAuth {
        verifier: verifier.to_string(),
        started_ms: now_ms(),
    };
    std::fs::write(auth_pending_path(), serde_json::to_string(&p)?)
}
