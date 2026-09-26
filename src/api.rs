use crate::config::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The HTTP client for SoundCloud, one per proxy setting: its clones share
/// one connection pool, so requests reuse connections instead of paying a
/// new TLS handshake each (a client used to be built per request).
pub fn build_http(settings: &Settings) -> Result<reqwest::Client> {
    static CLIENT: std::sync::Mutex<Option<(String, reqwest::Client)>> =
        std::sync::Mutex::new(None);
    let configured_proxy = settings.proxy.as_deref().unwrap_or_default();
    let proxy = if !configured_proxy.is_empty()
        && crate::proxy_pool::configured_proxy_is_verified(configured_proxy)
    {
        configured_proxy.to_string()
    } else {
        String::new()
    };
    if let Ok(guard) = CLIENT.lock() {
        if let Some((_, client)) = guard.as_ref().filter(|(key, _)| *key == proxy) {
            return Ok(client.clone());
        }
    }
    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90));
    if !proxy.is_empty() {
        builder = builder.proxy(reqwest::Proxy::all(&proxy)?);
    }
    let client = builder.build()?;
    if let Ok(mut guard) = CLIENT.lock() {
        *guard = Some((proxy, client.clone()));
    }
    Ok(client)
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ApiList<T> {
    #[serde(default)]
    pub collection: Vec<T>,
    #[serde(default)]
    pub next_href: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct UserMini {
    pub id: i64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default)]
    pub urn: Option<String>,
    #[serde(default)]
    pub track_count: Option<u64>,
    #[serde(default)]
    pub followers_count: Option<u64>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Transcoding {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub format: Option<TranscodingFormat>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub quality: Option<String>,
    /// Only a preview (Go+ track without a subscription).
    #[serde(default)]
    pub snipped: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TranscodingFormat {
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Media {
    #[serde(default)]
    pub transcodings: Vec<Transcoding>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Track {
    pub id: i64,
    #[serde(default)]
    pub media: Option<Media>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub user: Option<UserMini>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub full_duration: Option<u64>,
    #[serde(default)]
    pub waveform_url: Option<String>,
    #[serde(default)]
    pub comment_count: Option<u64>,
    #[serde(default)]
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub playback_count: Option<u64>,
    #[serde(default)]
    pub likes_count: Option<u64>,
    #[serde(default)]
    pub streamable: Option<bool>,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub urn: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub tag_list: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub reposts_count: Option<u64>,
    /// Label data of released music: "artist" is the real artist name when
    /// the uploader is an account like "coldplayofficial".
    #[serde(default)]
    pub publisher_metadata: Option<PublisherMetadata>,
    /// SoundCloud's "visuals": the banner an artist can set behind the
    /// waveform. `None` when the response didn't say (older caches, other
    /// APIs), `Some(None)` for a track without one (the API sends `null`).
    #[serde(
        default,
        deserialize_with = "deserialize_visuals",
        skip_serializing_if = "Option::is_none"
    )]
    pub visuals: Option<Option<Visuals>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct PublisherMetadata {
    #[serde(default)]
    pub artist: Option<String>,
}

/// `{"enabled": true, "visuals": [{"visual_url": "…-original.jpg", …}]}`
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Visuals {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub visuals: Option<Vec<Visual>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Visual {
    #[serde(default)]
    pub visual_url: Option<String>,
}

/// A present `visuals` key, whatever it holds: an unexpected shape reads as
/// "no visual" instead of failing the whole track.
fn deserialize_visuals<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Option<Visuals>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(Some(serde_json::from_value(v).ok()))
}

impl Track {
    /// The banner to draw behind the waveform: `None` when unknown (fetch
    /// the track to find out), `Some(None)` when it has none.
    pub fn visual_url(&self) -> Option<Option<&str>> {
        let visuals = self.visuals.as_ref()?;
        Some(
            visuals
                .as_ref()
                .filter(|v| v.enabled != Some(false))
                .and_then(|v| v.visuals.as_deref())
                .and_then(|list| {
                    list.iter()
                        .find_map(|x| x.visual_url.as_deref().filter(|u| !u.is_empty()))
                }),
        )
    }

    pub fn artwork_or_avatar(&self) -> Option<&str> {
        self.artwork_url
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.user
                    .as_ref()
                    .and_then(|u| u.avatar_url.as_deref())
                    .filter(|s| !s.is_empty())
            })
    }

    pub fn display_artist_and_title(&self, prefer_from_metadata: bool) -> (String, String) {
        let uploader = self
            .user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("");
        let artist = if prefer_from_metadata {
            self.publisher_metadata
                .as_ref()
                .and_then(|metadata| metadata.artist.as_deref())
                .map(str::trim)
                .filter(|artist| !artist.is_empty())
                .unwrap_or_else(|| uploader.trim())
        } else {
            uploader.trim()
        };
        (
            if artist.is_empty() { "Unknown" } else { artist }.to_string(),
            self.title.trim().to_string(),
        )
    }
}

/// Extracts artist and track title based on user preference.
/// When `prefer_from_name` is true, parses patterns like `Artist - Title` or `Title - Artist`
/// checking for two spaces around the hyphen (" - ").
pub fn parse_artist_and_title(
    raw_title: &str,
    uploader: &str,
    uploader_permalink: &str,
    prefer_from_name: bool,
) -> (String, String) {
    let fallback_artist = if !uploader.trim().is_empty() {
        uploader.trim().to_string()
    } else {
        "Unknown".to_string()
    };
    let fallback_title = raw_title.trim().to_string();

    if !prefer_from_name {
        return (fallback_artist, fallback_title);
    }

    // Check that there is a hyphen surrounded by spaces (" - ")
    if !raw_title.contains(" - ") {
        return (fallback_artist, fallback_title);
    }

    let parts: Vec<&str> = raw_title
        .split(" - ")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.len() < 2 {
        return (fallback_artist, fallback_title);
    }

    let first = parts[0];
    let last = parts[parts.len() - 1];

    let u_clean = uploader.trim().to_lowercase();
    let p_clean = uploader_permalink.trim().to_lowercase();
    let first_clean = first.to_lowercase();
    let last_clean = last.to_lowercase();

    let matches_user = |segment_clean: &str| -> bool {
        if segment_clean.is_empty() {
            return false;
        }
        if !u_clean.is_empty()
            && (segment_clean == u_clean
                || (segment_clean.len() >= 3 && u_clean.contains(segment_clean))
                || (u_clean.len() >= 3 && segment_clean.contains(&u_clean)))
        {
            return true;
        }
        if !p_clean.is_empty()
            && (segment_clean == p_clean
                || (segment_clean.len() >= 3 && p_clean.contains(segment_clean))
                || (p_clean.len() >= 3 && segment_clean.contains(&p_clean)))
        {
            return true;
        }
        false
    };

    let first_matches = matches_user(&first_clean);
    let last_matches = matches_user(&last_clean);

    if last_matches && !first_matches {
        // "Title - Artist" format, e.g. "кроме тебя - angelgrind"
        let artist = last.to_string();
        let title = parts[..parts.len() - 1].join(" - ");
        (artist, title)
    } else if first_matches && !last_matches {
        // "Artist - Title" format, e.g. "angelgrind - кроме тебя"
        let artist = first.to_string();
        let title = parts[1..].join(" - ");
        (artist, title)
    } else {
        // Neither matches uploader (third party channel / reupload) or both match.
        // Standard music convention is "Artist - Title".
        let artist = first.to_string();
        let title = parts[1..].join(" - ");
        (artist, title)
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Comment {
    #[serde(default, deserialize_with = "deserialize_id_lenient")]
    pub id: i64,
    #[serde(default)]
    pub body: String,
    /// ms from track start; can be negative for pre-track comments
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub user: Option<UserMini>,
}

impl Comment {
    pub fn author(&self) -> &str {
        self.user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("unknown")
    }
}

fn deserialize_id_lenient<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0)),
        serde_json::Value::String(s) => Ok(s.parse::<i64>().unwrap_or(0)),
        _ => Ok(0),
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Playlist {
    #[serde(default, deserialize_with = "deserialize_id_lenient")]
    pub id: i64,
    #[serde(default)]
    pub urn: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub calculated_artwork_url: Option<String>,
    #[serde(default)]
    pub user: Option<UserMini>,
    #[serde(default)]
    pub track_count: Option<u64>,
    #[serde(default)]
    pub is_album: Option<bool>,
    #[serde(default)]
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub tracks: Option<Vec<Track>>,
}

impl Playlist {
    pub fn id_or_urn(&self) -> String {
        if let Some(urn) = &self.urn {
            if !urn.is_empty() {
                return urn.clone();
            }
        }
        if self.id != 0 {
            return self.id.to_string();
        }
        String::new()
    }

    pub fn artwork(&self) -> Option<&str> {
        self.artwork_url
            .as_deref()
            .or(self.calculated_artwork_url.as_deref())
    }

    pub fn artwork_or_avatar(&self) -> Option<&str> {
        self.artwork()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.tracks
                    .as_ref()
                    .and_then(|ts| ts.first())
                    .and_then(|t| t.artwork_or_avatar())
            })
            .or_else(|| {
                self.user
                    .as_ref()
                    .and_then(|u| u.avatar_url.as_deref())
                    .filter(|s| !s.is_empty())
            })
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedEntity {
    Track(Track),
    User(UserMini),
    Playlist(Playlist),
    Unknown(serde_json::Value),
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct WebProfile {
    #[serde(default, deserialize_with = "deserialize_id_lenient")]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub service: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Me {
    pub id: i64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub urn: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TranscodingUrl {
    pub url: String,
}

/// What the player should play: either one direct URL (Range-streamable)
/// or an ordered list of HLS chunks (optionally with an fMP4 init segment).
#[derive(Clone, Debug)]
pub enum StreamSource {
    Single(String),
    Chunks {
        init: Option<String>,
        chunks: Vec<String>,
    },
}

impl StreamSource {
    pub fn describe(&self) -> String {
        match self {
            StreamSource::Single(u) => format!("single {}", &u[..u.len().min(60)]),
            StreamSource::Chunks { init, chunks } => {
                format!("chunks={} init={}", chunks.len(), init.is_some())
            }
        }
    }
}

/// Like collection item: `{ "track": { ... } }` or bare track.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct LikeItem {
    #[serde(default)]
    pub track: Option<Track>,
    #[serde(flatten)]
    pub bare: Option<TrackFlat>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TrackFlat {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
}

impl LikeItem {
    pub fn into_track(self) -> Option<Track> {
        if let Some(t) = self.track {
            if t.id != 0 {
                return Some(t);
            }
        }
        None
    }
}

fn parse_user_reposts_page(value: serde_json::Value) -> ApiList<Track> {
    let mut seen = std::collections::HashSet::new();
    let collection = value
        .get("collection")
        .and_then(|items| items.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let track = item
                .get("track")
                .or_else(|| item.get("playlist").is_none().then_some(item))?;
            serde_json::from_value::<Track>(track.clone()).ok()
        })
        .filter(|track| track.id != 0 && !track.title.is_empty() && seen.insert(track.id))
        .collect();
    let next_href = value
        .get("next_href")
        .and_then(|href| href.as_str())
        .filter(|href| !href.is_empty())
        .map(str::to_string);
    ApiList {
        collection,
        next_href,
    }
}

pub struct Api {
    http: reqwest::Client,
    pub base_v2: String,
    pub base_mobile: String,
    access: Option<String>,
    use_cookies: bool,
    bridge: Option<std::sync::Arc<crate::sc_web::ScBridge>>,
}

impl Api {
    pub fn new(http: reqwest::Client, _settings: &Settings) -> Api {
        Api {
            http,
            base_v2: V2_API.to_string(),
            base_mobile: MOBILE_API.to_string(),
            access: None,
            use_cookies: true,
            bridge: None,
        }
    }

    /// No OAuth token and no session cookies: for requests that go through
    /// a stranger's machine (a public proxy, see proxy_pool).
    pub fn anonymous(mut self) -> Self {
        self.access = None;
        self.use_cookies = false;
        self
    }

    pub fn with_access(mut self, access: Option<String>) -> Self {
        self.access = access.filter(|s| !s.is_empty());
        self
    }

    pub fn with_bridge(mut self, bridge: Option<std::sync::Arc<crate::sc_web::ScBridge>>) -> Self {
        self.bridge = bridge;
        self
    }

    /// Load the browser session-cookies (if the user authorized via cookie paste)
    /// — required for DataDome-passing write requests.
    fn cookie_header(&self) -> Option<String> {
        if !self.use_cookies {
            return None;
        }
        let raw = crate::cookie_auth::load_cookies().ok().unwrap_or_default();
        if let Some(tok) = &self.access {
            if !raw.contains("oauth_token=") {
                if raw.is_empty() {
                    return Some(format!("oauth_token={tok}"));
                } else {
                    return Some(format!("oauth_token={tok}; {raw}"));
                }
            }
        }
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    }

    fn locale(&self) -> String {
        settings_region().unwrap_or_else(|| "en".to_string())
    }

    fn v2_url(&self, path_and_query: &str, cid: &str) -> String {
        let sep = if path_and_query.contains('?') {
            '&'
        } else {
            '?'
        };
        format!(
            "{}{}{}client_id={}&app_version={}&app_locale={}",
            self.base_v2,
            path_and_query,
            sep,
            cid,
            APP_VERSION,
            self.locale()
        )
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        crate::console::http_request("GET", url, None);
        let mut req = self
            .http
            .get(url)
            .header("Accept", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION);
        if let Some(tok) = &self.access {
            req = req.header("Authorization", format!("OAuth {tok}"));
        }
        if let Some(ck) = self.cookie_header() {
            req = req.header("Cookie", ck);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let bytes = resp.bytes().await?;
        crate::console::http_response("GET", &response_url, status, &bytes);
        crate::dlog!("HTTP GET {} -> {status}", crate::console::redact(url));
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            crate::dlog!("  body: {}", body.chars().take(600).collect::<String>());
            anyhow::bail!(
                "HTTP {} for {} — {}",
                status,
                url,
                body.chars().take(180).collect::<String>()
            );
        }
        match serde_json::from_slice(&bytes) {
            Ok(v) => Ok(v),
            Err(e) => {
                crate::log!("json decode FAILED for {}: {e}", &url[..url.len().min(120)]);
                Err(e.into())
            }
        }
    }

    pub async fn track(&self, id: i64, cid: &str) -> Result<Track> {
        let u = self.v2_url(&format!("/tracks/{id}"), cid);
        self.get_json(&u).await
    }

    pub async fn search_tracks(&self, q: &str, cid: &str, limit: usize) -> Result<ApiList<Track>> {
        let u = self.v2_url(
            &format!("/search/tracks?q={}&limit={}", urlencoding_lite(q), limit),
            cid,
        );
        self.get_json(&u).await
    }

    /// Tracks with a genre or tag, as soundcloud.com's tag pages list them
    /// ("Hip-Hop & Rap", "digicore").
    pub async fn search_tracks_tagged(
        &self,
        tag: &str,
        cid: &str,
        limit: usize,
    ) -> Result<ApiList<Track>> {
        let u = self.v2_url(
            &format!(
                "/search/tracks?q=*&filter.genre_or_tag={}&limit={}",
                // the filter matches lowercase only ("Hip-Hop & Rap" finds nothing)
                urlencoding_lite(&tag.to_lowercase()),
                limit
            ),
            cid,
        );
        self.get_json(&u).await
    }

    pub async fn search_users(
        &self,
        q: &str,
        cid: &str,
        limit: usize,
    ) -> Result<ApiList<UserMini>> {
        let u = self.v2_url(
            &format!("/search/users?q={}&limit={}", urlencoding_lite(q), limit),
            cid,
        );
        self.get_json(&u).await
    }

    pub async fn search_playlists(
        &self,
        q: &str,
        cid: &str,
        limit: usize,
    ) -> Result<ApiList<Playlist>> {
        let u = self.v2_url(
            &format!(
                "/search/playlists?q={}&limit={}",
                urlencoding_lite(q),
                limit
            ),
            cid,
        );
        self.get_json(&u).await
    }

    pub async fn user_likes(&self, user_id: i64, cid: &str, limit: usize) -> Result<Vec<Track>> {
        // Working endpoints: /users/{id}/likes and /users/{id}/track_likes
        // (/users/{id}/likes/tracks is 404 on current API)
        for path in [
            format!("/users/{user_id}/likes?limit={limit}&linked_partitioning=1"),
            format!("/users/{user_id}/track_likes?limit={limit}&linked_partitioning=1"),
        ] {
            let u = self.v2_url(&path, cid);
            if let Ok(list) = self.get_json::<ApiList<LikeItem>>(&u).await {
                let tracks: Vec<Track> = list
                    .collection
                    .into_iter()
                    .filter_map(|i| i.into_track())
                    .filter(|t| t.id != 0)
                    .collect();
                if !tracks.is_empty() {
                    return Ok(tracks);
                }
            }
        }
        Ok(vec![])
    }

    /// One page of a user's liked tracks, preserving the server cursor for
    /// demand-driven pagination in profile views.
    pub async fn user_liked_tracks_page(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<ApiList<Track>> {
        let u = self.v2_url(
            &format!("/users/{user_id}/likes?limit={limit}&linked_partitioning=1"),
            cid,
        );
        let page: ApiList<LikeItem> = self.get_json(&u).await?;
        Ok(ApiList {
            collection: page
                .collection
                .into_iter()
                .filter_map(|item| item.into_track())
                .filter(|track| track.id != 0)
                .collect(),
            next_href: page.next_href.filter(|href| !href.is_empty()),
        })
    }

    pub async fn user_liked_tracks_next(
        &self,
        next_href: &str,
        cid: &str,
    ) -> Result<ApiList<Track>> {
        let url = if next_href.contains("client_id=") {
            next_href.to_string()
        } else {
            format!(
                "{next_href}{}client_id={cid}",
                if next_href.contains('?') { '&' } else { '?' }
            )
        };
        let page: ApiList<LikeItem> = self.get_json(&url).await?;
        Ok(ApiList {
            collection: page
                .collection
                .into_iter()
                .filter_map(|item| item.into_track())
                .filter(|track| track.id != 0)
                .collect(),
            next_href: page.next_href.filter(|href| !href.is_empty()),
        })
    }

    /// Hydrate stub track objects (id-only) returned by mixed-selections.
    pub async fn tracks_by_ids(&self, ids: &[i64], cid: &str) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut out = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(50) {
            let joined = chunk
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let u = self.v2_url(&format!("/tracks?ids={joined}"), cid);
            // API returns a bare JSON array
            let batch: Vec<Track> = self.get_json(&u).await.unwrap_or_default();
            out.extend(
                batch
                    .into_iter()
                    .filter(|t| t.id != 0 && !t.title.is_empty()),
            );
        }
        Ok(out)
    }

    pub async fn playlist(&self, id: i64, cid: &str) -> Result<Playlist> {
        self.fetch_playlist_any(&id.to_string(), cid).await
    }

    pub async fn fetch_playlist_any(&self, id_or_urn: &str, cid: &str) -> Result<Playlist> {
        let path = if id_or_urn.starts_with("soundcloud:system-playlists:") {
            format!("/system-playlists/{id_or_urn}")
        } else {
            format!("/playlists/{id_or_urn}")
        };
        let u = self.v2_url(&path, cid);
        let mut p: Playlist = self.get_json(&u).await?;
        if let Some(tracks) = &mut p.tracks {
            let stub_ids: Vec<i64> = tracks
                .iter()
                .filter(|t| t.title.is_empty() && t.id != 0)
                .map(|t| t.id)
                .collect();
            if !stub_ids.is_empty() {
                if let Ok(hydrated) = self.tracks_by_ids(&stub_ids, cid).await {
                    let map: std::collections::HashMap<i64, Track> =
                        hydrated.into_iter().map(|t| (t.id, t)).collect();
                    for t in tracks.iter_mut() {
                        if t.title.is_empty() {
                            if let Some(full) = map.get(&t.id) {
                                *t = full.clone();
                            }
                        }
                    }
                }
            }
            // Stubs still without a title are tracks the API wouldn't return
            // (private or removed): drop them rather than show blank
            // "Unknown / 0:00" rows. Playlist writes read their ids through
            // playlist_ids_for_write, so this can't drop tracks from a PUT.
            tracks.retain(|t| !t.title.is_empty());
        }
        Ok(p)
    }

    pub async fn related(&self, track_id: i64, cid: &str) -> Result<ApiList<Track>> {
        let u = self.v2_url(&format!("/tracks/{track_id}/related"), cid);
        self.get_json(&u).await
    }

    pub async fn mixed_selections(&self, cid: &str) -> Result<serde_json::Value> {
        let u = self.v2_url("/mixed-selections?limit=6", cid);
        self.get_json(&u).await
    }

    /// Follow-feed stream (posts and reposts of followed artists) — fallback
    /// data source for the stories module when artist shortcuts are missing.
    pub async fn stream_feed(&self, cid: &str, limit: usize) -> Result<serde_json::Value> {
        let u = self.v2_url(&format!("/stream?limit={limit}"), cid);
        self.get_json(&u).await
    }

    async fn get_mobile_json(&self, access: &str, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_mobile, path);
        crate::console::http_request("GET", &url, None);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION)
            .send()
            .await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let bytes = resp.bytes().await?;
        crate::console::http_response("GET", &response_url, status, &bytes);
        crate::dlog!("HTTP GET mobile {path} -> {status}");
        if !status.is_success() {
            anyhow::bail!("HTTP {status} for {path}");
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Artist shortcuts strip (SoundCloud stories): the followed artists that
    /// posted updates, with server-side read state.
    /// GET /you/artist_shortcuts → { "items": [{ user_urn, unread_update_at,
    /// has_read, user }] }
    pub async fn artist_shortcuts(&self, access: &str) -> Result<serde_json::Value> {
        self.get_mobile_json(access, "/you/artist_shortcuts").await
    }

    /// Stories of one artist shortcut.
    /// GET /you/artist_shortcuts/stories/{user_urn} → { artist_urn,
    /// last_read_story_timestamp, stories: [{ track_post | track_repost |
    /// playlist_post | playlist_repost }] }
    pub async fn artist_shortcut_stories(
        &self,
        access: &str,
        user_urn: &str,
    ) -> Result<serde_json::Value> {
        // URNs ("soundcloud:users:123") go into the path verbatim — colons are
        // legal in path segments and the official client does not escape them.
        self.get_mobile_json(access, &format!("/you/artist_shortcuts/stories/{user_urn}"))
            .await
    }

    /// Mark an artist's stories as read, exactly like the official client:
    /// POST /users/{urn}/updates/read_receipts with
    /// {"read_receipts":[{"artist": urn, "last_update_read": "yyyy/MM/dd HH:mm:ss +0000"}]}
    pub async fn artist_shortcut_mark_read(
        &self,
        access: &str,
        user_urn: &str,
        last_update_read: &str,
    ) -> Result<()> {
        let body = serde_json::json!({
            "read_receipts": [{
                "artist": user_urn,
                "last_update_read": last_update_read,
            }]
        });
        let url = format!(
            "{}/users/{user_urn}/updates/read_receipts",
            self.base_mobile
        );
        crate::console::http_request("POST", &url, Some(&body));
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json; charset=utf-8")
            .header("Content-Type", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION)
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let response_body = resp.bytes().await?;
        crate::console::http_response("POST", &response_url, status, &response_body);
        if !status.is_success() {
            anyhow::bail!("HTTP {status} for read_receipts");
        }
        Ok(())
    }

    /// Send one play marker using the Android client's `ApiRecentlyPlayed`
    /// format and `/recently-played/contexts/v2` endpoint.
    pub async fn record_listening_history(
        &self,
        access: &str,
        track_id: i64,
        played_at_ms: u64,
    ) -> Result<()> {
        if track_id <= 0 {
            anyhow::bail!("invalid track id for listening history: {track_id}");
        }
        let body = serde_json::json!({
            "collection": [{
                "played_at": played_at_ms,
                "urn": format!("soundcloud:tracks:{track_id}"),
            }]
        });
        self.mobile_write_ok(
            "listening history",
            reqwest::Method::POST,
            access,
            "/recently-played/contexts/v2",
            Some(body),
        )
        .await
    }

    /// Whether the mobile API takes this token (true for the login window's,
    /// false for a browser session's, which can't write).
    pub async fn mobile_accepts(&self, access: &str) -> bool {
        if self.get_mobile_json(access, "/me?treating=1").await.is_ok() {
            return true;
        }
        // /me answers 500 to every token lately, which says nothing. An
        // empty playlist create does: refused (401) for a token that can't
        // write, rejected as incomplete (400) for one that can. It creates
        // nothing either way.
        match self
            .mobile_write(
                reqwest::Method::POST,
                access,
                "/playlists",
                Some(&serde_json::json!({})),
            )
            .await
        {
            Ok(r) => !matches!(r.status().as_u16(), 401 | 403),
            Err(_) => false,
        }
    }

    pub async fn me(&self, access: &str) -> Result<Me> {
        // temporary override for this call
        let mobile_url = format!("{}/me?treating=1", self.base_mobile);
        crate::console::http_request("GET", &mobile_url, None);
        let req = self
            .http
            .get(&mobile_url)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION);
        let resp = req.send().await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let response_body = resp.bytes().await?;
        crate::console::http_response("GET", &response_url, status, &response_body);
        if status.is_success() {
            let v: serde_json::Value = serde_json::from_slice(&response_body)?;
            if let Some(user) = v.get("user") {
                return Ok(serde_json::from_value(user.clone())?);
            }
            return Ok(serde_json::from_value(v)?);
        }
        // v2 fallback
        let u = format!(
            "{}/me?client_id={}&app_version={}",
            self.base_v2, CLIENT_ID, APP_VERSION
        );
        crate::console::http_request("GET", &u, None);
        let response = self
            .http
            .get(&u)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = response.status();
        let response_url = response.url().to_string();
        let response_body = response.bytes().await?;
        crate::console::http_response("GET", &response_url, status, &response_body);
        if !status.is_success() {
            anyhow::bail!("HTTP {status} for /me");
        }
        let v: serde_json::Value = serde_json::from_slice(&response_body)?;
        Ok(serde_json::from_value(v)?)
    }

    /// Write-request common headers (browser-like: origin + cookies) — DataDome
    /// expects these for anything that mutates state.
    fn browser_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut r = req
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
            )
            .header("Origin", "https://soundcloud.com")
            .header("Referer", "https://soundcloud.com/")
            .header("Accept", "application/json; charset=utf-8");
        if let Some(ck) = self.cookie_header() {
            r = r.header("Cookie", ck);
        }
        r
    }

    /// DataDome answered a write with its check: shown in the bridge's
    /// window, which is the browser the check then vouches for. True once
    /// passed (the write is to go again, through the bridge), or when one
    /// was just passed and this is still refused (no second check then).
    async fn try_check(&self, body: &str) -> bool {
        let Some(url) = crate::captcha::url_in(body).filter(|u| !crate::captcha::is_block(u))
        else {
            return false;
        };
        if crate::captcha::passed_recently() {
            return true;
        }
        let Some(bridge) = &self.bridge else {
            return false;
        };
        if bridge.solve_check(&url).await {
            crate::captcha::note_passed();
            true
        } else {
            false
        }
    }

    fn handle_resp_cookie(&self, resp: &reqwest::Response) {
        for val in resp.headers().get_all("set-cookie") {
            if let Ok(s) = val.to_str() {
                crate::cookie_auth::update_datadome_cookie(s);
            }
        }
    }

    /// A write the way the Android app sends it: api-mobile.soundcloud.com,
    /// the Android User-Agent and the OAuth header, no browser cookies.
    /// api-v2 sits behind DataDome, which answers every write from outside a
    /// real browser session with a captcha (403); the mobile API does not.
    async fn mobile_write(
        &self,
        method: reqwest::Method,
        access: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response> {
        if crate::captcha::backing_off() {
            crate::dlog!("HTTP {method} mobile {path}: held (DataDome refused a save recently)");
            return Ok(refused_response(
                reqwest::StatusCode::FORBIDDEN,
                BROWSER_CHECK,
            ));
        }
        let url = format!("{}{}", self.base_mobile, path);
        crate::console::http_request(method.as_str(), &url, body);
        let mut req = self
            .http
            .request(method.clone(), &url)
            .header("Authorization", format!("OAuth {access}"))
            .header("Accept", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION);
        if let Some(b) = body {
            req = req.json(b);
        }
        let method_name = if crate::console::debug_enabled() {
            method_label(&req)
        } else {
            String::new()
        };
        let resp = req.send().await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let response_headers = resp.headers().clone();
        let response_body = resp.bytes().await?;
        crate::console::http_response(method_name.as_str(), &response_url, status, &response_body);
        crate::dlog!("HTTP {method_name} mobile {path} -> {status}");
        let body = String::from_utf8_lossy(&response_body).into_owned();
        if status != reqwest::StatusCode::FORBIDDEN {
            return response_with_body(status, response_headers, body);
        }
        if self.try_check(&body).await {
            return Ok(refused_response(status, CHECK_PASSED));
        }
        if crate::captcha::url_in(&body).is_some() {
            crate::captcha::note_challenge();
            return Ok(refused_response(status, BROWSER_CHECK));
        }
        Ok(refused_response(status, &body))
    }

    /// `mobile_write`, reporting failure as an error that names the action.
    async fn mobile_write_ok(
        &self,
        what: &str,
        method: reqwest::Method,
        access: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<()> {
        let resp = self
            .mobile_write(method, access, path, body.as_ref())
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "{what} failed: HTTP {st} {}",
            text.chars().take(160).collect::<String>()
        )
    }

    /// POST a GraphQL document to graph.soundcloud.com like the Android app
    /// ({query, variables}); returns `data`, or the first GraphQL error.
    async fn graphql(
        &self,
        access: Option<&str>,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload = serde_json::json!({ "query": query, "variables": variables });
        crate::console::http_request("POST", GRAPH_API, Some(&payload));
        let mut req = self
            .http
            .post(GRAPH_API)
            .header("Accept", "application/json; charset=utf-8")
            .header("User-Agent", USER_AGENT)
            .header("App-Version", APP_VERSION)
            .json(&payload);
        if let Some(tok) = access.filter(|t| !t.is_empty()) {
            req = req.header("Authorization", format!("OAuth {tok}"));
        }
        let resp = req.send().await?;
        let st = resp.status();
        let response_url = resp.url().to_string();
        let bytes = resp.bytes().await?;
        crate::console::http_response("POST", &response_url, st, &bytes);
        crate::dlog!(
            "HTTP POST graphql {} -> {st}",
            query.split(['(', '{']).next().unwrap_or("").trim()
        );
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        if let Some(err) = v.get("errors").and_then(|e| e.get(0)) {
            crate::dlog!("  graphql error: {err}");
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("error");
            anyhow::bail!("graphql: {msg}");
        }
        if !st.is_success() {
            anyhow::bail!("graphql: HTTP {st}");
        }
        Ok(v.get("data").cloned().unwrap_or_default())
    }

    /// Waveform reactions of a track at the given seconds, as the Android
    /// app polls them (ReactionsPerTarget): targets are "ts:<second>" under
    /// the track, up to `per_second` reactions each.
    pub async fn track_reactions(
        &self,
        access: Option<&str>,
        track_id: i64,
        seconds: &[u64],
        per_second: usize,
    ) -> Result<Vec<WaveReaction>> {
        const QUERY: &str =
            "query ReactionsPerTarget($interactionTypeUrn: String!, $limitPerTarget: Int!, \
            $parentUrn: String!, $targetUrns: [String!]!) { targetInteractions(interactionTypeUrn: \
            $interactionTypeUrn, parentUrn: $parentUrn, limitPerTarget: $limitPerTarget, \
            targetUrns: $targetUrns) { userInteraction targetUrn } }";
        // SoundCloud refuses a request over 200 (targets x limit): split
        const MAX_PER_REQUEST: usize = 200;
        let per_second = per_second.clamp(1, MAX_PER_REQUEST);
        let mut list = Vec::new();
        for chunk in seconds.chunks(MAX_PER_REQUEST / per_second) {
            let targets: Vec<String> = chunk.iter().map(|s| format!("ts:{s}")).collect();
            let data = self
                .graphql(
                    access,
                    QUERY,
                    serde_json::json!({
                        "interactionTypeUrn": TRACK_REACTION_TYPE,
                        "limitPerTarget": per_second,
                        "parentUrn": format!("soundcloud:tracks:{track_id}"),
                        "targetUrns": targets,
                    }),
                )
                .await?;
            if let Some(items) = data.get("targetInteractions").and_then(|l| l.as_array()) {
                list.extend(items.iter().cloned());
            }
        }
        Ok(list
            .iter()
            .filter_map(|it| {
                let second = it
                    .get("targetUrn")?
                    .as_str()?
                    .rsplit(':')
                    .next()?
                    .parse()
                    .ok()?;
                let value = it.get("userInteraction")?.as_str()?;
                let codepoint = value
                    .strip_prefix(REACTION_VALUE_PREFIX)
                    .unwrap_or(value)
                    .to_string();
                Some(WaveReaction { second, codepoint })
            })
            .collect())
    }

    /// React at a second of a track (UpsertInteraction), as the Android
    /// app's quick-reaction bar does.
    pub async fn add_track_reaction(
        &self,
        access: &str,
        track_id: i64,
        second: u64,
        codepoint: &str,
    ) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge
                .add_track_reaction(access, track_id, second, codepoint)
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) => {
                    crate::log!("bridge add_track_reaction failed: {e}; trying http fallback")
                }
            }
        }
        const MUTATION: &str = "mutation UpsertInteraction($input: InteractionInput!) { \
            upsertInteraction(input: $input) { targetUrn } }";
        self.graphql(
            Some(access),
            MUTATION,
            serde_json::json!({
                "input": {
                    "targetUrn": format!("ts:{second}"),
                    "parentUrn": format!("soundcloud:tracks:{track_id}"),
                    "interactionTypeUrn": TRACK_REACTION_TYPE,
                    "interactionTypeValueUrn": format!("{REACTION_VALUE_PREFIX}{codepoint}"),
                }
            }),
        )
        .await
        .map(|_| ())
    }

    async fn send_browser_write<F>(&self, build_req: F) -> Result<reqwest::Response>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        if crate::captcha::backing_off() {
            crate::dlog!("HTTP web write: held (DataDome refused a save recently)");
            return Ok(refused_response(
                reqwest::StatusCode::FORBIDDEN,
                BROWSER_CHECK,
            ));
        }
        let request = self.browser_headers(build_req());
        let request_parts = request.try_clone().and_then(|request| request.build().ok());
        if let Some(request) = request_parts.as_ref() {
            crate::console::http_request_raw(
                request.method().as_str(),
                request.url().as_str(),
                request.body().and_then(reqwest::Body::as_bytes),
            );
        }
        let method = request_parts
            .as_ref()
            .map(|request| request.method().to_string())
            .unwrap_or_else(|| "?".into());
        let resp = request.send().await?;
        let response_url = resp.url().to_string();
        let response_headers = resp.headers().clone();
        crate::dlog!(
            "HTTP web write {} -> {}",
            crate::console::redact(&response_url),
            resp.status()
        );
        self.handle_resp_cookie(&resp);
        let status = resp.status();
        let response_body = resp.bytes().await?;
        crate::console::http_response(&method, &response_url, status, &response_body);
        let body = String::from_utf8_lossy(&response_body).into_owned();
        if status != reqwest::StatusCode::FORBIDDEN {
            return response_with_body(status, response_headers, body);
        }
        // Refused: DataDome wants a browser check (or blocks the network).
        // Resending doesn't help, and neither does solving the captcha in a
        // window (DataDome ties the pass to the browser that solved it; seen
        // in a debug log). Saves hold off for a while instead (captcha.rs).
        crate::dlog!("  refused: {}", body.chars().take(600).collect::<String>());
        if self.try_check(&body).await {
            return Ok(refused_response(status, CHECK_PASSED));
        }
        let url = crate::captcha::url_in(&body);
        if url.is_some() {
            crate::captcha::note_challenge();
        }
        Ok(match url {
            Some(url) if crate::captcha::is_block(&url) => {
                refused_response(status, NETWORK_BLOCKED)
            }
            Some(_) => refused_response(status, BROWSER_CHECK),
            None => refused_response(status, &body),
        })
    }

    /// Like a track like the Android app: POST /likes/tracks/create with
    /// {"likes":[{"target_urn": ...}]}. The v2 web form is the fallback.
    pub async fn like_track(&self, access: &str, me_id: i64, track_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.like_track(access, me_id, track_id, false).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge like_track failed: {e}; trying http fallback"),
            }
        }
        let body =
            serde_json::json!({"likes": [{"target_urn": format!("soundcloud:tracks:{track_id}")}]});
        match self
            .mobile_write_ok(
                "like",
                reqwest::Method::POST,
                access,
                "/likes/tracks/create",
                Some(body),
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("like via mobile api: {e}; trying v2"),
        }
        let u = format!("{V2_API}/users/{me_id}/track_likes/{track_id}");
        let sep = if u.contains('?') { '&' } else { '?' };
        let url = format!("{u}{sep}client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .put(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("captcha-delivery") {
            anyhow::bail!("blocked by DataDome (status {st})");
        }
        anyhow::bail!("like failed: HTTP {st}")
    }

    pub async fn unlike_track(&self, access: &str, me_id: i64, track_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.like_track(access, me_id, track_id, true).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge unlike_track failed: {e}; trying http fallback"),
            }
        }
        let body =
            serde_json::json!({"likes": [{"target_urn": format!("soundcloud:tracks:{track_id}")}]});
        match self
            .mobile_write_ok(
                "unlike",
                reqwest::Method::POST,
                access,
                "/likes/tracks/delete",
                Some(body),
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("unlike via mobile api: {e}; trying v2"),
        }
        let u = format!("{V2_API}/users/{me_id}/track_likes/{track_id}");
        let sep = if u.contains('?') { '&' } else { '?' };
        let url = format!("{u}{sep}client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() || st == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("captcha-delivery") {
            anyhow::bail!("blocked by DataDome (status {st})");
        }
        anyhow::bail!("unlike failed: HTTP {st}")
    }

    /// Repost a track like the Android app: POST /reposts/tracks/{urn} with
    /// an (empty) caption. The v2 web form is the fallback.
    pub async fn repost_track(&self, access: &str, _me_id: i64, track_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.repost_track(access, track_id, false).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge repost_track failed: {e}; trying http fallback"),
            }
        }
        let path = format!("/reposts/tracks/soundcloud:tracks:{track_id}");
        let body = serde_json::json!({"caption": ""});
        match self
            .mobile_write_ok("repost", reqwest::Method::POST, access, &path, Some(body))
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("repost via mobile api: {e}; trying v2"),
        }
        let url = format!("{V2_API}/me/track_reposts/{track_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .put(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("captcha-delivery") {
            anyhow::bail!("blocked by DataDome (status {st})");
        }
        anyhow::bail!("repost failed: HTTP {st}")
    }

    /// Unrepost a track: DELETE /reposts/tracks/{urn} (mobile), v2 fallback.
    pub async fn unrepost_track(&self, access: &str, _me_id: i64, track_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.repost_track(access, track_id, true).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge unrepost_track failed: {e}; trying http fallback"),
            }
        }
        let path = format!("/reposts/tracks/soundcloud:tracks:{track_id}");
        match self
            .mobile_write(reqwest::Method::DELETE, access, &path, None)
            .await
        {
            Ok(r) if r.status().is_success() || r.status() == reqwest::StatusCode::NOT_FOUND => {
                return Ok(())
            }
            Ok(r) => crate::log!("unrepost via mobile api: HTTP {}; trying v2", r.status()),
            Err(e) => crate::log!("unrepost via mobile api: {e}; trying v2"),
        }
        let url = format!("{V2_API}/me/track_reposts/{track_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() || st == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("captcha-delivery") {
            anyhow::bail!("blocked by DataDome (status {st})");
        }
        anyhow::bail!("unrepost failed: HTTP {st}")
    }

    /// Follow a user like the Android app: POST /follows/users/{urn}
    /// (DELETE to unfollow). The v2 web form is the fallback.
    pub async fn follow_user(&self, access: &str, user_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.follow_user(access, user_id, false).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge follow_user failed: {e}; trying http fallback"),
            }
        }
        let path = format!("/follows/users/soundcloud:users:{user_id}");
        match self
            .mobile_write_ok("follow", reqwest::Method::POST, access, &path, None)
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("follow via mobile api: {e}; trying v2"),
        }
        let url = format!("{V2_API}/me/followings/{user_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .post(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("captcha-delivery") {
            anyhow::bail!("blocked by DataDome (status {st})");
        }
        anyhow::bail!("follow failed: HTTP {st}")
    }

    pub async fn unfollow_user(&self, access: &str, user_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.follow_user(access, user_id, true).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge unfollow_user failed: {e}; trying http fallback"),
            }
        }
        let path = format!("/follows/users/soundcloud:users:{user_id}");
        match self
            .mobile_write_ok("unfollow", reqwest::Method::DELETE, access, &path, None)
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("unfollow via mobile api: {e}; trying v2"),
        }
        let url = format!("{V2_API}/me/followings/{user_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        anyhow::bail!("unfollow failed: HTTP {st}")
    }

    pub async fn is_following(&self, access: &str, me_id: i64, user_id: i64) -> bool {
        let set = self.my_following_ids(access, me_id).await;
        set.contains(&user_id)
    }

    /// IDs of users I follow (single boolean-set query).
    pub async fn my_following_ids(
        &self,
        access: &str,
        me_id: i64,
    ) -> std::collections::HashSet<i64> {
        let u = format!(
            "{V2_API}/users/{me_id}/followings/ids?limit=5000&linked_partitioning=1&client_id={CLIENT_ID}"
        );
        let mut out = std::collections::HashSet::new();
        let mut next: Option<String> = Some(u);
        while let Some(curl) = next.take() {
            crate::console::http_request("GET", &curl, None);
            let r = match self
                .http
                .get(&curl)
                .header("Authorization", format!("OAuth {access}"))
                .header("Accept", "application/json; charset=utf-8")
                .send()
                .await
            {
                Ok(r) => r,
                _ => break,
            };
            let status = r.status();
            let response_url = r.url().to_string();
            let bytes = match r.bytes().await {
                Ok(bytes) => bytes,
                Err(_) => break,
            };
            crate::console::http_response("GET", &response_url, status, &bytes);
            if !status.is_success() {
                break;
            }
            let v: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => break,
            };
            if let Some(arr) = v.get("collection").and_then(|c| c.as_array()) {
                for x in arr {
                    if let Some(id) = x
                        .as_i64()
                        .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
                    {
                        out.insert(id);
                    }
                }
            }
            next = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .map(str::to_string);
            if out.len() > 20000 {
                break;
            }
        }
        out
    }

    /// IDs of tracks I reposted.
    /// SoundCloud's own playlists (mixes, stations) saved in the user's
    /// library, by URN (the web app's /me/system_playlist_likes/urns).
    pub async fn my_liked_system_playlists(
        &self,
        access: &str,
    ) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        let mut next = Some(format!(
            "{V2_API}/me/system_playlist_likes/urns?limit=200&client_id={CLIENT_ID}"
        ));
        while let Some(url) = next.take() {
            crate::console::http_request("GET", &url, None);
            let Ok(resp) = self
                .http
                .get(&url)
                .header("Authorization", format!("OAuth {access}"))
                .header("Accept", "application/json; charset=utf-8")
                .send()
                .await
            else {
                break;
            };
            let status = resp.status();
            let response_url = resp.url().to_string();
            let Ok(bytes) = resp.bytes().await else {
                break;
            };
            crate::console::http_response("GET", &response_url, status, &bytes);
            if !status.is_success() {
                break;
            }
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                break;
            };
            for x in v
                .get("collection")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                // plain URNs; objects carrying one are taken too
                let urn = x.as_str().or_else(|| x.get("urn").and_then(|u| u.as_str()));
                if let Some(urn) = urn {
                    out.insert(urn.to_string());
                }
            }
            next = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .map(str::to_string);
            if out.len() > 2000 {
                break;
            }
        }
        out
    }

    /// Save (or remove) a mix or station in the user's library.
    pub async fn like_system_playlist(
        &self,
        access: &str,
        me_id: i64,
        urn: &str,
        liked: bool,
    ) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.like_system_playlist(access, me_id, urn, liked).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge like_system_playlist failed: {e}; trying http"),
            }
        }
        let url =
            format!("{V2_API}/users/{me_id}/system_playlist_likes/{urn}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                let req = if liked {
                    self.http.delete(&url)
                } else {
                    self.http.put(&url)
                };
                req.header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() || (liked && st == reqwest::StatusCode::NOT_FOUND) {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "saving the station/mix failed: HTTP {st} {}",
            body.chars().take(160).collect::<String>()
        )
    }

    pub async fn my_repost_ids(&self, access: &str) -> std::collections::HashSet<i64> {
        let u = format!("{V2_API}/me/track_reposts/ids?limit=5000&client_id={CLIENT_ID}");
        let mut out = std::collections::HashSet::new();
        let mut next: Option<String> = Some(u);
        while let Some(curl) = next.take() {
            crate::console::http_request("GET", &curl, None);
            let resp = match self
                .http
                .get(&curl)
                .header("Authorization", format!("OAuth {access}"))
                .header("Accept", "application/json; charset=utf-8")
                .send()
                .await
            {
                Ok(r) => r,
                _ => break,
            };
            let status = resp.status();
            let response_url = resp.url().to_string();
            let bytes = match resp.bytes().await {
                Ok(bytes) => bytes,
                Err(_) => break,
            };
            crate::console::http_response("GET", &response_url, status, &bytes);
            if !status.is_success() {
                break;
            }
            let v: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => break,
            };
            if let Some(arr) = v
                .get("collection")
                .and_then(|c| c.as_array())
                .or_else(|| v.as_array())
            {
                for x in arr {
                    if let Some(id) = x
                        .as_i64()
                        .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
                    {
                        out.insert(id);
                    }
                }
            }
            next = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .map(str::to_string);
            if out.len() > 20000 {
                break;
            }
        }
        out
    }

    /// Post a comment at `track_time` (ms) with body text.
    pub async fn post_comment(
        &self,
        access: &str,
        track_id: i64,
        body_text: &str,
        track_time_ms: u64,
    ) -> Result<Comment> {
        // Follow the Android client first. Its mobile route avoids the public
        // API's CORS/DataDome path and accepts the same JSON comment body.
        let payload = serde_json::json!({
            "comment": {
                "body": body_text,
                "timestamp": track_time_ms,
            }
        });
        let mut failures = Vec::new();
        let mobile_path = format!("/tracks/{track_id}/comments");
        match self
            .mobile_write(reqwest::Method::POST, access, &mobile_path, Some(&payload))
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let response_body = resp.bytes().await?;
                if status.is_success() {
                    let value: serde_json::Value = serde_json::from_slice(&response_body)?;
                    return Ok(comment_from_json(&value));
                }
                let body = String::from_utf8_lossy(&response_body).into_owned();
                if status == reqwest::StatusCode::BAD_REQUEST {
                    anyhow::bail!(
                        "comment failed: HTTP {status} {}",
                        body.chars().take(220).collect::<String>()
                    );
                }
                if body.contains(CHECK_PASSED) {
                    crate::log!("SoundCloud's check passed for comment: retrying mobile API once");
                    match self
                        .mobile_write(reqwest::Method::POST, access, &mobile_path, Some(&payload))
                        .await
                    {
                        Ok(retry) => {
                            let retry_status = retry.status();
                            let retry_body = retry.bytes().await?;
                            if retry_status.is_success() {
                                let value: serde_json::Value = serde_json::from_slice(&retry_body)?;
                                return Ok(comment_from_json(&value));
                            }
                            failures.push(format!(
                                "mobile API retry HTTP {retry_status}: {}",
                                String::from_utf8_lossy(&retry_body)
                                    .chars()
                                    .take(160)
                                    .collect::<String>()
                            ));
                        }
                        Err(e) => failures.push(format!("mobile API retry: {e}")),
                    }
                } else {
                    failures.push(format!(
                        "mobile API HTTP {status}: {}",
                        body.chars().take(160).collect::<String>()
                    ));
                }
            }
            Err(e) => failures.push(format!("mobile API request: {e}")),
        }

        // If the mobile route is refused, try the web app's v2 form endpoint
        // inside WebView2 (same API family the SoundCloud site uses).
        if let Some(bridge) = &self.bridge {
            match bridge
                .post_comment(access, track_id, body_text, track_time_ms)
                .await
            {
                Ok(comment) => return Ok(comment),
                Err(e) => {
                    failures.push(format!("browser bridge: {e}"));
                    crate::log!("bridge post_comment failed: {e}; trying v2 form API");
                }
            }
        }

        // The v2 form parser expects bracketed fields, not the nested JSON
        // accepted by the mobile and public API routes.
        let v2_url = format!("{V2_API}/tracks/{track_id}/comments?client_id={CLIENT_ID}");
        let timestamp = track_time_ms.to_string();
        let form_fields = [
            ("comment[body]", body_text),
            ("comment[timestamp]", timestamp.as_str()),
        ];
        let legacy = self
            .send_browser_write(|| {
                self.http
                    .post(&v2_url)
                    .header("Authorization", format!("OAuth {access}"))
                    .form(&form_fields)
            })
            .await;
        match legacy {
            Ok(resp) => {
                let status = resp.status();
                let bytes = resp.bytes().await?;
                if status.is_success() {
                    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
                    return Ok(comment_from_json(&value));
                }
                failures.push(format!(
                    "v2 form API HTTP {status}: {}",
                    String::from_utf8_lossy(&bytes)
                        .chars()
                        .take(160)
                        .collect::<String>()
                ));
            }
            Err(e) => failures.push(format!("v2 form API request: {e}")),
        }

        anyhow::bail!("comment failed after all routes: {}", failures.join("; "))
    }

    pub async fn delete_comment(&self, access: &str, comment_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.delete_comment(access, comment_id).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge delete_comment failed: {e}; trying http fallback"),
            }
        }
        let path = format!("/comments/soundcloud:comments:{comment_id}");
        match self
            .mobile_write_ok(
                "delete comment",
                reqwest::Method::DELETE,
                access,
                &path,
                None,
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("delete comment via mobile api: {e}; trying v2"),
        }
        let url = format!("{V2_API}/comments/{comment_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("delete comment failed: HTTP {}", resp.status())
        }
    }

    /// Radio: tracks from a track-station. Station URN derives from the track:
    /// soundcloud:system-playlists:track-stations:{id} — served via system-playlists.
    /// SoundCloud returns 50 stub track IDs that must be hydrated via tracks_by_ids.
    /// The original station order is strictly preserved via HashMap lookup.
    /// The seed track is guaranteed to be at index 0.
    /// If the system station playlist is empty or missing, falls back to seed track + related tracks.
    pub async fn station_for_track(
        &self,
        track_id: i64,
        cid: &str,
    ) -> Result<(Vec<Track>, Option<String>)> {
        let urn = format!("soundcloud:system-playlists:track-stations:{track_id}");
        let u = self.v2_url(&format!("/system-playlists/{urn}"), cid);
        let mut artwork_url = None;
        if let Ok(v) = self.get_json::<serde_json::Value>(&u).await {
            artwork_url = v
                .get("artwork_url")
                .and_then(serde_json::Value::as_str)
                .filter(|url| !url.is_empty())
                .map(str::to_string);
            let raw_tracks = v
                .get("tracks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            let ids: Vec<i64> = raw_tracks
                .iter()
                .filter_map(|t| t.get("id").and_then(|i| i.as_i64()))
                .filter(|id| *id != 0)
                .collect();
            if !ids.is_empty() {
                if let Ok(hydrated) = self.tracks_by_ids(&ids, cid).await {
                    let map: std::collections::HashMap<i64, Track> =
                        hydrated.into_iter().map(|t| (t.id, t)).collect();
                    let mut ordered: Vec<Track> =
                        ids.iter().filter_map(|id| map.get(id).cloned()).collect();
                    if !ordered.is_empty() {
                        // Ensure track_id is at index 0 if present in station, or if missing, fetch and prepend it
                        if let Some(pos) = ordered.iter().position(|t| t.id == track_id) {
                            if pos > 0 {
                                let seed = ordered.remove(pos);
                                ordered.insert(0, seed);
                            }
                        } else if let Ok(seed) = self.track(track_id, cid).await {
                            ordered.insert(0, seed);
                        }
                        return Ok((ordered, artwork_url));
                    }
                }
            }
        }
        // Fallback: seed track + related tracks
        let mut result = Vec::new();
        if let Ok(seed) = self.track(track_id, cid).await {
            result.push(seed);
        }
        if let Ok(rel) = self.related(track_id, cid).await {
            for t in rel.collection {
                if t.id != track_id && !result.iter().any(|r| r.id == t.id) {
                    result.push(t);
                }
            }
        }
        if !result.is_empty() {
            Ok((result, artwork_url))
        } else {
            anyhow::bail!("No station tracks found for track {}", track_id)
        }
    }

    /// Artist radio station. Hydrates stub track IDs and preserves station order.
    /// Falls back to user top tracks and related artists if station playlist is unavailable.
    pub async fn station_for_artist(
        &self,
        user_id: i64,
        cid: &str,
    ) -> Result<(Vec<Track>, Option<String>)> {
        let urn = format!("soundcloud:system-playlists:artist-stations:{user_id}");
        let u = self.v2_url(&format!("/system-playlists/{urn}"), cid);
        let mut artwork_url = None;
        if let Ok(v) = self.get_json::<serde_json::Value>(&u).await {
            artwork_url = v
                .get("artwork_url")
                .and_then(serde_json::Value::as_str)
                .filter(|url| !url.is_empty())
                .map(str::to_string);
            let raw_tracks = v
                .get("tracks")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            let ids: Vec<i64> = raw_tracks
                .iter()
                .filter_map(|t| t.get("id").and_then(|i| i.as_i64()))
                .filter(|id| *id != 0)
                .collect();
            if !ids.is_empty() {
                if let Ok(hydrated) = self.tracks_by_ids(&ids, cid).await {
                    let map: std::collections::HashMap<i64, Track> =
                        hydrated.into_iter().map(|t| (t.id, t)).collect();
                    let ordered: Vec<Track> =
                        ids.iter().filter_map(|id| map.get(id).cloned()).collect();
                    if !ordered.is_empty() {
                        return Ok((ordered, artwork_url));
                    }
                }
            }
        }
        // Fallback: Artist top tracks (or all tracks) + related artists' top tracks
        let mut result = self.user_top_tracks(user_id, cid).await.unwrap_or_default();
        if result.is_empty() {
            result = self
                .user_all_tracks(user_id, cid, 20)
                .await
                .unwrap_or_default();
        }
        if let Ok(rel_artists) = self.user_related_artists(user_id, cid).await {
            for a in rel_artists.iter().take(5) {
                if result.len() >= 40 {
                    break;
                }
                if let Ok(a_tracks) = self.user_top_tracks(a.id, cid).await {
                    for t in a_tracks {
                        if !result.iter().any(|r| r.id == t.id) {
                            result.push(t);
                        }
                    }
                }
            }
        }
        if !result.is_empty() {
            Ok((result, artwork_url))
        } else {
            anyhow::bail!("No station tracks found for artist {}", user_id)
        }
    }

    /// Top tracks — web form first, then the mobile client's route if needed.
    pub async fn user_top_tracks(&self, user_id: i64, cid: &str) -> Result<Vec<Track>> {
        let u = self.v2_url(
            &format!("/users/{user_id}/toptracks?limit=10&offset=0&linked_partitioning=1"),
            cid,
        );
        match self.get_json::<ApiList<Track>>(&u).await {
            Ok(list) => Ok(list.collection),
            Err(web_error) => {
                let Some(access) = self.access.as_deref() else {
                    return Err(web_error);
                };
                let value = self
                    .get_mobile_json(access, &format!("/users/{user_id}/top-tracks"))
                    .await
                    .with_context(|| format!("web top-tracks request failed: {web_error}"))?;
                let list: ApiList<Track> = serde_json::from_value(value)
                    .context("mobile top-tracks response had an unexpected shape")?;
                Ok(list.collection)
            }
        }
    }

    /// All posted tracks for a profile (long pagination).
    pub async fn user_all_tracks(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<Track>> {
        let u = self.v2_url(
            &format!(
                "/users/{user_id}/tracks?representation=&limit={limit}&offset=0&linked_partitioning=1"
            ),
            cid,
        );
        let list: ApiList<Track> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// Followers list.
    pub async fn user_followers(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<UserMini>> {
        let u = self.v2_url(
            &format!("/users/{user_id}/followers?limit={limit}&offset=0&linked_partitioning=1"),
            cid,
        );
        let list: ApiList<UserMini> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// Followings list.
    pub async fn user_followings(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<UserMini>> {
        let u = self.v2_url(
            &format!("/users/{user_id}/followings?limit={limit}&offset=0&linked_partitioning=1"),
            cid,
        );
        let list: ApiList<UserMini> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// Track likers / favoriters (users who liked the track).
    pub async fn track_favoriters(
        &self,
        track_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<UserMini>> {
        let u = self.v2_url(
            &format!("/tracks/{track_id}/likers?limit={limit}&offset=0&linked_partitioning=1"),
            cid,
        );
        let list: ApiList<UserMini> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// Track reposters (users who reposted the track).
    pub async fn track_reposters(
        &self,
        track_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<UserMini>> {
        let u = self.v2_url(
            &format!("/tracks/{track_id}/reposters?limit={limit}&offset=0&linked_partitioning=1"),
            cid,
        );
        let list: ApiList<UserMini> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// User's liked tracks (paginated single page).
    pub async fn user_liked_tracks(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<Track>> {
        let u = self.v2_url(
            &format!("/users/{user_id}/likes?limit={limit}&linked_partitioning=1"),
            cid,
        );
        let list: ApiList<LikeItem> = self.get_json(&u).await?;
        let tracks = list
            .collection
            .into_iter()
            .filter_map(|it| it.into_track())
            .collect();
        Ok(tracks)
    }

    /// Related artists suggestions for the profile page.
    pub async fn user_related_artists(&self, user_id: i64, cid: &str) -> Result<Vec<UserMini>> {
        let u = self.v2_url(
            &format!(
                "/users/{user_id}/relatedartists?creators_only=false&page_size=12&limit=12&offset=0&linked_partitioning=1"
            ),
            cid,
        );
        let list: ApiList<UserMini> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    pub async fn user_albums(&self, user_id: i64, cid: &str) -> Result<Vec<Playlist>> {
        let u = self.v2_url(&format!("/users/{user_id}/albums?limit=20"), cid);
        let list: ApiList<Playlist> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// User's posted playlists (handles direct playlists and {playlist: {...}} wrappers).
    /// A user's playlists, 50 per page, following next_href for up to
    /// `max_pages` pages. When more than one page is wanted a failed later
    /// page is an error, not a silently shorter list.
    pub async fn user_playlists_posted(
        &self,
        user_id: i64,
        cid: &str,
        max_pages: usize,
    ) -> Result<Vec<Playlist>> {
        let first = self.v2_url(
            &format!("/users/{user_id}/playlists_without_albums?limit=50&linked_partitioning=1"),
            cid,
        );
        let mut v: serde_json::Value = match self.get_json(&first).await {
            Ok(v) => v,
            Err(_) => {
                let u2 = self.v2_url(
                    &format!("/users/{user_id}/playlists?limit=50&linked_partitioning=1"),
                    cid,
                );
                self.get_json(&u2).await?
            }
        };
        let mut out: Vec<Playlist> = Vec::new();
        let mut pages = 1;
        loop {
            if let Some(col) = v.get("collection").and_then(|c| c.as_array()) {
                for item in col {
                    let pl = item.get("playlist").cloned().or_else(|| Some(item.clone()));
                    if let Some(pl) = pl {
                        if let Ok(p) = serde_json::from_value::<Playlist>(pl) {
                            if p.id != 0 && !out.iter().any(|o| o.id == p.id) {
                                out.push(p);
                            }
                        }
                    }
                }
            }
            let next = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .filter(|n| !n.is_empty())
                .map(str::to_string);
            let Some(next) = next else { break };
            if pages >= max_pages.max(1) {
                break;
            }
            pages += 1;
            let url = if next.contains("client_id=") {
                next
            } else {
                let sep = if next.contains('?') { '&' } else { '?' };
                format!("{next}{sep}client_id={cid}")
            };
            v = self.get_json(&url).await?;
        }
        Ok(out)
    }

    /// Tracks a user reposted — /users/{id}/reposts (wrappers {track}).
    pub async fn user_reposts(&self, user_id: i64, cid: &str) -> Result<Vec<Track>> {
        Ok(self.user_reposts_page(user_id, cid, 40).await?.collection)
    }

    /// One page of reposted tracks plus SoundCloud's cursor for the next page.
    pub async fn user_reposts_page(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<ApiList<Track>> {
        let url = self.v2_url(
            &format!("/stream/users/{user_id}/reposts?limit={limit}&linked_partitioning=1"),
            cid,
        );
        let value: serde_json::Value = self.get_json(&url).await?;
        Ok(parse_user_reposts_page(value))
    }

    pub async fn user_reposts_next(&self, next_href: &str, cid: &str) -> Result<ApiList<Track>> {
        let url = if next_href.contains("client_id=") {
            next_href.to_string()
        } else {
            format!(
                "{next_href}{}client_id={cid}",
                if next_href.contains('?') { '&' } else { '?' }
            )
        };
        let value: serde_json::Value = self.get_json(&url).await?;
        Ok(parse_user_reposts_page(value))
    }

    /// Reposted tracks and playlists from one paginated stream request.
    pub async fn user_repost_items(
        &self,
        user_id: i64,
        cid: &str,
    ) -> Result<(Vec<Track>, Vec<Playlist>)> {
        let mut url = self.v2_url(
            &format!("/stream/users/{user_id}/reposts?limit=200&linked_partitioning=1"),
            cid,
        );
        let mut tracks = Vec::new();
        let mut playlists = Vec::new();
        let mut seen_tracks = std::collections::HashSet::new();
        let mut seen_playlists = std::collections::HashSet::new();
        for _ in 0..25 {
            let v: serde_json::Value = self.get_json(&url).await?;
            if let Some(col) = v.get("collection").and_then(|c| c.as_array()) {
                for item in col {
                    let track_value = item
                        .get("track")
                        .or_else(|| item.get("playlist").is_none().then_some(item));
                    if let Some(t) = track_value {
                        if let Ok(tr) = serde_json::from_value::<Track>(t.clone()) {
                            if tr.id != 0 && !tr.title.is_empty() && seen_tracks.insert(tr.id) {
                                tracks.push(tr);
                            }
                        }
                    }
                    if let Some(value) = item.get("playlist") {
                        if let Ok(playlist) = serde_json::from_value::<Playlist>(value.clone()) {
                            let key = playlist.id_or_urn();
                            if !key.is_empty()
                                && !playlist.title.is_empty()
                                && seen_playlists.insert(key)
                            {
                                playlists.push(playlist);
                            }
                        }
                    }
                }
            }
            let Some(next) = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .filter(|s| !s.is_empty())
            else {
                break;
            };
            url = if next.contains("client_id=") {
                next.to_string()
            } else {
                format!(
                    "{next}{}client_id={cid}",
                    if next.contains('?') { '&' } else { '?' }
                )
            };
        }
        Ok((tracks, playlists))
    }

    /// Playlists reposted by a user. The stream endpoint mixes repostable
    /// entity types in one collection; playlist reposts are wrapped as
    /// `{ playlist: ... }` (some responses use `track` only).
    pub async fn user_reposted_playlists(&self, user_id: i64, cid: &str) -> Result<Vec<Playlist>> {
        Ok(self.user_repost_items(user_id, cid).await?.1)
    }

    /// Next page for a search (follows next_href).
    pub async fn search_tracks_next(&self, next_href: &str) -> Result<ApiList<Track>> {
        let sep = if next_href.contains('?') { '&' } else { '?' };
        let u = format!("{next_href}{sep}app_locale={}", self.locale());
        self.get_json(&u).await
    }

    /// Liked playlists — web form: /users/{id}/playlist_likes (wrappers {playlist}).
    /// Follows next_href (up to 25 pages) so likes past the first page still
    /// reach the sidebar and the liked-playlist ids. A failed later page ends
    /// the list early instead of discarding the pages already read.
    pub async fn user_liked_playlists(&self, user_id: i64, cid: &str) -> Result<Vec<Playlist>> {
        const MAX_PAGES: usize = 25;
        let u = self.v2_url(
            &format!("/users/{user_id}/playlist_likes?limit=200&linked_partitioning=1"),
            cid,
        );
        let mut v: serde_json::Value = self.get_json(&u).await?;
        let mut out: Vec<Playlist> = Vec::new();
        let mut pages = 1;
        loop {
            if let Some(col) = v.get("collection").and_then(|c| c.as_array()) {
                for item in col {
                    let pl = item.get("playlist").cloned().or_else(|| Some(item.clone()));
                    if let Some(pl) = pl {
                        if let Ok(p) = serde_json::from_value::<Playlist>(pl) {
                            if p.id != 0 && !out.iter().any(|o| o.id == p.id) {
                                out.push(p);
                            }
                        }
                    }
                }
            }
            let next = v
                .get("next_href")
                .and_then(|n| n.as_str())
                .filter(|n| !n.is_empty())
                .map(str::to_string);
            let Some(next) = next else { break };
            if pages >= MAX_PAGES {
                break;
            }
            pages += 1;
            let url = if next.contains("client_id=") {
                next
            } else {
                let sep = if next.contains('?') { '&' } else { '?' };
                format!("{next}{sep}client_id={cid}")
            };
            v = match self.get_json(&url).await {
                Ok(v) => v,
                Err(e) => {
                    crate::log!("liked playlists page {pages} FAILED: {e}");
                    break;
                }
            };
        }
        Ok(out)
    }

    /// Like/unlike a playlist like the Android app: POST
    /// /likes/playlists/create|delete; the v2 web form is the fallback.
    pub async fn like_playlist(&self, access: &str, me_id: i64, playlist_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge
                .like_playlist(access, me_id, playlist_id, false)
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge like_playlist failed: {e}; trying http fallback"),
            }
        }
        let body = serde_json::json!({"likes": [{"target_urn": format!("soundcloud:playlists:{playlist_id}")}]});
        match self
            .mobile_write_ok(
                "like playlist",
                reqwest::Method::POST,
                access,
                "/likes/playlists/create",
                Some(body),
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("like playlist via mobile api: {e}; trying v2"),
        }
        let url =
            format!("{V2_API}/users/{me_id}/playlist_likes/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .put(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        let st = resp.status();
        if st.is_success() {
            return Ok(());
        }
        anyhow::bail!("like playlist failed: HTTP {st}")
    }

    pub async fn unlike_playlist(&self, access: &str, me_id: i64, playlist_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.like_playlist(access, me_id, playlist_id, true).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge unlike_playlist failed: {e}; trying http fallback"),
            }
        }
        let body = serde_json::json!({"likes": [{"target_urn": format!("soundcloud:playlists:{playlist_id}")}]});
        match self
            .mobile_write_ok(
                "unlike playlist",
                reqwest::Method::POST,
                access,
                "/likes/playlists/delete",
                Some(body),
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("unlike playlist via mobile api: {e}; trying v2"),
        }
        let url =
            format!("{V2_API}/users/{me_id}/playlist_likes/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("unlike playlist failed: HTTP {}", resp.status())
        }
    }

    /// Full user record (followers, followings counts, description, verified).
    pub async fn user_full(&self, user_id: i64, cid: &str) -> Result<serde_json::Value> {
        let u = self.v2_url(&format!("/users/{user_id}"), cid);
        self.get_json(&u).await
    }

    /// Resolve a canonical SoundCloud URL (track, user, playlist) via /resolve
    pub async fn resolve_url(&self, url: &str, cid: &str) -> Result<serde_json::Value> {
        let clean_url = url.trim();
        let canonical_url =
            if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
                format!("https://{clean_url}")
            } else {
                clean_url.to_string()
            };
        let encoded = urlencoding_lite(&canonical_url);
        let u = format!("{V2_API}/resolve?url={encoded}&client_id={cid}");
        self.get_json(&u).await
    }

    pub async fn resolve(&self, url: &str, cid: &str) -> Result<ResolvedEntity> {
        let v = self.resolve_url(url, cid).await?;
        let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        match kind {
            "track" => {
                let t: Track = serde_json::from_value(v)?;
                Ok(ResolvedEntity::Track(t))
            }
            "user" => {
                let u: UserMini = serde_json::from_value(v)?;
                Ok(ResolvedEntity::User(u))
            }
            "playlist" => {
                let p: Playlist = serde_json::from_value(v)?;
                Ok(ResolvedEntity::Playlist(p))
            }
            _ => Ok(ResolvedEntity::Unknown(v)),
        }
    }

    /// My reposted-track ids come via /me/track_reposts/ids; liked playlist ids:
    pub async fn my_liked_playlist_ids(
        &self,
        _access: &str,
        me_id: i64,
    ) -> std::collections::HashSet<i64> {
        let list = self
            .user_liked_playlists(me_id, CLIENT_ID)
            .await
            .unwrap_or_default();
        list.into_iter()
            .map(|p| p.id)
            .filter(|id| *id != 0)
            .collect()
    }
    /// Progressive CDN URL, or HLS resolved into the FULL ordered list of
    /// media chunk URLs (so the player can stream the whole track, chunk by
    /// chunk, without downloading it upfront).
    pub async fn resolve_stream(
        &self,
        track_id: i64,
        cid: &str,
        quality: &str,
    ) -> Result<StreamSource> {
        let t = self.track(track_id, cid).await?;
        self.resolve_stream_track(&t, cid, quality).await
    }

    /// Resolve stream directly from an existing Track object (skipping GET /tracks/{id}).
    pub async fn resolve_stream_track(
        &self,
        t: &Track,
        cid: &str,
        quality: &str,
    ) -> Result<StreamSource> {
        let url = self.resolve_transcoding(t, cid, quality).await?;
        if url.contains(".m3u8") {
            match self.resolve_hls(&url).await {
                Ok(s) => return Ok(s),
                Err(e) => {
                    crate::log!("hls resolution failed ({e}), falling back to progressive");
                    if let Ok(prog_url) = self.resolve_transcoding(t, cid, "progressive").await {
                        return Ok(StreamSource::Single(prog_url));
                    }
                }
            }
        }
        Ok(StreamSource::Single(url))
    }

    /// Same as resolve_stream but reuses an already fetched Track (no HLS resolution).
    pub async fn resolve_transcoding(&self, t: &Track, cid: &str, quality: &str) -> Result<String> {
        let media = t.media.clone().unwrap_or_default();
        let score = |tr: &Transcoding| -> i32 {
            let proto = tr
                .format
                .as_ref()
                .and_then(|f| f.protocol.clone())
                .unwrap_or_default();
            let preset = tr.preset.clone().unwrap_or_default();
            let is_highest = quality == "highest" || quality == "hls" || quality == "hq";
            let is_lowest = quality == "lowest";
            let is_hq = preset.contains("hq") || preset.contains("256") || preset.contains("high");
            let is_mp3 = preset.contains("mp3");
            let is_aac = preset.contains("aac");
            let is_opus = preset.contains("opus");

            if is_highest {
                if is_hq && is_aac {
                    120
                } else if is_hq {
                    110
                } else if is_mp3 && proto == "progressive" {
                    100
                } else if is_mp3 && proto == "hls" {
                    90
                } else if is_aac {
                    80
                } else {
                    30
                }
            } else if is_lowest {
                if is_opus {
                    100
                } else if is_mp3 && proto == "progressive" {
                    80
                } else if is_mp3 && proto == "hls" {
                    70
                } else {
                    40
                }
            } else {
                if is_mp3 && proto == "progressive" {
                    100
                } else if is_mp3 && proto == "hls" {
                    90
                } else if is_aac {
                    70
                } else {
                    30
                }
            }
        };
        let mut transcodings: Vec<&Transcoding> = media
            .transcodings
            .iter()
            .filter(|tr| tr.url.is_some())
            .collect();
        transcodings.sort_by_key(|tr| std::cmp::Reverse(score(tr)));

        for tr in transcodings {
            if let Some(url) = &tr.url {
                let sep = if url.contains('?') { '&' } else { '?' };
                if let Ok(resolved) = self
                    .get_json::<TranscodingUrl>(&format!("{url}{sep}client_id={cid}"))
                    .await
                {
                    if !resolved.url.is_empty() {
                        return Ok(resolved.url);
                    }
                }
            }
        }
        anyhow::bail!("no stream for track (quality={quality})")
    }
    /// Fetch an HLS playlist and return its init segment (fMP4) plus every
    /// media chunk URL in order. For plain mp3 HLS playlists `init` is None.
    async fn resolve_hls(&self, m3u8_url: &str) -> Result<StreamSource> {
        crate::console::http_request("GET", m3u8_url, None);
        let resp = self
            .http
            .get(m3u8_url)
            .header("Accept", "*/*")
            .send()
            .await?;
        let status = resp.status();
        let response_url = resp.url().to_string();
        let bytes = resp.bytes().await?;
        crate::console::http_response("GET", &response_url, status, &bytes);
        if !status.is_success() {
            anyhow::bail!("HLS playlist request failed: HTTP {status}");
        }
        let body = String::from_utf8_lossy(&bytes);
        let mut init: Option<String> = None;
        let mut chunks: Vec<String> = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("#EXT-X-MAP:URI=\"") {
                if let Some(url) = rest.split('"').next() {
                    init = Some(url.to_string());
                }
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let url = if line.starts_with("http") {
                line.to_string()
            } else {
                // relative URL: resolve against playlist path
                let dir = m3u8_url
                    .rsplit_once('/')
                    .map(|(d, _)| d.to_string())
                    .unwrap_or_default();
                format!("{dir}/{line}")
            };
            chunks.push(url);
        }
        if chunks.is_empty() {
            anyhow::bail!("hls playlist has no chunks");
        }
        crate::log!(
            "hls resolved: {} chunks, init={}",
            chunks.len(),
            init.is_some()
        );
        Ok(StreamSource::Chunks { init, chunks })
    }

    /// Waveform peak samples: JSON at waveform_url → 0..~140 amplitude per pixel.
    pub async fn waveform(&self, track_id: i64, cid: &str) -> Result<Vec<u8>> {
        let t = self.track(track_id, cid).await?;
        let wf_url = match t.waveform_url {
            Some(u) if !u.is_empty() => u,
            _ => anyhow::bail!("no waveform_url for track {track_id}"),
        };
        let url = if wf_url.contains('?') {
            wf_url
        } else {
            format!("{wf_url}?client_id={cid}")
        };
        let v: serde_json::Value = self.get_json(&url).await?;
        let samples = v
            .get("samples")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_u64().map(|n| n.min(255) as u8))
                    .collect::<Vec<u8>>()
            })
            .unwrap_or_default();
        Ok(samples)
    }

    /// Paginated comments for a track (timestamp in ms from track start).
    pub async fn comments(
        &self,
        track_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<(Vec<Comment>, Option<String>)> {
        let u = self.v2_url(
            &format!("/tracks/{track_id}/comments?limit={limit}&threaded=0"),
            cid,
        );
        let list: ApiList<Comment> = self.get_json(&u).await?;
        Ok((list.collection, list.next_href))
    }

    pub async fn comments_next(&self, next_href: &str) -> Result<(Vec<Comment>, Option<String>)> {
        let sep = if next_href.contains('?') { '&' } else { '?' };
        let u = format!("{next_href}{sep}app_locale={}", self.locale());
        let list: ApiList<Comment> = self.get_json(&u).await?;
        Ok((list.collection, list.next_href))
    }

    /// Full liked-tracks list with pagination (follows next_href until exhausted).
    pub async fn all_user_likes(
        &self,
        user_id: i64,
        cid: &str,
        limit: usize,
    ) -> Result<Vec<Track>> {
        let mut out: Vec<Track> = Vec::new();
        let mut next: Option<String> = Some(self.v2_url(
            &format!("/users/{user_id}/likes?limit={limit}&linked_partitioning=1"),
            cid,
        ));
        while let Some(url) = next.take() {
            crate::log!("likes page GET {}…", &url[..url.len().min(90)]);
            let t0 = std::time::Instant::now();
            let list: ApiList<LikeItem> = match self.get_json(&url).await {
                Ok(l) => l,
                Err(e) => {
                    crate::log!("likes page FAILED: {e}");
                    return Err(e);
                }
            };
            crate::log!(
                "likes page ok: {} items in {:.1}s",
                list.collection.len(),
                t0.elapsed().as_secs_f64()
            );
            for item in list.collection.into_iter() {
                if let Some(t) = item.into_track() {
                    if t.id != 0 {
                        out.push(t);
                    }
                }
            }
            next = list.next_href;
            if next.is_none() {
                break;
            }
        }
        crate::log!("likes done: {} total", out.len());
        Ok(out)
    }

    /// User's external web profiles (Instagram, Twitter, YouTube, Spotify, website, etc.)
    pub async fn user_web_profiles(&self, user_id: i64, cid: &str) -> Result<Vec<WebProfile>> {
        let u = self.v2_url(
            &format!("/users/soundcloud:users:{user_id}/web-profiles"),
            cid,
        );
        if let Ok(profiles) = self.get_json::<Vec<WebProfile>>(&u).await {
            return Ok(profiles);
        }
        let list: ApiList<WebProfile> = self.get_json(&u).await?;
        Ok(list.collection)
    }

    /// Create a new playlist (optionally containing an initial track).
    pub async fn create_playlist(
        &self,
        access: &str,
        title: &str,
        sharing: &str,
        track_id: Option<i64>,
    ) -> Result<Playlist> {
        if let Some(bridge) = &self.bridge {
            match bridge
                .create_playlist(access, title, sharing, track_id)
                .await
            {
                Ok(pl) => return Ok(pl),
                Err(e) => crate::log!("bridge create_playlist failed: {e}; trying http fallback"),
            }
        }
        let mut pl_map = serde_json::Map::new();
        pl_map.insert("title".to_string(), serde_json::json!(title));
        pl_map.insert("sharing".to_string(), serde_json::json!(sharing));
        if let Some(tid) = track_id {
            pl_map.insert(
                "tracks".to_string(),
                serde_json::json!([{"id": tid, "urn": format!("soundcloud:tracks:{tid}")}]),
            );
        }
        let body = serde_json::json!({
            "playlist": pl_map
        });

        let url = format!("{V2_API}/playlists?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .post(&url)
                    .header("Authorization", format!("OAuth {access}"))
                    .json(&body)
            })
            .await?;

        let st = resp.status();
        if st.is_success() {
            let pl = resp.json::<Playlist>().await?;
            Ok(pl)
        } else {
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("create playlist failed (HTTP {st}): {err}");
        }
    }

    /// A playlist's track ids exactly as stored (stubs included, nothing
    /// hydrated) and its track_count: the input of a whole-list PUT. Skipping
    /// hydration keeps the read-to-PUT window short.
    async fn playlist_ids_for_write(&self, playlist_id: i64) -> Result<(Vec<i64>, usize)> {
        let u = self.v2_url(&format!("/playlists/{playlist_id}"), CLIENT_ID);
        let pl: Playlist = self.get_json(&u).await?;
        let expected = pl.track_count.unwrap_or(0) as usize;
        let ids = pl
            .tracks
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.id)
            .filter(|id| *id != 0)
            .collect();
        Ok((ids, expected))
    }

    /// Add a track to an existing playlist, preserving all existing tracks.
    pub async fn add_track_to_playlist(
        &self,
        access: &str,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            if let Ok((existing, _)) = self.playlist_ids_for_write(playlist_id).await {
                match bridge
                    .add_track_to_playlist(access, playlist_id, track_id, existing)
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err(e) => crate::log!(
                        "bridge add_track_to_playlist failed: {e}; trying http fallback"
                    ),
                }
            }
        }
        // The Android app adds one track in place (POST /playlists/{urn}/tracks),
        // which can't lose any; the whole-list PUT is the fallback.
        let path = format!("/playlists/soundcloud:playlists:{playlist_id}/tracks");
        let body = serde_json::json!({"urn": format!("soundcloud:tracks:{track_id}")});
        match self
            .mobile_write_ok(
                "add to playlist",
                reqwest::Method::POST,
                access,
                &path,
                Some(body),
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("add to playlist via mobile api: {e}; trying v2"),
        }
        // The PUT below replaces the whole track list, so it must start from
        // the complete current list: if the playlist can't be read (network,
        // private playlist) or comes back short, stop instead of overwriting
        // the playlist with just this one track.
        let (mut track_ids, expected) = self
            .playlist_ids_for_write(playlist_id)
            .await
            .map_err(|e| anyhow::anyhow!("couldn't read the playlist: {e}"))?;
        if track_ids.len() < expected {
            anyhow::bail!(
                "playlist returned {} of {} tracks; not saving to avoid losing tracks",
                track_ids.len(),
                expected
            );
        }

        if !track_ids.contains(&track_id) {
            track_ids.push(track_id);
        }

        let body = serde_json::json!({
            "playlist": {
                "tracks": track_ids
            }
        });
        let url_put = format!("{V2_API}/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .put(&url_put)
                    .header("Authorization", format!("OAuth {access}"))
                    .json(&body)
            })
            .await?;

        if resp.status().is_success() {
            return Ok(());
        }

        // Secondary fallback: mobile API /tracks add endpoint
        let track_urn = format!("soundcloud:tracks:{track_id}");
        let body_post = serde_json::json!({
            "add": track_urn
        });
        let url_post = format!("{V2_API}/playlists/{playlist_id}/tracks?client_id={CLIENT_ID}");
        let resp2 = self
            .send_browser_write(|| {
                self.http
                    .post(&url_post)
                    .header("Authorization", format!("OAuth {access}"))
                    .json(&body_post)
            })
            .await?;

        if resp2.status().is_success() {
            Ok(())
        } else {
            let err = resp2.text().await.unwrap_or_default();
            anyhow::bail!("add track to playlist failed: {err}");
        }
    }

    /// Remove a track from an existing playlist.
    pub async fn remove_track_from_playlist(
        &self,
        access: &str,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            if let Ok((existing, _)) = self.playlist_ids_for_write(playlist_id).await {
                match bridge
                    .remove_track_from_playlist(access, playlist_id, track_id, existing)
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err(e) => crate::log!(
                        "bridge remove_track_from_playlist failed: {e}; trying http fallback"
                    ),
                }
            }
        }
        // in place, like the Android app: DELETE /playlists/{urn}/tracks/{urn}
        let path = format!(
            "/playlists/soundcloud:playlists:{playlist_id}/tracks/soundcloud:tracks:{track_id}"
        );
        match self
            .mobile_write_ok(
                "remove from playlist",
                reqwest::Method::DELETE,
                access,
                &path,
                None,
            )
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => crate::log!("remove from playlist via mobile api: {e}; trying v2"),
        }
        let (all_ids, expected) = self.playlist_ids_for_write(playlist_id).await?;
        // same guard as adding: never PUT a list shorter than the playlist
        if all_ids.len() < expected {
            anyhow::bail!(
                "playlist returned {} of {} tracks; not saving to avoid losing tracks",
                all_ids.len(),
                expected
            );
        }
        let track_ids: Vec<i64> = all_ids.into_iter().filter(|id| *id != track_id).collect();

        let body = serde_json::json!({
            "playlist": {
                "tracks": track_ids
            }
        });
        let url_put = format!("{V2_API}/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .put(&url_put)
                    .header("Authorization", format!("OAuth {access}"))
                    .json(&body)
            })
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("remove track from playlist failed: {err}");
        }
    }

    /// Delete a playlist owned by the user.
    pub async fn delete_playlist(&self, access: &str, playlist_id: i64) -> Result<()> {
        if let Some(bridge) = &self.bridge {
            match bridge.delete_playlist(access, playlist_id).await {
                Ok(()) => return Ok(()),
                Err(e) => crate::log!("bridge delete_playlist failed: {e}; trying http fallback"),
            }
        }
        let url = format!("{V2_API}/playlists/{playlist_id}?client_id={CLIENT_ID}");
        let resp = self
            .send_browser_write(|| {
                self.http
                    .delete(&url)
                    .header("Authorization", format!("OAuth {access}"))
            })
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("delete playlist failed: HTTP {}", resp.status())
        }
    }
}

/// A refused answer, rebuilt after its body was read (or never sent).
fn refused_response(status: reqwest::StatusCode, body: &str) -> reqwest::Response {
    http::Response::builder()
        .status(status)
        .body(body.to_string())
        .map(reqwest::Response::from)
        .expect("a status and a text body")
}

/// Rebuild a consumed JSON write response so callers retain its status,
/// headers and body after the debug logger has recorded the full payload.
fn response_with_body(
    status: reqwest::StatusCode,
    headers: http::HeaderMap,
    body: String,
) -> Result<reqwest::Response> {
    let mut response = http::Response::builder().status(status).body(body)?;
    *response.headers_mut() = headers;
    Ok(reqwest::Response::from(response))
}

/// "POST" / "DELETE" ... of a request about to go (for the debug log).
fn method_label(req: &reqwest::RequestBuilder) -> String {
    req.try_clone()
        .and_then(|r| r.build().ok())
        .map(|r| r.method().to_string())
        .unwrap_or_else(|| "?".into())
}

/// The error text of a web-API write DataDome answers with a captcha: a
/// browser check Wavify's own requests can't pass.
pub const BROWSER_CHECK: &str = "SoundCloud's bot protection wants a browser check for this";

/// The error text of a write refused again right after SoundCloud's check
/// was passed: the write is retried once (see the ui's `after_check`), and
/// this is what's left when that fails too.
pub const CHECK_PASSED: &str = "SoundCloud's check was passed, but it still refused this";

/// The error text of a web-API write DataDome refuses from this network
/// outright (a block, no captcha to solve).
pub const NETWORK_BLOCKED: &str = "SoundCloud's bot protection is blocking this network for now";

/// The quick reactions the Android app offers, in its order: fire, clap,
/// pleading face. The API names a reaction by its emoji codepoint.
pub const QUICK_REACTIONS: [(&str, &str); 3] = [("1f525", "🔥"), ("1f44f", "👏"), ("1f979", "🥹")];

/// Interaction type of a waveform reaction (sc:interactiontype:trackreaction).
const TRACK_REACTION_TYPE: &str = "sc:interactiontype:trackreaction";
const REACTION_VALUE_PREFIX: &str = "sc:interactiontypevalue:";

/// One reaction pinned to a second of a track.
#[derive(Debug, Clone)]
pub struct WaveReaction {
    pub second: u64,
    /// Emoji codepoint in hex ("1f525").
    pub codepoint: String,
}

impl WaveReaction {
    /// The emoji itself, from the codepoint ("1f525" -> 🔥).
    pub fn emoji(&self) -> String {
        u32::from_str_radix(&self.codepoint, 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default()
    }
}

/// A posted comment from either API: v2 answers with a numeric `id`, the
/// mobile API with an `urn` ("soundcloud:comments:123").
fn comment_from_json(v: &serde_json::Value) -> Comment {
    let mut c: Comment = serde_json::from_value(v.clone()).unwrap_or_default();
    if c.id == 0 {
        c.id = v
            .get("urn")
            .and_then(|u| u.as_str())
            .and_then(|u| u.rsplit(':').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    c
}

pub fn urlencoding_lite(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            &0x20 => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
