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

use crate::api::{Api, StreamSource, Track};
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
/// A manually configured proxy may carry all SoundCloud traffic only after
/// an anonymous SoundCloud request has proven the route works. Recheck every
/// few minutes so a Tor exit or other proxy that went stale is not trusted.
const CONFIGURED_PROXY_CHECK_TTL: Duration = Duration::from_secs(5 * 60);

static VERIFIED_CONFIGURED_PROXY: std::sync::Mutex<Option<(String, Instant)>> =
    std::sync::Mutex::new(None);
static DIRECT_SOUNDCLOUD_CHECK: std::sync::Mutex<Option<(bool, Instant)>> =
    std::sync::Mutex::new(None);

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

/// Whether this exact configured proxy recently passed the SoundCloud probe.
/// Callers that build ordinary app clients must never route through an
/// unverified configured proxy.
pub fn configured_proxy_is_verified(proxy: &str) -> bool {
    VERIFIED_CONFIGURED_PROXY
        .lock()
        .ok()
        .and_then(|verified| {
            verified
                .as_ref()
                .filter(|(known, checked)| {
                    known == proxy && checked.elapsed() < CONFIGURED_PROXY_CHECK_TTL
                })
                .map(|_| ())
        })
        .is_some()
}

async fn probe_soundcloud(client: reqwest::Client) -> Result<()> {
    let response = client
        .get("https://soundcloud.com/robots.txt")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    let status = response.status();
    let host = response
        .url()
        .host_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !status.is_success() || !(host == "soundcloud.com" || host.ends_with(".soundcloud.com")) {
        bail!("SoundCloud probe returned HTTP {status} from {host}");
    }
    let _ = response.bytes().await?;
    Ok(())
}

/// Check the direct route once per TTL, without sending account data. This
/// lets startup explain that a proxy is needed when SoundCloud is blocked.
pub async fn soundcloud_reachable_directly() -> bool {
    if let Ok(check) = DIRECT_SOUNDCLOUD_CHECK.lock() {
        if let Some((reachable, checked)) = check.as_ref() {
            if checked.elapsed() < CONFIGURED_PROXY_CHECK_TTL {
                return *reachable;
            }
        }
    }
    let client = reqwest::Client::builder()
        .user_agent(crate::config::USER_AGENT)
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(10))
        .build();
    let reachable = match client {
        Ok(client) => probe_soundcloud(client).await.is_ok(),
        Err(_) => false,
    };
    if let Ok(mut check) = DIRECT_SOUNDCLOUD_CHECK.lock() {
        *check = Some((reachable, Instant::now()));
    }
    crate::log!("direct SoundCloud route reachable: {reachable}");
    reachable
}

/// Probe SoundCloud anonymously through the user's configured proxy before
/// allowing it to become the app's primary route. The final response must
/// still be hosted by SoundCloud, rejecting captive portals and block pages.
pub async fn verify_configured_proxy(proxy: &str) -> Result<()> {
    if configured_proxy_is_verified(proxy) {
        return Ok(());
    }

    probe_soundcloud(
        client_via(proxy)
            .map_err(|e| anyhow::anyhow!("configured proxy is invalid or unavailable: {e}"))?,
    )
    .await
    .map_err(|e| anyhow::anyhow!("proxy did not reach SoundCloud: {e}"))?;

    if let Ok(mut verified) = VERIFIED_CONFIGURED_PROXY.lock() {
        *verified = Some((proxy.to_string(), Instant::now()));
    }
    crate::log!("configured proxy passed the SoundCloud reachability check");
    Ok(())
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

async fn search_through_proxy(
    proxy: &str,
    query: &str,
    cid: &str,
    limit: usize,
) -> Result<Vec<Track>> {
    let api = Api::new(client_via(proxy)?, &Settings::default()).anonymous();
    let list = if let Some(tag) = query
        .strip_prefix('#')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        api.search_tracks_tagged(tag, cid, limit).await?
    } else {
        api.search_tracks(query, cid, limit).await?
    };
    Ok(list.collection)
}

/// Search anonymously through other regions as a supplement to the normal
/// results. SoundCloud can omit geo-restricted tracks from the local search
/// response entirely; no account token or cookies are sent through proxies.
pub async fn search_tracks_via_proxies(
    query: String,
    cid: String,
    known_ids: HashSet<i64>,
    limit: usize,
) -> Vec<Track> {
    let (good, mut candidates, stale) = {
        let mut guard = POOL.lock().await;
        let pool = guard.get_or_insert_with(Pool::default);
        pool.load_good();
        let stale =
            pool.fetched_at.map_or(true, |t| t.elapsed() > LIST_TTL) || pool.candidates.is_empty();
        let good = pool.good.clone();
        let candidates = if stale {
            Vec::new()
        } else {
            pool.candidates
                .iter()
                .filter(|p| !pool.bad.contains(*p) && !pool.good.contains(*p))
                .cloned()
                .collect()
        };
        (good, candidates, stale)
    };

    let query = query.as_str();
    let cid = cid.as_str();
    let known_ids = std::sync::Arc::new(known_ids);
    let find_novel = |tracks: Vec<Track>| {
        tracks
            .into_iter()
            .filter(|track| !known_ids.contains(&track.id))
            .collect::<Vec<_>>()
    };

    // Reuse bypasses already proven to reach playable SoundCloud regions.
    for proxy in good.into_iter().take(4) {
        match search_through_proxy(&proxy, query, cid, limit).await {
            Ok(tracks) => {
                let tracks = find_novel(tracks);
                if !tracks.is_empty() {
                    let mut guard = POOL.lock().await;
                    guard.get_or_insert_with(Pool::default).promote(&proxy);
                    return tracks;
                }
            }
            Err(e) => {
                crate::log!("bypass search: known proxy {proxy} failed: {e}");
                mark_dead(&proxy).await;
            }
        }
    }

    if stale {
        let skip = {
            let mut guard = POOL.lock().await;
            let pool = guard.get_or_insert_with(Pool::default);
            pool.load_good();
            pool.bad
                .iter()
                .chain(pool.good.iter())
                .cloned()
                .collect::<HashSet<_>>()
        };
        candidates = fetch_candidates(&skip).await;
        let mut guard = POOL.lock().await;
        let pool = guard.get_or_insert_with(Pool::default);
        pool.candidates = candidates.clone();
        pool.fetched_at = Some(Instant::now());
    }
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut queue = candidates.into_iter().take(48);
    let mut running = FuturesUnordered::new();
    let spawn = |proxy: String| {
        let q = query.to_string();
        let cid = cid.to_string();
        async move {
            let result = search_through_proxy(&proxy, &q, &cid, limit).await;
            (proxy, result)
        }
    };
    for proxy in queue.by_ref().take(PARALLEL) {
        running.push(spawn(proxy));
    }
    while let Some((proxy, result)) = running.next().await {
        match result {
            Ok(tracks) => {
                let tracks = find_novel(tracks);
                if !tracks.is_empty() {
                    let mut guard = POOL.lock().await;
                    guard.get_or_insert_with(Pool::default).promote(&proxy);
                    crate::log!(
                        "bypass search: found {} additional tracks via {proxy}",
                        tracks.len()
                    );
                    return tracks;
                }
            }
            Err(e) => {
                crate::log!("bypass search: proxy {proxy} failed: {e}");
                mark_dead(&proxy).await;
            }
        }
        if let Some(next) = queue.next() {
            running.push(spawn(next));
        }
    }
    Vec::new()
}
