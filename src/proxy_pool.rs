//! "Bypass unavailability": a track SoundCloud blocks in the user's country
//! is resolved (and then streamed) through a public proxy in another one.
//!
//! Public proxy lists are fetched from a few well-known sources, and the
//! candidates are tried against the actual track: the first proxy through
//! which SoundCloud hands out its stream wins. Proxies that worked are kept
//! (also on disk) and tried first next time; one that fails or dies is
//! dropped and the next one takes over.
//!
//! Public proxies are run by strangers, so nothing personal goes through
//! them: requests via a proxy carry only the app's client id, never the
//! OAuth token or the session cookies.

use crate::api::{Api, StreamSource};
use crate::config::{config_dir, Settings, CLIENT_ID};
use anyhow::{bail, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Proxy list sources: plain "host:port" lines, with the scheme to use.
const SOURCES: &[(&str, &str)] = &[
    (
        "https://api.proxyscrape.com/v4/free-proxy-list/get?request=display_proxies&protocol=http&proxy_format=ipport&format=text&timeout=6000",
        "http",
    ),
    (
        "https://api.proxyscrape.com/v4/free-proxy-list/get?request=display_proxies&protocol=socks5&proxy_format=ipport&format=text&timeout=6000",
        "socks5",
    ),
    ("https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/http.txt", "http"),
    ("https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/socks5.txt", "socks5"),
    ("https://raw.githubusercontent.com/monosans/proxy-list/main/proxies/http.txt", "http"),
    ("https://raw.githubusercontent.com/monosans/proxy-list/main/proxies/socks5.txt", "socks5"),
];

/// Candidates tried at once, and at most per bypass (public lists are mostly
/// dead entries, so it takes a wide net).
const PARALLEL: usize = 48;
const MAX_TRIES: usize = 600;
/// Lists are refetched when older than this.
const LIST_TTL: Duration = Duration::from_secs(15 * 60);
/// Proxies remembered as working.
const KEEP_GOOD: usize = 16;

#[derive(Default)]
struct Pool {
    /// Known-good proxy URLs, best first.
    good: Vec<String>,
    /// Failed this run: never tried again.
    bad: HashSet<String>,
    candidates: Vec<String>,
    fetched_at: Option<Instant>,
    loaded: bool,
}

static POOL: tokio::sync::Mutex<Option<Pool>> = tokio::sync::Mutex::const_new(None);

fn good_path() -> std::path::PathBuf {
    config_dir().join("bypass_proxies.json")
}

impl Pool {
    fn load_good(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Ok(j) = std::fs::read_to_string(good_path()) {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&j) {
                self.good = list;
            }
        }
    }

    fn save_good(&self) {
        if let Ok(j) = serde_json::to_string(&self.good) {
            let _ = std::fs::write(good_path(), j);
        }
    }

    fn promote(&mut self, proxy: &str) {
        self.good.retain(|p| p != proxy);
        self.good.insert(0, proxy.to_string());
        self.good.truncate(KEEP_GOOD);
        self.save_good();
    }

    fn drop_proxy(&mut self, proxy: &str) {
        self.bad.insert(proxy.to_string());
        if self.good.iter().any(|p| p == proxy) {
            self.good.retain(|p| p != proxy);
            self.save_good();
        }
    }
}

/// A proxy that stopped working mid-stream: never use it again this run.
pub async fn mark_dead(proxy: &str) {
    let mut guard = POOL.lock().await;
    guard.get_or_insert_with(Pool::default).drop_proxy(proxy);
}

/// An HTTP client that goes through `proxy`, with short timeouts (a slow
/// public proxy is as good as a dead one).
pub fn client_via(proxy: &str) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy)?)
        .user_agent(crate::config::USER_AGENT)
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(15))
        .build()?)
}

/// The track's stream as seen through `proxy`, anonymously.
async fn resolve_through(proxy: &str, track_id: i64, quality: &str) -> Result<StreamSource> {
    let http = client_via(proxy)?;
    let api = Api::new(http, &Settings::default()).anonymous();
    api.resolve_stream(track_id, CLIENT_ID, quality).await
}

/// Fresh candidates from every list source (fetched over the user's normal
/// connection), shuffled, without the ones already known.
async fn fetch_candidates(skip: &HashSet<String>) -> Vec<String> {
    use rand::seq::SliceRandom;
    let Ok(http) = crate::api::build_http(&Settings::load()) else {
        return Vec::new();
    };
    let lists = futures_util::future::join_all(SOURCES.iter().map(|(url, scheme)| {
        let http = http.clone();
        async move {
            let body = match http.get(*url).timeout(Duration::from_secs(15)).send().await {
                Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
                _ => String::new(),
            };
            body.lines()
                .filter_map(|l| {
                    let l = l.trim();
                    let hostport = l.rsplit("://").next()?;
                    let (host, port) = hostport.rsplit_once(':')?;
                    port.parse::<u16>().ok()?;
                    (!host.is_empty()).then(|| format!("{scheme}://{host}:{port}"))
                })
                .collect::<Vec<_>>()
        }
    }))
    .await;
    let mut seen = HashSet::new();
    let mut all: Vec<String> = lists
        .into_iter()
        .flatten()
        .filter(|p| !skip.contains(p) && seen.insert(p.clone()))
        .collect();
    all.shuffle(&mut rand::thread_rng());
    crate::log!("bypass: {} proxy candidates", all.len());
    all
}

/// Resolve a track that is unavailable here through a public proxy.
/// Returns its stream and the proxy it came through: the audio has to be
/// fetched through the same one.
pub async fn resolve_via_proxies(track_id: i64, quality: &str) -> Result<(StreamSource, String)> {
    // 1. proxies that worked before
    let good = {
        let mut guard = POOL.lock().await;
        let pool = guard.get_or_insert_with(Pool::default);
        pool.load_good();
        pool.good.clone()
    };
    for proxy in good {
        match resolve_through(&proxy, track_id, quality).await {
            Ok(src) => {
                crate::log!("bypass: track {track_id} via {proxy} (known good)");
                let mut guard = POOL.lock().await;
                guard.get_or_insert_with(Pool::default).promote(&proxy);
                return Ok((src, proxy));
            }
            Err(e) => {
                crate::log!("bypass: known proxy {proxy} failed: {e}");
                mark_dead(&proxy).await;
            }
        }
    }

    // 2. fresh candidates from the public lists, many at a time
    let candidates = {
        let mut guard = POOL.lock().await;
        let pool = guard.get_or_insert_with(Pool::default);
        let stale = pool.fetched_at.map_or(true, |t| t.elapsed() > LIST_TTL);
        if stale || pool.candidates.is_empty() {
            let skip: HashSet<String> = pool.bad.iter().chain(pool.good.iter()).cloned().collect();
            drop(guard);
            let fresh = fetch_candidates(&skip).await;
            let mut guard = POOL.lock().await;
            let pool = guard.get_or_insert_with(Pool::default);
            pool.candidates = fresh;
            pool.fetched_at = Some(Instant::now());
            pool.candidates.clone()
        } else {
            let bad = pool.bad.clone();
            pool.candidates.retain(|p| !bad.contains(p));
            pool.candidates.clone()
        }
    };
    if candidates.is_empty() {
        bail!("no public proxies could be fetched");
    }

    let quality = quality.to_string();
    let mut queue = candidates.into_iter().take(MAX_TRIES);
    let mut running = FuturesUnordered::new();
    let spawn = |proxy: String| {
        let q = quality.clone();
        async move {
            let r = resolve_through(&proxy, track_id, &q).await;
            (proxy, r)
        }
    };
    for p in queue.by_ref().take(PARALLEL) {
        running.push(spawn(p));
    }
    let mut tried = 0usize;
    while let Some((proxy, result)) = running.next().await {
        tried += 1;
        match result {
            Ok(src) => {
                crate::log!("bypass: track {track_id} via {proxy} ({tried} tried)");
                let mut guard = POOL.lock().await;
                guard.get_or_insert_with(Pool::default).promote(&proxy);
                return Ok((src, proxy));
            }
            Err(_) => {
                let mut guard = POOL.lock().await;
                guard.get_or_insert_with(Pool::default).drop_proxy(&proxy);
            }
        }
        if let Some(next) = queue.next() {
            running.push(spawn(next));
        }
    }
    bail!("no working proxy found ({tried} tried)")
}
