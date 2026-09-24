use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CLIENT_ID: &str = "hqiIMkQLy7B8EFrfa2k0zxNdZKDnBe2s";
pub const CLIENT_SECRET: &str = "VJJYdzxTpcMx3FM81yA5BgGnzBXX7Yk3";

pub const AUTH_API: &str = "https://api-auth.soundcloud.com";
pub const V2_API: &str = "https://api-v2.soundcloud.com";
pub const MOBILE_API: &str = "https://api-mobile.soundcloud.com";
/// GraphQL endpoint of the mobile app (quick reactions and more).
pub const GRAPH_API: &str = "https://graph.soundcloud.com/graphql";
pub const USER_AGENT: &str = "SoundCloud-Android/2026.06.03 (Android 16; VK 358040)";
pub const APP_VERSION: &str = "2026.06.03";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    /// Your own proxy, for everything Wavify loads:
    /// protocol://host:port or protocol://user:pass@host:port.
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub client_id_override: Option<String>,
    /// Streaming quality: None (automatic), "highest" or "lowest".
    #[serde(default)]
    pub audio_quality: Option<String>,
    /// Downloads: "highest" (default) or "lowest".
    #[serde(default)]
    pub download_quality: Option<String>,
    #[serde(default)]
    pub hide_scrollbars: bool,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub volume: Option<f32>,
    /// Listening activity: the playing track on your Discord profile.
    #[serde(default = "default_true")]
    pub discord_rpc: bool,
    #[serde(default)]
    pub prefer_artist_from_name: bool,
    /// Unlock through a proxy: tracks unavailable in this country play
    /// through public proxies from elsewhere.
    #[serde(default)]
    pub bypass_unavailable: bool,
    /// Unlock through YouTube Music: tracks SoundCloud can't play in full
    /// (Go+, region-blocked) play in YouTube Music's own web player.
    #[serde(default = "default_true")]
    pub youtube_music: bool,
    /// Zoom level, 0.7 to 1.3.
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Private session: until then (ms since the epoch) nothing shows what
    /// you play, and nothing goes into your listening history.
    #[serde(default)]
    pub private_until_ms: Option<u64>,
    /// Crossfade between tracks, seconds (0: off).
    #[serde(default)]
    pub crossfade_secs: u8,
    #[serde(default)]
    pub normalize_volume: bool,
    #[serde(default)]
    pub volume_level: VolumeLevel,
    #[serde(default)]
    pub mono_audio: bool,
    #[serde(default)]
    pub equalizer: bool,
    /// Gains of the equalizer's six bands, dB.
    #[serde(default)]
    pub eq_gains: [f32; 6],
    /// Keep playing similar tracks when the queue ends.
    #[serde(default = "default_true")]
    pub autoplay: bool,
    #[serde(default)]
    pub shuffle_style: ShuffleStyle,
    #[serde(default)]
    pub open_at_login: OpenAtLogin,
    /// The window's close button minimizes it instead of quitting.
    #[serde(default)]
    pub close_minimizes: bool,
    #[serde(default)]
    pub compact_library: bool,
    /// Keep what's played on this computer: the audio, its cover and
    /// waveform banner, the waveform, comments and the track's details. Off,
    /// every track streams from SoundCloud (downloads stay either way).
    #[serde(default)]
    pub cache_tracks: bool,
    /// How Your Library orders followed artists.
    #[serde(default)]
    pub artist_sort: ArtistSort,
    /// Check the GitHub Releases feed on startup and from Settings.
    #[serde(default = "default_true")]
    pub check_updates: bool,
    /// The release declined with "Remind me after new version" enabled.
    #[serde(default)]
    pub ignored_update_version: Option<String>,
}

/// Your Library's order of followed artists.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtistSort {
    /// Most followers first.
    Subscribers,
    /// By name.
    Alphabet,
    /// Most tracks first.
    #[default]
    Tracks,
}

/// Normalization's target loudness.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VolumeLevel {
    Loud,
    #[default]
    Normal,
    Quiet,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShuffleStyle {
    /// Tracks by one artist are kept apart.
    #[default]
    FewerRepeats,
    Standard,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpenAtLogin {
    #[default]
    No,
    Minimized,
    Yes,
}

fn default_true() -> bool {
    true
}

fn default_zoom() -> f32 {
    1.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            proxy: None,
            client_id_override: None,
            audio_quality: None,
            download_quality: None,
            hide_scrollbars: false,
            offline_mode: false,
            volume: None,
            discord_rpc: true,
            prefer_artist_from_name: false,
            bypass_unavailable: false,
            youtube_music: true,
            zoom: 1.0,
            private_until_ms: None,
            crossfade_secs: 0,
            normalize_volume: false,
            volume_level: VolumeLevel::Normal,
            mono_audio: false,
            equalizer: false,
            eq_gains: [0.0; 6],
            autoplay: true,
            shuffle_style: ShuffleStyle::FewerRepeats,
            open_at_login: OpenAtLogin::No,
            close_minimizes: false,
            compact_library: false,
            cache_tracks: false,
            artist_sort: ArtistSort::Tracks,
            check_updates: true,
            ignored_update_version: None,
        }
    }
}

impl Settings {
    /// A private session is on (it ends by itself after 6 hours).
    pub fn private_session(&self) -> bool {
        self.private_until_ms.is_some_and(|t| now_ms() < t)
    }
}

/// Is this a proxy URL Wavify can use: http, https, socks5 or socks5h,
/// a host and a port, optionally user:pass@?
pub fn valid_proxy(s: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(s.trim()) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.port().is_some()
}

/// "Open Wavify automatically after you log into the computer": the
/// Windows Run entry (HKCU, no admin rights needed).
#[cfg(windows)]
pub fn set_open_at_login(mode: OpenAtLogin) -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")?;
    match mode {
        OpenAtLogin::No => match run.delete_value("Wavify") {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        OpenAtLogin::Minimized | OpenAtLogin::Yes => {
            let exe = std::env::current_exe()?;
            let flag = if mode == OpenAtLogin::Minimized {
                " --minimized"
            } else {
                ""
            };
            run.set_value("Wavify", &format!("\"{}\"{flag}", exe.to_string_lossy()))
        }
    }
}

#[cfg(not(windows))]
pub fn set_open_at_login(_mode: OpenAtLogin) -> std::io::Result<()> {
    Ok(())
}

/// The app's own folder of a kind (config or cache). The first run as
/// Wavify moves the LiteCloud folder over, so settings, sign-in, history and
/// downloads carry on. While a LiteCloud still runs its folder is in use and
/// can't move: then it keeps being used, and moves on a later start.
fn app_dir(kind: fn(&directories::ProjectDirs) -> &std::path::Path) -> PathBuf {
    let new = directories::ProjectDirs::from("com", "wavify", "Wavify")
        .map(|d| kind(&d).to_path_buf())
        .expect("no home dir");
    if !new.exists() {
        if let Some(old) = directories::ProjectDirs::from("com", "litecloud", "LiteCloud")
            .map(|d| kind(&d).to_path_buf())
            .filter(|o| o.exists())
        {
            if let Some(parent) = new.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if std::fs::rename(&old, &new).is_err() {
                return old;
            }
        }
    }
    std::fs::create_dir_all(&new).ok();
    new
}

pub fn config_dir() -> PathBuf {
    // decided once per run: every settings read comes through here
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| app_dir(directories::ProjectDirs::config_dir))
        .clone()
}

pub fn token_path() -> PathBuf {
    config_dir().join("token.json")
}

pub fn cookies_path() -> PathBuf {
    config_dir().join("session_cookies.json")
}

pub fn auth_error_path() -> PathBuf {
    config_dir().join("auth_error.json")
}

pub fn auth_pending_path() -> PathBuf {
    config_dir().join("auth_pending.json")
}

pub fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

pub fn playback_path() -> PathBuf {
    config_dir().join("playback.json")
}

pub fn stories_path() -> PathBuf {
    config_dir().join("stories.json")
}

pub fn stories_read_path() -> PathBuf {
    config_dir().join("stories_read.json")
}

pub fn offline_playlists_path() -> PathBuf {
    config_dir().join("offline_playlists.json")
}

/// Metadata of downloaded tracks/playlists (see ui::OfflineStore).
pub fn offline_store_path() -> PathBuf {
    config_dir().join("offline_store.json")
}

/// Go+ tracks whose cached audio is a full version from another source.
pub fn full_versions_path() -> PathBuf {
    config_dir().join("full_versions.json")
}

/// Last loaded Liked Tracks, so the library opens without a network.
pub fn library_cache_path() -> PathBuf {
    config_dir().join("library_cache.json")
}

/// Playback speed per track id, for tracks not played at 1.0x.
pub fn track_speeds_path() -> PathBuf {
    config_dir().join("track_speeds.json")
}

pub fn cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| app_dir(directories::ProjectDirs::cache_dir))
        .clone()
}

/// A subfolder of the cache, created on first use. These paths are asked
/// for per row and per frame, so the folder is made once, not every time.
fn cache_subdir(cell: &'static std::sync::OnceLock<PathBuf>, name: &str) -> PathBuf {
    cell.get_or_init(|| {
        let dir = cache_dir().join(name);
        std::fs::create_dir_all(&dir).ok();
        dir
    })
    .clone()
}

pub fn audio_cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    cache_subdir(&DIR, "audio")
}

pub fn cached_audio_path(track_id: i64) -> PathBuf {
    audio_cache_dir().join(format!("{track_id}.mp3"))
}

pub fn waveform_cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    cache_subdir(&DIR, "waveforms")
}

pub fn cached_waveform_path(track_id: i64) -> PathBuf {
    waveform_cache_dir().join(format!("{track_id}.bin"))
}

pub fn comments_cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    cache_subdir(&DIR, "comments")
}

pub fn cached_comments_path(track_id: i64) -> PathBuf {
    comments_cache_dir().join(format!("{track_id}.json"))
}

/// A played track's details, kept with its audio (Cache tracks locally).
pub fn track_info_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    cache_subdir(&DIR, "tracks")
}

pub fn cached_track_info_path(track_id: i64) -> PathBuf {
    track_info_dir().join(format!("{track_id}.json"))
}

pub fn artwork_cache_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    cache_subdir(&DIR, "artwork")
}

pub fn cached_artwork_path(url: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hex::encode(hasher.finalize());
    artwork_cache_dir().join(format!("{hash}.img"))
}

/// Register sc:// protocol handler (HKCU, no admin required).
#[cfg(windows)]
pub fn register_sc_protocol() -> std::io::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;
    let exe = std::env::current_exe()?;
    let exe_str = exe.to_string_lossy().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(r"Software\Classes\sc")?;
    key.set_value("", &"URL:SoundCloud Auth")?;
    key.set_value("URL Protocol", &"")?;
    let (cmd, _) = hkcu.create_subkey(r"Software\Classes\sc\shell\open\command")?;
    cmd.set_value("", &format!("\"{}\" \"%1\"", exe_str))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn register_sc_protocol() -> std::io::Result<()> {
    Ok(())
}

pub const REDIRECT_URI: &str = "https://soundcloud.com/auth_mobile_callback";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub scope: String,
    pub expires_at_ms: u64,
}

impl Token {
    pub fn from_oauth(resp: &OAuthTokenResponse) -> Token {
        Token {
            access_token: resp.access_token.clone(),
            refresh_token: resp.refresh_token.clone().unwrap_or_default(),
            scope: resp.scope.clone().unwrap_or_default(),
            expires_at_ms: now_ms() + resp.expires_in.unwrap_or(2159999) as u64 * 1000,
        }
    }

    pub fn expired(&self) -> bool {
        now_ms() > self.expires_at_ms.saturating_sub(60_000)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Region sent as app_locale. A RwLock, not a OnceLock: Settings > Save
/// changes or clears it while the app runs.
static SETTINGS_REGION: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn set_settings_region(region: Option<String>) {
    if let Ok(mut r) = SETTINGS_REGION.write() {
        *r = region;
    }
}

pub fn settings_region() -> Option<String> {
    SETTINGS_REGION.read().ok().and_then(|r| r.clone())
}

pub fn save_settings(s: &Settings) {
    if let Ok(j) = serde_json::to_string_pretty(s) {
        // Write a temp file and rename it over: background tasks load
        // settings.json at any time, and a truncate-then-write could hand
        // them an empty file (= default settings, no proxy).
        let path = config_dir().join("settings.json");
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &j).is_err() || std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::write(&path, j);
        }
    }
}

impl Settings {
    pub fn load() -> Settings {
        let p = config_dir().join("settings.json");
        if let Ok(s) = std::fs::read_to_string(&p) {
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            Settings::default()
        }
    }

    pub fn save(&self) {
        save_settings(self)
    }
}
