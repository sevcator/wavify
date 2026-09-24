use crate::api::*;
use crate::auth::*;
use crate::config::*;
use crate::discord_rpc::{ActivityData, DiscordRpcHandle};
use crate::player;
use crate::player::PlayerCommand;
use anyhow::Result;
use iced::keyboard::{self, key::Named, Key};
use iced::widget::{
    button, column, container, horizontal_space, image, row, scrollable, slider, stack, text_input,
    vertical_space,
};
use serde::{Deserialize, Serialize};
// The `text` module (Style, Wrapping, Shaping, ...); the `text()` constructor
// is the local wrapper below.
use iced::widget::canvas;
use iced::widget::text::{self};
use iced::{
    border, clipboard, mouse, Background, Border, Color, Element, Length, Padding, Point,
    Rectangle, Shadow, Size, Subscription, Task, Vector,
};

mod settings_view;
use settings_view::Pref;

// Spotify Dark Minimal — Design Tokens
const BG: Color = Color::from_rgb(0.0, 0.0, 0.0); // #000000 pure black window canvas
const BG_MUTED: Color = Color::from_rgb(0.071, 0.071, 0.071); // #121212 Spotify island panel background
const BG_CARD: Color = Color::from_rgb(0.122, 0.122, 0.122); // #1F1F1F --background-elevated-base
const BG_HOVER: Color = Color::from_rgb(0.165, 0.165, 0.165); // #2A2A2A --background-elevated-highlight
const BG_SIDE: Color = Color::from_rgb(0.071, 0.071, 0.071); // #121212 sidebar panel = island
const BG_ELEV: Color = Color::from_rgb(0.122, 0.122, 0.122); // #1F1F1F --background-elevated-base
const BG_PLAYER: Color = Color::from_rgb(0.0, 0.0, 0.0); // #000000 player bar (pure black)
const BG_INPUT: Color = Color::from_rgb(0.141, 0.141, 0.141); // #242424 top-bar search field

const ORANGE: Color = Color::from_rgb(1.0, 0.333, 0.0); // #FF5500 SoundCloud
const ORANGE_DIM: Color = Color::from_rgb(0.85, 0.30, 0.02);

const TEXT: Color = Color::from_rgb(1.0, 1.0, 1.0); // #FFFFFF primary text
const TEXT_DIM: Color = Color::from_rgb(0.702, 0.702, 0.702); // #B3B3B3 secondary text
const TEXT_MUTED: Color = Color::from_rgb(0.486, 0.486, 0.486); // #7C7C7C --essential-subdued

// Spotify's tinted surfaces: white at 10% / 14% / 21% over the page background.
const BG_TINT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.10); // --background-tinted-base
/// The green check of downloaded tracks and playlists.
/// The "on this computer" mark (downloads, cached tracks): the accent.
const DOWNLOADED: Color = ORANGE;
const BG_TINT_HI: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.14); // --background-tinted-highlight

// Title-bar controls share one hit-box so every button in the bar is the same
// size (Spotify's --encore-control-size-smaller is 32px).
const TITLEBAR_CTRL: f32 = 32.0;
const TITLEBAR_ICON: u16 = 14;
/// Segoe UI Bold for Spotify's bold labels (popover title, footer buttons).
const UI_BOLD: iced::Font = iced::Font {
    family: iced::font::Family::Name("Segoe UI"),
    weight: iced::font::Weight::Bold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

// Player bar geometry, shared by the bar and the "Add to playlist" popover
// that opens from its save button.
const PB_PAD_X: f32 = 16.0;
const PB_H: f32 = 76.0;
const PB_GAP: f32 = 6.0;
const PB_BTN: f32 = 28.0; // square hit box for every icon button
const PB_COVER: f32 = 40.0;
const PB_INFO_W: f32 = 158.0; // title/artist are fitted to this in pixels
/// Left edge of the save button: cover, title block, shuffle and repeat come first.
const PB_SAVE_X: f32 = PB_PAD_X + PB_COVER + PB_GAP + PB_INFO_W + PB_GAP + 2.0 * (PB_BTN + PB_GAP);
/// Left edge of the 16px save glyph inside its 28px hit box.
const PB_SAVE_ICON_X: f32 = PB_SAVE_X + (PB_BTN - 16.0) / 2.0;
/// Left and right blocks of the bar (the speed pill opens the right one).
const PB_SIDE_W: f32 = 340.0;
/// The waveform's distance from either window edge: bar padding, a side
/// block, the row gap, a time label and its gap (see view_waveform).
const PB_WAVE_INSET: f32 = PB_PAD_X + PB_SIDE_W + 16.0 + 44.0 + 10.0;
/// Width of the right block's controls (speed, queue, volume), flush right.
const PB_RIGHT_W: f32 = 288.0;
/// Speed pill: widest label "1.25x ⚡" (~47px) + side padding.
const PB_SPEED_W: f32 = 56.0;
/// Bar popups sit 8px above the button they open from, like Spotify's
/// "top-start" popovers: bar centre + half a hit box + 8.
const PB_POP_BOTTOM: f32 = PB_H / 2.0 + PB_BTN / 2.0 + 8.0;

/// Height of the player bar's waveform, and the corner radius of the visual
/// banner behind it (logical px).
const PB_WAVE_H: f32 = 48.0;
const PB_WAVE_RADIUS: f32 = 6.0;
/// The banner is darkened to this share of its brightness, so the bars on
/// top of it stay readable.
const PB_WAVE_DIM: f32 = 0.32;

/// Playback speed range of the speed popup, the wheel and the player.
const SPEED_MIN: f32 = 0.1;
const SPEED_MAX: f32 = 2.0;
/// One wheel notch over the speed pill.
const SPEED_STEP: f32 = 0.05;

pub const FA_SOLID: iced::Font = iced::Font {
    family: iced::font::Family::Name("Font Awesome 6 Free"),
    weight: iced::font::Weight::Black,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};
pub const FA_REGULAR: iced::Font = iced::Font {
    family: iced::font::Family::Name("Font Awesome 6 Free"),
    weight: iced::font::Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};
pub const FA_BRANDS: iced::Font = iced::Font {
    family: iced::font::Family::Name("Font Awesome 6 Brands"),
    weight: iced::font::Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub mod icons {
    // Navigation & Window Controls
    pub const CHEVRON_LEFT: &str = "\u{f053}";
    pub const CHEVRON_RIGHT: &str = "\u{f054}";
    pub const CHEVRON_UP: &str = "\u{f077}";
    pub const CHEVRON_DOWN: &str = "\u{f078}";
    pub const CARET_UP: &str = "\u{f0d8}";
    pub const CARET_DOWN: &str = "\u{f0d7}";
    pub const HOUSE: &str = "\u{f015}";
    pub const MINUS: &str = "\u{f068}";
    pub const SQUARE: &str = "\u{f0c8}";
    pub const XMARK: &str = "\u{f00d}";

    // Sidebar & Library
    pub const BARS: &str = "\u{f0c9}";
    pub const PLUS: &str = "\u{f067}";
    pub const TRASH: &str = "\u{f1f8}";

    // Player Bar Transport & Audio
    pub const PLAY: &str = "\u{f04b}";
    pub const PAUSE: &str = "\u{f04c}";
    pub const SHUFFLE: &str = "\u{f074}";
    pub const REPEAT: &str = "\u{f363}";
    pub const HEART: &str = "\u{f004}";
    pub const QUEUE: &str = "\u{f0ca}"; // list-ul
    pub const ELLIPSIS: &str = "\u{f141}";

    // Volume Speakers
    pub const VOLUME_XMARK: &str = "\u{f6a9}";
    pub const VOLUME_OFF: &str = "\u{f026}";
    pub const VOLUME_LOW: &str = "\u{f027}";
    pub const VOLUME_HIGH: &str = "\u{f028}";

    // Track Actions & Context Menus
    pub const COMPACT_DISC: &str = "\u{f51f}";
    pub const RADIO: &str = "\u{f519}"; // tower-broadcast
    pub const COPY: &str = "\u{f0c5}";
    pub const FOLDER_PLUS: &str = "\u{f65e}";
    pub const COMMENTS: &str = "\u{f075}";
    pub const REPOST: &str = "\u{f079}"; // retweet
    pub const DOWNLOAD: &str = "\u{f019}";
    pub const CHECK_CIRCLE: &str = "\u{f058}";
    /// circle-arrow-down: Spotify's "downloaded" mark
    pub const CIRCLE_DOWN: &str = "\u{f0ab}";
    pub const EXTERNAL_LINK: &str = "\u{f08e}"; // arrow-up-right-from-square
    pub const USER: &str = "\u{f007}";
    pub const GEAR: &str = "\u{f013}";
    pub const LOGOUT: &str = "\u{f2f5}"; // right-from-bracket

    // Notifications / Toasts
    pub const CHECK: &str = "\u{f00c}";
    pub const INFO: &str = "\u{f05a}";

    // Brands (FA_BRANDS)
    pub const BRAND_INSTAGRAM: &str = "\u{f16d}";
    pub const BRAND_TWITTER: &str = "\u{e61b}";
    pub const BRAND_YOUTUBE: &str = "\u{f167}";
    pub const BRAND_SPOTIFY: &str = "\u{f1bc}";
    pub const BRAND_SOUNDCLOUD: &str = "\u{f1be}";
    pub const BRAND_FACEBOOK: &str = "\u{f39e}";
    pub const BRAND_TIKTOK: &str = "\u{e07b}";
    pub const BRAND_BANDCAMP: &str = "\u{f2d5}";
    pub const BRAND_APPLE: &str = "\u{f179}";
    pub const BRAND_PATREON: &str = "\u{f3d9}";
    pub const BRAND_TWITCH: &str = "\u{f1e8}";
    pub const BRAND_TUMBLR: &str = "\u{f173}";
    pub const BRAND_VIMEO: &str = "\u{f27d}";
    pub const BRAND_REDDIT: &str = "\u{f1a1}";
    pub const BRAND_DISCORD: &str = "\u{f392}";
    pub const BRAND_LINKEDIN: &str = "\u{f08c}";
}

const GREEN_LIKE: Color = Color::from_rgb(0.122, 0.745, 0.404); // #1FBE67
const HEART: Color = Color::from_rgb(0.957, 0.255, 0.431); // #F44171
const DANGER_RED: Color = Color::from_rgb(0.92, 0.22, 0.22);

#[cfg(windows)]
pub mod screen {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(
            ui_action: u32,
            ui_param: u32,
            pv_param: *mut Rect,
            f_win_ini: u32,
        ) -> i32;
        fn GetSystemMetrics(n_index: i32) -> i32;
        fn GetDC(hwnd: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
        fn ReleaseDC(hwnd: *mut core::ffi::c_void, hdc: *mut core::ffi::c_void) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetDeviceCaps(hdc: *mut core::ffi::c_void, index: i32) -> i32;
    }

    /// Screen pixels per 96-DPI pixel, as this process sees the screen: 1.0
    /// while it isn't DPI-aware (Windows then scales for it), the system
    /// scale once it is.
    pub fn dpi_scale() -> f32 {
        let dpi = unsafe {
            let dc = GetDC(std::ptr::null_mut());
            if dc.is_null() {
                return 1.0;
            }
            let dpi = GetDeviceCaps(dc, 88 /* LOGPIXELSX */);
            ReleaseDC(std::ptr::null_mut(), dc);
            dpi
        };
        if dpi > 0 {
            dpi as f32 / 96.0
        } else {
            1.0
        }
    }

    pub fn get_work_area() -> (f32, f32, f32, f32) {
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe {
            SystemParametersInfoW(48 /* SPI_GETWORKAREA */, 0, &mut rect, 0)
        };
        if ok != 0 && rect.right > rect.left && rect.bottom > rect.top {
            (
                rect.left as f32,
                rect.top as f32,
                (rect.right - rect.left) as f32,
                (rect.bottom - rect.top) as f32,
            )
        } else {
            let w = unsafe {
                GetSystemMetrics(0 /* SM_CXSCREEN */)
            };
            let h = unsafe {
                GetSystemMetrics(1 /* SM_CYSCREEN */)
            };
            if w > 0 && h > 0 {
                (0.0, 0.0, w as f32, h as f32)
            } else {
                (0.0, 0.0, 1440.0, 900.0)
            }
        }
    }
}

#[cfg(not(windows))]
pub mod screen {
    pub fn get_work_area() -> (f32, f32, f32, f32) {
        (0.0, 0.0, 1440.0, 900.0)
    }

    pub fn dpi_scale() -> f32 {
        1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub created_at: std::time::Instant,
    /// Text button on the right ("Change" after a quick save) and its message.
    pub action: Option<(String, Box<Message>)>,
}

/// Spotify's "Add to playlist" popover over the player bar's save button.
/// Ticks are staged here and written only on "Done" (or "New playlist").
#[derive(Debug, Clone)]
pub struct AddPopover {
    pub track: Track,
    pub query: String,
    /// "New playlist" pressed: the name being typed for it.
    pub new_name: Option<String>,
    pub liked: bool,
    pub liked_was: bool,
    pub picked: std::collections::HashSet<i64>,
    pub picked_was: std::collections::HashSet<i64>,
}

/// Outcome of committing the popover, so failures can be undone locally.
#[derive(Debug, Clone)]
pub struct SaveReport {
    pub track_id: i64,
    pub like: Option<bool>,
    pub like_ok: bool,
    pub playlists_touched: bool,
    pub errors: Vec<String>,
}

/// A track list that an entity page plays, downloads and queues as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    Playlist(i64),
    Profile(i64),
    Liked,
}

/// What the "..." side panel is open for.
#[derive(Debug, Clone)]
pub enum ActionMenu {
    Track(Track),
    Collection(Collection),
}

/// What a track does when YouTube Music has no match for it either.
#[derive(Debug, Clone, Copy)]
pub enum YtFallback {
    /// Go+: SoundCloud's 30 s preview.
    Preview,
    /// Unavailable here: skip it.
    Skip,
}

/// Where a menu drops down from: the button that opened it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAnchor {
    /// A track row's "...": track id and row index.
    Row(i64, usize),
    /// A track row's save ("+") button: track id and row index.
    RowSave(i64, usize),
    /// The open page's own "..." (track, playlist, artist, Liked Tracks).
    Page,
    /// The quick actions of the Comments & Details panel.
    Inspector,
    /// A story's add-to-playlist button.
    Story,
    /// A story's "...".
    StoryMore,
    /// The player bar's save button.
    PlayerBar,
}

/// Player bar link under the pointer (it is underlined, like Spotify).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbLink {
    Title,
    Artist,
    /// The queue panel's Now Playing card.
    QueueTitle,
    QueueArtist,
    /// An open story: who posted it, its title and artist.
    StoryUser,
    StoryTitle,
    StoryArtist,
    /// The open playlist's author, under its title.
    PlaylistAuthor,
    /// A playlist card's owner line; the key tells the cards apart.
    Owner(u64),
}

/// What offline mode shows without a network: metadata of everything
/// downloaded, plus the last known library and sidebar playlists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfflineStore {
    /// Every track downloaded for offline, newest first.
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Downloaded playlists and albums, with their track lists.
    #[serde(default)]
    pub playlists: Vec<PlaylistDetail>,
    #[serde(default)]
    pub my_playlists: Vec<Playlist>,
    #[serde(default)]
    pub liked_playlists: Vec<Playlist>,
    #[serde(default)]
    pub me: Option<Me>,
    #[serde(default)]
    pub my_followings: Vec<UserMini>,
    /// Your Library's mixes and radio stations (see LibraryItem).
    #[serde(default)]
    pub mixes: Vec<LibraryItem>,
    #[serde(default)]
    pub radios: Vec<LibraryItem>,
}

/// A mix or a radio station in Your Library. Both are SoundCloud system
/// playlists, opened by URN like any playlist: mixes made for you (Home's
/// "Made for you" and "Mixed for …", and mixes you played), stations you
/// started here or played in SoundCloud's apps (Home's "Recently Played").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryItem {
    pub urn: String,
    pub title: String,
    #[serde(default)]
    pub artwork_url: Option<String>,
    /// When a radio was started in Wavify (epoch ms); 0 for items that come
    /// from Home, which are replaced whenever Home loads.
    #[serde(default)]
    pub started_ms: u64,
}

/// A radio being started: only the latest one's tracks are used.
#[derive(Debug, Clone)]
pub struct RadioRequest {
    pub gen: u64,
    /// The track it's based on (an artist radio has none).
    pub seed: Option<i64>,
    pub title: String,
    /// Its station, a SoundCloud system playlist.
    pub urn: String,
}

/// A long list that builds only its rows on screen (see virtual_list.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListKey {
    Library,
    Playlist,
    Search,
    ProfileTracks,
    ProfileLikes,
    Queue,
    OfflineHome,
}

/// Your Library's filter chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFilter {
    Playlists,
    Mixes,
    Radio,
    Artists,
}

/// A radio station's system playlist (a track's or an artist's station).
fn is_station_urn(urn: &str) -> bool {
    urn.contains(":track-stations:") || urn.contains(":artist-stations:")
}

/// A station's picture, as Spotify shows an artist's radio: only its
/// artist's avatar (drawn round), not SoundCloud's "STATION" collage. The
/// artist is the seed's: the station artist, or whoever uploaded the seed
/// track; failing that, the first track's.
fn station_avatar(p: &PlaylistDetail) -> Option<String> {
    if !is_station_urn(&p.id_or_urn) {
        return None;
    }
    let seed = parse_trailing_id(&p.id_or_urn);
    let artist_station = p.id_or_urn.contains(":artist-stations:");
    p.tracks
        .iter()
        .find(|t| {
            if artist_station {
                t.user.as_ref().is_some_and(|u| u.id == seed)
            } else {
                t.id == seed
            }
        })
        .or(p.tracks.first())
        .and_then(|t| t.user.as_ref())
        .and_then(|u| u.avatar_url.clone())
        .filter(|u| !u.is_empty())
}

/// SoundCloud's own playlists (mixes, stations): no numeric id, not likable
/// or deletable, opened by URN.
fn is_system_playlist(id_or_urn: &str) -> bool {
    id_or_urn.starts_with("soundcloud:system-playlists:")
}

/// The id a playlist page is keyed by (downloads, the Play button's state).
/// A system playlist gets a stable negative one from its URN (FNV-1a), so
/// no two mixes share one and none clashes with a real playlist id.
fn playlist_id_for(id_or_urn: &str) -> i64 {
    if !is_system_playlist(id_or_urn) {
        return parse_trailing_id(id_or_urn);
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id_or_urn.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    -((h >> 1) as i64).max(1)
}

/// Followed artists as Your Library lists them (Settings > Your Library >
/// Sort artists): most followers, by name, or most tracks, then by name.
/// Sorted when they load or the setting changes, not on every frame.
fn sort_followings(users: &mut [UserMini], sort: crate::config::ArtistSort) {
    use crate::config::ArtistSort;
    let name = |u: &UserMini| clean_username(&u.username).to_lowercase();
    users.sort_by(|a, b| {
        let first = match sort {
            ArtistSort::Subscribers => b
                .followers_count
                .unwrap_or(0)
                .cmp(&a.followers_count.unwrap_or(0)),
            ArtistSort::Tracks => b.track_count.unwrap_or(0).cmp(&a.track_count.unwrap_or(0)),
            ArtistSort::Alphabet => std::cmp::Ordering::Equal,
        };
        first.then_with(|| name(a).cmp(&name(b)))
    });
}

/// A repost, follow or playlist like sent to SoundCloud, and how it was
/// before (the UI shows the new state at once; a failure puts it back).
#[derive(Debug, Clone, Copy)]
pub enum Toggle {
    Repost(i64, bool),
    Follow(i64, bool),
    PlaylistLike(i64, bool),
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    SearchSubmit,
    SearchRetry(String),
    Tab(Tab),
    PlayTrack(i64),
    LikeCurrent,
    LikeTrack(i64),
    PlayerToggle,
    /// Pause if playing (the media Stop key); never starts playback.
    PlayerPause,
    PlayerVolume(f32),
    /// Volume drag released: persist the level once, not on every move.
    VolumeCommit,
    NextTrack,
    /// The track played to its end: honours Repeat One, else NextTrack
    /// (a manual Next always moves on).
    TrackEnded,
    PrevTrack,
    ToggleShuffle,
    ToggleRepeat,
    ToggleMute,
    SeekRelative(i64),
    VolumeRelative(f32),
    CloseModals,
    ToggleUserMenu,
    CloseUserMenu,
    UserMenuAccount,
    UserMenuSettings,
    ToggleQueue,
    PlayQueueTrack(usize),
    ClearUpcomingQueue,
    OpenPlaylist(String),
    /// A playlist, tagged with the id_or_urn it was asked for by.
    PlaylistLoaded(String, Result<PlaylistDetail, String>),
    NavBack,
    NavForward,
    ArtistClicked(String),
    /// The profile found for an artist name, tagged with the lookup it answers.
    ArtistProfileFound(u64, Result<Option<i64>, String>),
    LoginStart,
    CookieInput(String),
    CookieOpenBrowser,
    CookieAuthorize,
    PasteClipboard,
    /// Ctrl+V that no text field took (see PasteShortcut in update).
    PasteShortcut,
    CookiePasted(String),
    TokenPolled(bool),
    LoginFailed(String),
    Logout,
    SettingsProxyChanged(String),
    SettingsHideScrollbarsToggled(bool),
    SettingsOfflineModeToggled(bool),
    SettingsBypassToggled(bool),
    SettingsYoutubeToggled(bool),
    YoutubeSignIn,
    YoutubeSignedIn(Result<(), String>),
    YoutubeSignOut,
    YoutubeSignedOut,
    /// An event of the YouTube Music player (position, end, session).
    Yt(crate::yt_web::PageEvent),
    /// The track's song on YouTube Music (or why not), and what to do if
    /// there's none.
    YtFound(
        u64,
        i64,
        Result<crate::alt_source::YtSong, String>,
        YtFallback,
    ),
    SettingsDiscordRpcToggled(bool),
    SettingsPreferArtistFromName(bool),
    SettingsSave,
    /// A switch, select or slider in Settings (see settings_view::set_pref).
    SetPref(Pref),
    /// A slider or the equalizer let go: save what it set.
    SavePrefs,
    /// Settings > Storage > Remove all downloads (the first press asks).
    RemoveAllDownloads,
    DownloadsRemoved(u64),
    /// Settings > Storage: the folder downloads and the cache live in.
    OpenStorageFolder,
    /// Ctrl+- / Ctrl++ (a zoom step), Ctrl+0 (100%).
    ZoomStep(i8),
    /// The title bar's close button: quits, or minimizes (a setting).
    CloseButton,
    /// A profile's banner, decoded: user id, image.
    ProfileBannerLoaded(i64, Result<image::Handle, String>),
    /// "Search in Settings" typed.
    SettingsSearch(String),
    CheckUpdates,
    UpdatesChecked(Result<Option<crate::updater::Release>, String>, bool),
    AcceptUpdate,
    DeclineUpdate,
    RemindAfterNewVersion(bool),
    UpdateInstallerDownloaded(Result<std::path::PathBuf, String>),
    /// Settings > Storage measured: bytes of cache, bytes of downloads.
    StorageMeasured(u64, u64),
    /// Settings > Storage > Clear cache, and what it freed.
    ClearCache,
    CacheCleared(u64),
    /// Forget every track's saved playback speed.
    ClearTrackSpeeds,
    // Stories
    ToggleStoriesExpanded,
    OpenStory(usize),
    StoryReceiptDone(StoryReceiptKey, u8, Result<(), String>),
    CloseStory,
    NextStory,
    PrevStory,
    // Offline / Downloads
    /// Download a whole collection for offline playback, one track at a time.
    DownloadCollection(Collection),
    CancelOfflineDownload,
    /// Ok(Some(source)): the audio is a Go+ track's full version from there.
    TrackDownloaded(i64, Result<(), String>),
    /// Download one track for offline playback, beside any collection run.
    DownloadTrackOffline(Track),
    CancelOfflineTrack(i64),
    TrackCachedOffline(Track, Result<(), String>),
    /// Play an entity page's tracks from the top (or shuffled).
    PlayCollection(Collection, bool),
    QueueCollection(Collection),
    ApiReady(Result<ApiStateClone, String>),
    /// Resolved stream, tagged with the play_index generation that asked for it.
    StreamReady(u64, Result<(i64, crate::api::StreamSource), String>),
    CachedAudioReady(u64, i64, std::path::PathBuf),
    StreamPrefetched(Result<(i64, crate::api::StreamSource), String>),
    /// A blocked track resolved through a public proxy (bypass), tagged with
    /// the play_index generation: the stream and the proxy it came through.
    BypassReady(u64, i64, Result<(crate::api::StreamSource, String), String>),
    /// A Go+ track's full version from another source (or why none).
    /// Search results, tagged with the search generation that asked.
    SearchLoaded(u64, Result<(SearchResults, Option<String>), String>),
    HomeLoaded(Result<Vec<HomeSection>, String>),
    ReloadHome,
    LibraryLoaded(Result<Vec<Track>, String>),
    ToggleDone(Toggle, Result<(), String>),
    /// A track like / unlike went through (or not): track, now liked, and
    /// the Liked Tracks row an unlike took out (put back if it failed).
    TrackLikeDone(i64, bool, Option<Box<Track>>, Result<(), String>),
    /// Covers put in the disk cache; true when any came from the network.
    ArtworkLoaded(bool),
    ArtworkReady((Vec<ProcessedArt>, Vec<ArtKey>)),
    StoriesLoaded(Result<Vec<StoryItem>, String>),
    WindowResized(Size),
    PlayerBarHover(Option<PbLink>),
    /// The artist link of a track row under the pointer (track id, row).
    RowArtistHover(Option<(i64, usize)>),
    /// Track rows highlight only while the pointer is over them.
    TrackRowHover(Option<i64>),
    ToggleTrackDescription(i64),
    /// A save ("+") button: the track goes to Liked Tracks, or, saved
    /// already, the playlist picker opens there.
    SaveTrackClicked(Track, MenuAnchor),
    OpenImageViewer(String),
    CloseImageViewer,
    FullImageLoaded(Result<(String, u32, u32, Vec<u8>), String>),
    Noop,
    // waveform player
    WaveLoaded(Result<(i64, Vec<u8>), String>),
    /// A track's visual banner URL, looked up (Ok(None): it has none).
    WaveVisualResolved(i64, Result<Option<String>, String>),
    /// The playing track's banner, decoded (or why not).
    WaveVisualLoaded(i64, Result<VisualPixels, String>),
    /// The banner baked for the waveform: track id, size in pixels (None:
    /// the bake failed).
    WaveVisualBaked(i64, (u32, u32), Option<image::Handle>),
    WaveSeek(f32),
    WaveHover(Option<f32>),
    /// Right-click a waveform position to open the contextual comment/reaction panel.
    WaveContextOpen(f32),
    WaveContextClose,
    WaveContextCommentInput(String),
    WaveContextPostComment,
    WaveContextReact(String),
    /// A comment posted from the waveform panel: track, position, text.
    WaveContextPosted(i64, u64, String, Result<(), String>),
    /// Once a second while playing: fetch the reactions coming up.
    ReactionsPoll,
    /// Reactions of `track` for the asked `seconds`.
    ReactionsLoaded(i64, Vec<u64>, Result<Vec<WaveReaction>, String>),
    ReactionPosted(Result<(), String>),
    WaveWheel(f32),
    CommentsLoaded(Result<CommentsPage, String>),
    CommentsMore,
    /// Related tracks for the seed track whose queue ran out.
    RelatedLoaded(i64, Result<Vec<Track>, String>),
    OpenProfile(i64),
    ProfileLoaded(i64, Result<ProfileDetail, String>),
    ProfileSubTabSelected(ProfileSubTab),
    /// A profile sub-tab's list, by user id (failures too, so a reply
    /// only ends the spinner of the tab it was asked for).
    ProfileFollowersLoaded(i64, Result<Vec<UserMini>, String>),
    ProfileFollowingsLoaded(i64, Result<Vec<UserMini>, String>),
    ProfileLikesLoaded(i64, Result<Vec<Track>, String>),
    ProfileTracksLoaded(i64, Result<Vec<Track>, String>),
    FollowToggle,
    FollowUserToggle(i64, bool),
    OpenTrackInspector(Track),
    CloseTrackInspector,
    InspectorTabSelected(InspectorTab),
    /// The inspector's lists, by track id: replies for a track shown
    /// before don't count towards the one open now.
    TrackFavoritersLoaded(i64, Result<Vec<UserMini>, String>),
    TrackRepostersLoaded(i64, Result<Vec<UserMini>, String>),
    InspectorCommentsLoaded(i64, Result<Vec<Comment>, String>),
    /// A comment's timestamp: that track at that point (track id, ms).
    InspectorSeek(i64, u64),
    TrackRepostToggle(i64),
    LikedPlaylistsLoaded(Result<Vec<Playlist>, String>),
    /// A station's tracks, for the radio request with this number.
    RadioLoaded(u64, Result<Vec<Track>, String>),
    UserFlagsLoaded(
        Result<
            (
                std::collections::HashSet<i64>,
                std::collections::HashSet<i64>,
                std::collections::HashSet<i64>,
                std::collections::HashSet<String>,
            ),
            String,
        >,
    ),
    PlaylistLikeToggle(i64),
    /// Save (or remove) a mix or station in Your Library, by URN.
    SystemPlaylistLikeToggle(String),
    /// It went through (or not): URN, saved before, result.
    SystemPlaylistLikeDone(String, bool, Result<(), String>),
    SearchMore,
    /// The next page of search results: the search it's for, the page URL
    /// (kept for a retry), its tracks and the page after it.
    SearchMoreLoaded(u64, String, Result<(Vec<Track>, Option<String>), String>),
    ToggleLibraryCollapsed,
    /// A Your Library filter chip: shows only that kind (again: all).
    LibraryFilterPicked(LibraryFilter),
    /// A long list scrolled: build its rows `first..last` (see virtual_list).
    ListWindow(ListKey, usize, usize),
    SidebarCreatePlaylist,
    SidebarCreatePlaylistTitle(String),
    SidebarCreatePlaylistSubmit,
    SidebarCreatePlaylistCancel,
    /// my_playlists response, tagged with the generation it was fetched at.
    PlaylistsLoaded(u64, Result<Vec<Playlist>, String>),
    /// Escape that a focused field or slider already consumed: still closes
    /// an open bar popup (keyboard::on_key_press never sees it).
    EscapePopups,
    /// Player bar save button: saves to Liked Tracks, or opens the popover
    /// once the track is saved somewhere (Spotify's "+" / check button).
    SaveCurrentClicked,
    /// The playlist picker for a track, dropped down from `MenuAnchor`.
    OpenAddPopover(Track, MenuAnchor),
    /// Right / middle click on the save button: the picker, never a quick save.
    OpenAddPopoverCurrent,
    CloseAddPopover,
    /// Speed popup (right click on the speed pill) and wheel over the pill.
    OpenSpeedPopup,
    CloseSpeedPopup,
    SpeedSlider(f32),
    SpeedWheel(f32),
    SpeedInput(String),
    SpeedInputSubmit,
    SpeedReset,
    AddPopoverQuery(String),
    AddPopoverToggleLiked,
    AddPopoverToggle(i64),
    AddPopoverNewPlaylist,
    AddPopoverNewName(String),
    AddPopoverCreate,
    /// Search SoundCloud for a genre or tag (a track page's tag pills).
    SearchTag(String),
    /// Close the open story, then do this (open a profile from it, ...).
    CloseStoryThen(Box<Message>),
    AddPopoverDone,
    SavesApplied(SaveReport),
    PlaylistActionDone(Result<String, String>),
    FollowingsLoaded(Result<Vec<UserMini>, String>),
    RequestDeletePlaylist(i64, String),
    CancelDeletePlaylist,
    DeletePlaylist(i64),
    PlaylistDeleted(i64, Result<(), String>),
    OpenExternalLink(String),
    DownloadTrack(Track),
    DownloadDone(Track, Result<String, String>),
    DismissToast,
    OpenActionMenu(ActionMenu, MenuAnchor),
    /// A menu item: closes the menu, then does the item's action.
    MenuPick(Box<Message>),
    CloseActionMenu,
    CopyTrackLink(Track),
    CopyPlaylistLink(i64, Option<String>),
    CopyProfileLink(i64, String, Option<String>),
    StartTrackRadio(i64, String),
    StartArtistRadio(i64, String),
    AddToQueue(Track),
    SetPlaybackSpeed(f32),
    CyclePlaybackSpeed,
    Tick,
    AnimTick,
    InitWindowId(Option<iced::window::Id>),
    InitWindowScale(f32),
    WindowDragStart,
    WindowMinimize,
    WindowToggleMaximize,
    WindowClose,
    // Track Page
    OpenTrackPage(Box<Track>),
    TrackPageDetailLoaded(Result<Track, String>),
    TrackPageRelatedLoaded(Result<(i64, Vec<Track>), String>),
    TrackPageCommentsLoaded(Result<(i64, Vec<Comment>), String>),
    TrackPageLikersLoaded(Result<(i64, Vec<UserMini>), String>),
    TrackPageRepostersLoaded(Result<(i64, Vec<UserMini>), String>),
    TrackPageSubTabSelected(TrackSubTab),
    TrackPageCommentInput(String),
    TrackPagePostComment,
    TrackPageReact(String),
    TrackPageCommentPosted(Result<(), String>),
    TrackPagePlayToggle,
    TrackPagePlayAllRelated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProfileSubTab {
    #[default]
    Overview,
    Tracks,
    Playlists,
    Likes,
    Followers,
    Following,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Comments,
    Likers,
    Reposters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TrackSubTab {
    #[default]
    Related,
    Comments,
    Likers,
    Reposters,
}

#[derive(Debug, Clone, Default)]
pub struct TrackDetailPage {
    pub track: Track,
    pub related_tracks: Vec<Track>,
    pub comments: Vec<Comment>,
    pub likers: Vec<UserMini>,
    pub reposters: Vec<UserMini>,
    pub active_tab: TrackSubTab,
    /// Sub-tabs whose list is still on its way (each shows a spinner, not
    /// "none yet", until its own response lands).
    pub pending: std::collections::HashSet<TrackSubTab>,
    pub comment_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Home,
    Search,
    Library,
    Settings,
    Playlist,
    Profile,
    Track,
}

/// A page Back and Forward return to: a tab, or the playlist, profile or
/// track that was open (Profile A -> Profile B -> Back is A again).
#[derive(Debug, Clone)]
pub enum Route {
    Tab(Tab),
    Playlist(String),
    Profile(i64),
    Track(Box<Track>),
}

impl Route {
    fn tab(&self) -> Tab {
        match self {
            Route::Tab(t) => t.clone(),
            Route::Playlist(_) => Tab::Playlist,
            Route::Profile(_) => Tab::Profile,
            Route::Track(_) => Tab::Track,
        }
    }

    fn same(&self, other: &Route) -> bool {
        match (self, other) {
            (Route::Tab(a), Route::Tab(b)) => a == b,
            (Route::Playlist(a), Route::Playlist(b)) => a == b,
            (Route::Profile(a), Route::Profile(b)) => a == b,
            (Route::Track(a), Route::Track(b)) => a.id == b.id,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub users: Vec<UserMini>,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistDetail {
    pub id: i64,
    pub id_or_urn: String,
    pub title: String,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    pub author: String,
    pub author_avatar: Option<String>,
    /// The author's user id: their name and avatar open their profile.
    #[serde(default)]
    pub author_id: Option<i64>,
    /// Its soundcloud.com page (mixes and stations live under /discover).
    #[serde(default)]
    pub permalink_url: Option<String>,
    pub track_count: usize,
    pub tracks: Vec<Track>,
    pub is_album: bool,
}

#[derive(Debug, Clone)]
pub struct HomePlaylist {
    pub id_or_urn: String,
    pub title: String,
    pub subtitle: String,
    pub artwork_url: Option<String>,
    pub tracks: Vec<Track>,
    /// Who made it (a user's playlist; None for SoundCloud's own mixes):
    /// the card's "By …" line opens their profile.
    pub owner: Option<UserMini>,
}

impl HomePlaylist {
    pub fn artwork_or_avatar(&self) -> Option<&str> {
        self.artwork_url
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| self.tracks.first().and_then(|t| t.artwork_or_avatar()))
    }
}

/// Full-resolution photo overlay (track / album / playlist / artist artwork).
#[derive(Debug, Clone)]
pub struct ImageViewer {
    pub url: String,
    pub handle: Option<image::Handle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryItem {
    pub user_id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub track_id: i64,
    pub track_title: String,
    pub artwork_url: Option<String>,
    pub duration_ms: u64,
    pub track: Track,
    /// When the followed artist posted/reposted this track (epoch ms, 0 = unknown).
    #[serde(default)]
    pub created_at_ms: u64,
    /// True when the update is a repost rather than an own upload.
    #[serde(default)]
    pub reposted: bool,
    /// Server-side read state from /you/artist_shortcuts (has_read).
    #[serde(default)]
    pub server_read: bool,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct StoryReceiptKey {
    user_id: i64,
    track_id: i64,
    created_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoryReceiptOutcome {
    Complete,
    Retry,
    Failed,
}

fn story_receipt_outcome(attempt: u8, succeeded: bool) -> StoryReceiptOutcome {
    if succeeded {
        StoryReceiptOutcome::Complete
    } else if attempt == 0 {
        StoryReceiptOutcome::Retry
    } else {
        StoryReceiptOutcome::Failed
    }
}

fn web_profile_icon(service: Option<&str>, url: &str) -> (&'static str, iced::Font) {
    let service = service
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let host = url
        .trim()
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url.trim())
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let host_is = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    let matches = |names: &[&str], domains: &[&str]| {
        names.contains(&service.as_str()) || domains.iter().any(|domain| host_is(domain))
    };
    let glyph = if matches(&["instagram", "insta"], &["instagram.com"]) {
        icons::BRAND_INSTAGRAM
    } else if matches(&["twitter", "x"], &["twitter.com", "x.com"]) {
        icons::BRAND_TWITTER
    } else if matches(&["youtube", "youtu"], &["youtube.com", "youtu.be"]) {
        icons::BRAND_YOUTUBE
    } else if matches(&["spotify"], &["spotify.com"]) {
        icons::BRAND_SPOTIFY
    } else if matches(&["soundcloud", "sndsc"], &["soundcloud.com", "snd.sc"]) {
        icons::BRAND_SOUNDCLOUD
    } else if matches(&["facebook", "fb"], &["facebook.com", "fb.com"]) {
        icons::BRAND_FACEBOOK
    } else if matches(&["tiktok"], &["tiktok.com"]) {
        icons::BRAND_TIKTOK
    } else if matches(&["bandcamp"], &["bandcamp.com"]) {
        icons::BRAND_BANDCAMP
    } else if matches(
        &["apple", "applemusic", "itunes"],
        &["music.apple.com", "apple.com"],
    ) {
        icons::BRAND_APPLE
    } else if matches(&["patreon"], &["patreon.com"]) {
        icons::BRAND_PATREON
    } else if matches(&["twitch"], &["twitch.tv", "twitch.com"]) {
        icons::BRAND_TWITCH
    } else if matches(&["tumblr"], &["tumblr.com"]) {
        icons::BRAND_TUMBLR
    } else if matches(&["vimeo"], &["vimeo.com"]) {
        icons::BRAND_VIMEO
    } else if matches(&["reddit"], &["reddit.com", "redd.it"]) {
        icons::BRAND_REDDIT
    } else if matches(&["discord"], &["discord.com", "discord.gg"]) {
        icons::BRAND_DISCORD
    } else if matches(&["linkedin"], &["linkedin.com"]) {
        icons::BRAND_LINKEDIN
    } else {
        return (icons::EXTERNAL_LINK, FA_SOLID);
    };
    (glyph, FA_BRANDS)
}

#[derive(Debug, Clone)]
pub enum HomeShelf {
    Playlists(Vec<HomePlaylist>),
    Tracks(Vec<Track>),
}

#[derive(Debug, Clone)]
pub struct HomeSection {
    pub title: String,
    /// The selection's URN ("soundcloud:selections:recently-played:…");
    /// empty for offline Home's own sections.
    pub urn: String,
    pub shelf: HomeShelf,
}

/// Artist profile page bundle.
#[derive(Debug, Clone, Default)]
pub struct ProfileDetail {
    pub id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub permalink_url: Option<String>,
    pub followers: u64,
    pub followings: u64,
    pub track_count: u64,
    pub likes_count: u64,
    pub playlist_count: u64,
    pub following: bool,
    pub top_tracks: Vec<Track>,
    pub all_tracks: Vec<Track>,
    pub reposts: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub likes: Vec<Track>,
    pub followers_list: Vec<UserMini>,
    pub followings_list: Vec<UserMini>,
    pub related: Vec<UserMini>,
    pub web_profiles: Vec<WebProfile>,
    pub active_tab: ProfileSubTab,
    /// The banner the artist set on SoundCloud ("visuals"), if any.
    pub banner_url: Option<String>,
    /// Their real name and where they are, as SoundCloud shows them under
    /// the username.
    pub full_name: String,
    pub location: String,
}

/// One page of a track's waveform comments.
#[derive(Debug, Clone)]
pub struct CommentsPage {
    pub track_id: i64,
    pub comments: Vec<Comment>,
    /// The next page, when SoundCloud has more.
    pub next: Option<String>,
    /// Read from the disk cache: nothing new to save back.
    pub cached: bool,
}

/// A comment marker positioned on the waveform.
#[derive(Debug, Clone)]
pub struct WaveComment {
    pub ts_ms: u64,
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlaybackState {
    track: Track,
    pos_ms: u64,
}

#[derive(Debug, Clone)]
struct StoryReturnState {
    queue: Vec<Track>,
    queue_pos: usize,
    pos_ms: u64,
    was_paused: bool,
}

// Waveform reactions work like the SoundCloud Android app's (read from its
// quick-reactions module), except that a second can hold several: while a
// track plays the next minute of seconds is kept fetched, and when the
// playhead reaches a second, its reactions float up from the playhead one
// after another over that second.

/// Next unfetched second closer than this to the playhead: fetch ahead.
pub const REACTION_LOOKAHEAD_S: u64 = 5;
/// Seconds fetched per request (the app's 60 s window).
pub const REACTION_WINDOW_S: u64 = 60;
/// A floating reaction's flight: 900 ms, rising 75 px, drifting up to 10 px
/// sideways, growing from 0.7x to full size while it fades.
pub const REACTION_FLOAT_MS: f32 = 900.0;
pub const REACTION_RISE: f32 = 75.0;
pub const REACTION_DRIFT: f32 = 10.0;
/// Reactions fetched per second of the track.
pub const REACTION_PER_SECOND: usize = 5;
/// Burst when you react yourself: 24 emoji particles.
pub const BURST_PARTICLES: usize = 24;
/// Particles in flight at most, however fast the taps come (the oldest go).
pub const MAX_PARTICLES: usize = 8 * BURST_PARTICLES;

/// Apple's emoji for the quick reactions, as pictures: Windows 10's Segoe UI
/// Emoji has no 🥹 (it draws a box), and a picture scales through the
/// animations without a glyph rasterised at every size. Other codepoints
/// fall back to the emoji font.
fn reaction_image(codepoint: &str) -> Option<image::Handle> {
    static HANDLES: std::sync::OnceLock<[image::Handle; 3]> = std::sync::OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            image::Handle::from_bytes(&include_bytes!("../assets/emoji/1f525.png")[..]),
            image::Handle::from_bytes(&include_bytes!("../assets/emoji/1f44f.png")[..]),
            image::Handle::from_bytes(&include_bytes!("../assets/emoji/1f979.png")[..]),
        ]
    });
    QUICK_REACTIONS
        .iter()
        .position(|(c, _)| *c == codepoint)
        .map(|i| handles[i].clone())
}

/// A reaction floating up from the playhead.
#[derive(Debug, Clone)]
pub struct FloatingReaction {
    /// When it sets off (later ones of the same second wait their turn).
    pub spawned_at: std::time::Instant,
    /// Emoji codepoint in hex ("1f525").
    pub codepoint: String,
    /// Playhead position (fraction of the track) when it appeared.
    pub x_frac: f32,
    /// Sideways drift direction and amount, -1..1.
    pub drift: f32,
}

/// One emoji of the burst when you react. Positions are in window pixels.
#[derive(Debug, Clone)]
pub struct Particle {
    pub spawned_at: std::time::Instant,
    /// Emoji codepoint in hex ("1f525").
    pub codepoint: String,
    pub origin: Point,
    /// Initial velocity, px/s (vy < 0 is up).
    pub vx: f32,
    pub vy: f32,
    /// Peak size factor (of 30 px) and opacity.
    pub scale: f32,
    pub alpha: f32,
    pub life_ms: f32,
}

impl Particle {
    /// A burst of emoji particles at `origin`, like the app's: speeds of
    /// ±240 px/s sideways and 250-800 px/s up, sizes 0.5-1.3x, lifetimes
    /// 1.5-2.2 s (small, faint ones last longer, as further away).
    pub fn burst(codepoint: &str, origin: Point) -> Vec<Particle> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let now = std::time::Instant::now();
        (0..BURST_PARTICLES)
            .map(|_| {
                let depth: f32 = rng.gen();
                let life = 1500.0 + 700.0 * rng.gen::<f32>();
                Particle {
                    spawned_at: now,
                    codepoint: codepoint.to_string(),
                    origin: Point::new(
                        origin.x + rng.gen_range(-30.0..30.0),
                        origin.y + rng.gen_range(-30.0..30.0),
                    ),
                    vx: rng.gen_range(-240.0..240.0),
                    vy: -rng.gen_range(250.0..800.0),
                    scale: 0.5 + 0.8 * depth,
                    alpha: 0.55 + 0.45 * depth,
                    life_ms: life * (1.0 + 0.3 * (1.0 - depth)),
                }
            })
            .collect()
    }

    /// Size factor at progress `t` (0..1): pops in over the first 8%, holds
    /// to 45%, then shrinks away in steps (the app's keyframes).
    pub fn scale_at(&self, t: f32) -> f32 {
        let s = self.scale;
        let lerp = |a: f32, b: f32, f: f32| a + (b - a) * f.clamp(0.0, 1.0);
        if t < 0.08 {
            lerp(0.0, s, t / 0.08)
        } else if t < 0.45 {
            s
        } else if t < 0.7 {
            lerp(s, 0.6 * s, (t - 0.45) / 0.25)
        } else if t < 0.9 {
            lerp(0.6 * s, 0.2 * s, (t - 0.7) / 0.2)
        } else {
            lerp(0.2 * s, 0.0, (t - 0.9) / 0.1)
        }
    }
}

/// Clone-able mirror of shared API state.
#[derive(Debug, Clone)]
pub struct ApiStateClone {
    pub client_id: String,
    pub me: Option<Me>,
    pub authenticated: bool,
    /// The token is one the mobile API takes (from the login window). A
    /// token from pasted browser cookies reads fine, but SoundCloud blocks
    /// every like, repost, follow and comment made with it.
    pub can_write: bool,
}

pub struct App {
    settings: Settings,
    state: ApiStateClone,
    pub login_error: Option<String>,
    pub cookie_input: String,
    /// Settings > Network fields as typed. They reach `settings` only on
    /// Save: the volume and toggle auto-saves write `settings` to disk, and
    /// make_api rereads it on every request, so a half-typed proxy would
    /// take effect.
    pub proxy_draft: String,
    /// Settings > Storage > Remove all downloads was pressed once: the next
    /// press removes them.
    pub confirm_remove_downloads: bool,
    /// Started by Windows at login, minimized: the window goes down once it
    /// has an id.
    pub start_minimized: bool,
    /// The next play_index is a crossfade: the old track fades out instead
    /// of stopping.
    pub crossfade_next: bool,
    /// The track a crossfade already started from (once per play).
    pub crossfaded_from: Option<i64>,
    /// The open profile's banner (user id, image).
    pub profile_banner: Option<(i64, image::Handle)>,
    pub tab: Tab,
    pub nav_history: Vec<Route>,
    pub nav_future: Vec<Route>,
    /// Back / Forward is reopening a page: it isn't a new step in history.
    pub nav_restoring: bool,
    pub search_query: String,
    pub search: SearchResults,
    pub home: Vec<HomeSection>,
    pub current_playlist: Option<PlaylistDetail>,
    pub track_page: Option<TrackDetailPage>,
    pub library: Vec<Track>,
    pub queue: Vec<Track>,
    pub queue_pos: usize,
    pub liked_ids: std::collections::HashSet<i64>,
    pub my_following_ids: std::collections::HashSet<i64>,
    pub playing_id: Option<i64>,
    pub playing_title: String,
    pub playing_artist: String,
    pub is_paused: bool,
    /// A story preview uses the shared audio engine without becoming the
    /// track shown in the persistent Now Playing bar.
    pub story_playback: bool,
    story_return_state: Option<StoryReturnState>,
    /// Seek applied after the next stream/cached-audio command starts.
    pub pending_seek_ms: Option<u64>,
    pub pending_seek_track: Option<i64>,
    pub last_saved_playback_second: u64,
    pub volume: f32,
    /// The playlist / profile a page waits for: it shows a spinner until
    /// that response lands, and a response for anything else is dropped.
    pub loading_playlist: Option<String>,
    pub loading_profile: Option<i64>,
    /// Bumped per search: an older search's results arriving late are dropped.
    pub search_gen: u64,
    pub search_loading: bool,
    pub library_loading: bool,
    pub home_loading: bool,
    /// Home's feed failed to load (shown with a Retry).
    pub home_error: Option<String>,
    /// Profile sub-tabs (tracks, likes, followers...) loading, by user: a
    /// reply ends only its own tab's spinner, and a tab isn't asked twice.
    pub profile_tabs_loading: std::collections::HashSet<(i64, ProfileSubTab)>,
    /// Why the open page couldn't load, and the message that retries it.
    pub page_error: Option<(Tab, String, Box<Message>)>,
    pub login_pending: bool,
    pub player: Option<player::PlayerHandle>,
    pub prefetched_stream: Option<(i64, crate::api::StreamSource)>,
    /// When the prefetched stream was resolved: its signed URLs expire, so
    /// an old one (after a long pause) is resolved anew.
    pub prefetched_at: Option<std::time::Instant>,
    pub prefetch_in_progress: Option<i64>,
    /// Bumped by every play_index; a stream result from an older one is stale.
    pub play_gen: u64,
    /// The playing track came through this public proxy (bypass), and how
    /// many proxies it has tried (a dead one is swapped for the next).
    pub bypass_proxy: Option<(i64, String)>,
    pub bypass_attempts: u8,
    /// Go+ tracks whose cached audio is a full version, and its source.
    pub full_versions: std::collections::HashMap<i64, String>,
    /// The playing track's YouTube Music song (track id, video id).
    pub yt_video: Option<(i64, String)>,
    /// An ad is playing in the YouTube Music player.
    pub yt_ad: bool,
    pub yt_signed_in: bool,
    /// The YouTube Music player was restarted once for the playing song.
    pub yt_restarted: bool,
    /// The current track played to its end (or failed) and nothing is
    /// loaded: Play has to start a track, a Resume would hit an empty sink.
    pub at_end: bool,
    /// Set when the play command went out, cleared once audio moves: a
    /// track still silent after STALL_MS failed inside the player.
    pub awaiting_audio: Option<std::time::Instant>,
    /// Unplayable tracks skipped in a row (stop once the whole queue failed).
    pub play_failures: usize,
    /// Seed of the related-tracks fetch started when the queue ran out.
    pub related_seed: Option<i64>,
    /// The queue's own order (track ids), as it was set. Shuffle reorders
    /// the queue itself, so Next Up shows what really plays next; turning
    /// it off puts this order back.
    pub queue_order: Vec<i64>,
    /// Decoded tiles keyed by `art_key_hash(url, shape, display px)`, so a hit
    /// needs no allocation while view() runs.
    pub artwork: std::collections::HashMap<u64, image::Handle>,
    /// Bytes of each decoded tile, and their sum: the cache is held to a
    /// memory budget, not a count (a 500px cover is 130 thumbnails).
    pub artwork_sizes: std::collections::HashMap<u64, usize>,
    pub artwork_bytes: usize,
    /// Tiles requested by views this frame (interior mutability: filled in view()).
    pub art_wanted: std::cell::RefCell<std::collections::HashMap<u64, ArtKey>>,
    pub art_inflight: std::collections::HashSet<u64>,
    /// Failed requests with the failure time; retried after a cooldown or
    /// once a batch proves the network is back.
    pub art_failed: std::collections::HashMap<u64, std::time::Instant>,
    /// Keys the views actually drew recently — the eviction pass never
    /// removes these, so on-screen art can't churn.
    pub art_touched: std::cell::RefCell<std::collections::HashSet<u64>>,
    /// Precomputed waveform bars (downsampled once per track, not per frame).
    pub wave_bars: std::sync::Arc<Vec<f32>>,
    pub last_pos_poll: std::time::Instant,
    pub show_queue: bool,
    /// The radio being started (see RadioRequest), and the last request's number.
    pub radio_request: Option<RadioRequest>,
    pub radio_gen: u64,
    pub discord_rpc: Option<DiscordRpcHandle>,
    // waveform + progress
    pub pos_ms: u64,
    pub dur_ms: u64,
    pub hover_frac: Option<f32>,
    /// Waveform context panel: selected fraction and its draft comment.
    pub wave_context_frac: Option<f32>,
    pub wave_context_comment: String,
    pub wave_comments: Vec<WaveComment>,
    /// Waveform reactions of the playing track by second, and the seconds
    /// already asked for (fetched a minute at a time, ahead of the playhead).
    pub reactions: std::collections::HashMap<u64, Vec<WaveReaction>>,
    pub reactions_fetched: std::collections::BTreeSet<u64>,
    pub reactions_inflight: bool,
    /// The last playback second whose reaction had its turn.
    pub reaction_sec: Option<u64>,
    pub floating: Vec<FloatingReaction>,
    pub particles: Vec<Particle>,
    /// Your last reaction sent (track, second, codepoint): taps repeating
    /// it aren't sent again.
    pub last_posted_reaction: Option<(i64, u64, String)>,
    pub comments_next: Option<String>,
    pub comments_loading: bool,
    /// Comment pages loaded for the playing track (paging stops at
    /// COMMENTS_MAX_PAGES).
    pub comments_pages: u32,
    /// The track the waveform's comment panel was opened on.
    pub wave_context_track: Option<i64>,
    pub profile: Option<ProfileDetail>,
    pub reposted_ids: std::collections::HashSet<i64>,
    pub liked_playlists: Vec<Playlist>,
    pub liked_playlist_ids: std::collections::HashSet<i64>,
    /// Mixes and stations saved in Your Library, by URN.
    pub liked_system_urns: std::collections::HashSet<String>,
    pub search_next: Option<String>,
    // Track inspector modal / drawer
    pub inspector_track: Option<Track>,
    pub inspector_tab: InspectorTab,
    pub inspector_favoriters: Vec<UserMini>,
    pub inspector_comments: Vec<Comment>,
    /// Number of outstanding async tasks for the inspector panel.
    /// Spinner shows while > 0. Avoids the "loading forever" bug when
    /// one of the 3 concurrent tasks fails silently.
    pub inspector_tasks_pending: u8,
    // Add to playlist modal
    pub add_popover: Option<AddPopover>,
    /// Speed popup: open while `Some`, holding the value field's text.
    pub speed_popup: Option<String>,
    /// Fractional wheel notches (touchpads) not yet turned into a speed step.
    pub speed_wheel_acc: f32,
    /// my_playlists fetch generation: bumped when a save commit starts and
    /// when it finishes, so a response from before either is dropped.
    pub playlists_gen: u64,
    /// Save commits that touch playlists and haven't reported back yet.
    pub saves_in_flight: u32,
    /// Your Library sidebar collapsed to an icon rail (Spotify parity).
    pub library_collapsed: bool,
    /// Sidebar "+" pressed -> inline title input instead of the modal dialog.
    pub sidebar_create_mode: bool,
    pub sidebar_create_title: String,
    pub my_playlists: Vec<Playlist>,
    pub my_followings: Vec<UserMini>,
    pub delete_playlist_confirm: Option<(i64, String)>,
    // Track actions menu modal / drawer
    pub action_menu: Option<ActionMenu>,
    /// The button the open "..." menu dropped down from.
    pub menu_anchor: Option<MenuAnchor>,
    /// Where the playlist picker drops down from.
    pub add_popover_anchor: MenuAnchor,
    // Listening history
    pub history: Vec<Track>,
    // Playback state & transport controls
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub last_wheel_skip: Option<std::time::Instant>,
    pub toast: Option<Toast>,
    pub is_muted: bool,
    pub prev_volume: f32,
    pub inspector_reposters: Vec<UserMini>,
    pub show_user_menu: bool,
    pub playback_speed: f32,
    // Stories
    pub stories: Vec<StoryItem>,
    pub stories_expanded: bool,
    pub active_story_index: Option<usize>,
    /// Track ids of stories the user already opened (unread-ring state).
    pub stories_read: std::collections::HashSet<i64>,
    /// Read receipts currently being sent; keys distinguish reposted stories.
    story_receipts_pending: std::collections::HashSet<StoryReceiptKey>,
    /// Open full-resolution photo overlay, if any.
    pub image_viewer: Option<ImageViewer>,
    /// Header tint sampled from each artwork, for Spotify-style entity headers.
    pub art_colors: std::collections::HashMap<String, Color>,
    /// True once stories came from the follow-feed stream (don't overwrite
    /// them with the weaker home-section extraction fallback).
    pub stories_from_stream: bool,
    // Offline / Downloads
    pub downloaded_track_ids: std::collections::HashSet<i64>,
    pub downloaded_playlist_ids: std::collections::HashSet<String>,
    pub offline_downloading: Option<Collection>,
    /// Cooperative cancellation flags for audio downloads.
    pub offline_batch_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Single-track downloads in flight (see DownloadTrackOffline).
    pub offline_single: std::collections::HashSet<i64>,
    pub offline_single_cancel:
        std::collections::HashMap<i64, std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub offline_store: OfflineStore,
    pub offline_download_progress: (usize, usize),
    /// Tracks of the running playlist download, snapshotted when it started.
    /// Re-deriving them from the open page on every step retried a failing
    /// track forever, and opening another playlist ended the download and
    /// marked it complete.
    pub offline_plan: Vec<Track>,
    /// Track ids tried in this run, succeeded or failed: each is tried once.
    pub offline_attempted: std::collections::HashSet<i64>,
    pub offline_failed: usize,
    // Animation state
    /// Wall-clock origin for animations, so their speed is independent of the
    /// tick cadence (which now varies between 16ms and 40ms).
    pub anim_start: std::time::Instant,
    pub window_id: Option<iced::window::Id>,
    pub window_scale: f32,
    /// Current logical window size, kept live via resize events; header text
    /// and the waveform banner are fitted to this.
    pub window_size: Size,
    pub pb_hover: Option<PbLink>,
    pub row_artist_hover: Option<(i64, usize)>,
    pub hovered_track_row: Option<i64>,
    /// The one expanded track description, scoped to its track id.
    pub expanded_description_track: Option<i64>,
    pub wave_color_t: f32, // 0.0 (grey) .. 1.0 (orange)
    pub vol_color_t: f32,  // 0.0 (grey) .. 1.0 (orange)
    /// Visual banner URL per track id once known this session (None: the
    /// track has none), so replays don't ask the API again.
    pub visual_urls: std::collections::HashMap<i64, Option<String>>,
    /// The playing track's banner, decoded: kept to bake it again when the
    /// waveform changes size.
    pub wave_visual_src: Option<(i64, VisualPixels)>,
    /// The banner baked for the waveform box (see bake_wave_visual).
    pub wave_visual: Option<WaveVisual>,
    /// A bake is running; one at a time, the next starts when it lands.
    pub wave_visual_baking: bool,
    /// Playback speed per track id (tracks at 1.0x aren't stored).
    pub track_speeds: std::collections::HashMap<i64, f32>,
    /// `track_speeds` changed since it was last written (see Tick).
    pub track_speeds_dirty: bool,
    /// The volume changed by wheel or keys since settings.json was written.
    pub volume_dirty: bool,
    /// Settings page filter ("Search in Settings").
    pub settings_search: String,
    pub update_release: Option<crate::updater::Release>,
    pub update_checking: bool,
    pub update_check_manual: bool,
    pub update_installing: bool,
    pub remind_after_new_version: bool,
    /// Settings > Storage: (cache, downloads) in bytes, once measured.
    pub storage: Option<(u64, u64)>,
    /// Your Library's mixes and radio stations (see LibraryItem).
    pub library_mixes: Vec<LibraryItem>,
    pub library_radios: Vec<LibraryItem>,
    /// Your Library shows only this kind while a chip is on.
    pub library_filter: Option<LibraryFilter>,
    /// An artist name being looked up (see ArtistClicked): its token and name.
    pub artist_lookup: Option<(u64, String)>,
    /// The rows each long list builds (see virtual_rows).
    pub list_windows: std::collections::HashMap<ListKey, (usize, usize)>,
    pub artist_lookup_gen: u64,
}

/// A decoded banner in a message: Debug prints its size, not its pixels.
#[derive(Clone)]
pub struct VisualPixels(pub std::sync::Arc<::image::RgbaImage>);

impl std::fmt::Debug for VisualPixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VisualPixels({}x{})", self.0.width(), self.0.height())
    }
}

/// The waveform's banner, baked at `size` physical pixels for `track_id`.
#[derive(Debug, Clone)]
pub struct WaveVisual {
    pub track_id: i64,
    pub size: (u32, u32),
    pub handle: image::Handle,
}

/// A cache file to write off the UI thread (see save_in_background).
enum DiskJob {
    Write(
        std::path::PathBuf,
        Box<dyn FnOnce() -> Option<Vec<u8>> + Send>,
    ),
    Flush(std::sync::mpsc::Sender<()>),
}

/// The one thread that writes caches: jobs run in order, so a file's later
/// version never loses to an earlier one.
fn disk_writer() -> &'static std::sync::mpsc::Sender<DiskJob> {
    static TX: std::sync::OnceLock<std::sync::mpsc::Sender<DiskJob>> = std::sync::OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<DiskJob>();
        let spawned = std::thread::Builder::new()
            .name("disk-writer".into())
            .spawn(move || {
                for job in rx {
                    match job {
                        DiskJob::Write(path, make) => {
                            if let Some(bytes) = make() {
                                write_atomic(&path, &bytes);
                            }
                        }
                        DiskJob::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            crate::log!("disk writer thread failed to start: {e}");
        }
        tx
    })
}

/// Serialize (`make`) and write a cache off the UI thread: the library,
/// history and offline store run to megabytes. WindowClose waits for them.
fn save_in_background(
    path: std::path::PathBuf,
    make: impl FnOnce() -> Option<Vec<u8>> + Send + 'static,
) {
    if let Err(std::sync::mpsc::SendError(DiskJob::Write(path, make))) =
        disk_writer().send(DiskJob::Write(path, Box::new(make)))
    {
        // no writer thread: write it here
        if let Some(bytes) = make() {
            write_atomic(&path, &bytes);
        }
    }
}

/// Wait (up to 3 s) for the caches queued so far to be on disk.
fn flush_disk_writes() {
    let (done, wait) = std::sync::mpsc::channel();
    if disk_writer().send(DiskJob::Flush(done)).is_ok() {
        let _ = wait.recv_timeout(std::time::Duration::from_secs(3));
    }
}

/// A temp file renamed over the old one: a crash mid-write leaves the
/// previous version, not half a file.
fn write_atomic(path: &std::path::Path, bytes: &[u8]) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, bytes)
        .and_then(|_| std::fs::rename(&tmp, path))
        .is_err()
    {
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::write(path, bytes);
    }
}

impl App {
    pub fn show_toast(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            created_at: std::time::Instant::now(),
            action: None,
        });
    }

    /// Toast with a text action on the right, like Spotify's
    /// "Added to Liked Songs.  Change".
    /// A like / repost / follow / reaction the API refused. With a read-only
    /// sign-in that's why, and signing in with the login window fixes it.
    pub fn action_failed(&mut self, e: &str) {
        self.write_failed("Action failed", e);
    }

    /// A write SoundCloud refused, as "<what>: <why>". A read-only sign-in
    /// (browser session: the app API refuses its writes, so they go to the
    /// web API behind the bot protection) is the usual reason, and signing
    /// in with the login window fixes it, so that's what the toast offers.
    fn write_failed(&mut self, what: &str, e: &str) {
        if self.state.authenticated && !self.state.can_write {
            self.show_toast_action(
                "SoundCloud blocks actions for this sign-in",
                ToastKind::Error,
                "Sign in",
                Message::LoginStart,
            );
        } else if e.contains(BROWSER_CHECK) || e.contains(NETWORK_BLOCKED) {
            // the web API's bot protection: the app sign-in goes around it
            self.show_toast_action(
                format!("{what}: SoundCloud's bot protection refused it"),
                ToastKind::Error,
                "Sign in",
                Message::LoginStart,
            );
        } else {
            self.show_toast(format!("{what}: {e}"), ToastKind::Error);
        }
    }

    pub fn show_toast_action(
        &mut self,
        message: impl Into<String>,
        kind: ToastKind,
        label: &str,
        action: Message,
    ) {
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            created_at: std::time::Instant::now(),
            action: Some((label.to_string(), Box::new(action))),
        });
    }

    /// The playing track, or a stub with its id and title when it is not in
    /// any loaded list (enough to save it).
    fn playing_track(&self) -> Option<Track> {
        let id = self.playing_id?;
        Some(self.find_track_anywhere(id).unwrap_or_else(|| Track {
            id,
            title: self.playing_title.clone(),
            ..Default::default()
        }))
    }

    /// Speed from the wheel, the slider or the value field: no toast (the pill
    /// shows it), rounded to 0.01x inside SPEED_MIN..=SPEED_MAX. `sync_field`
    /// rewrites the popup's field; typing passes false so the text being
    /// typed is left alone.
    fn set_speed_quiet(&mut self, v: f32, sync_field: bool) {
        let v =
            ((v.clamp(SPEED_MIN, SPEED_MAX) * 100.0).round() / 100.0).clamp(SPEED_MIN, SPEED_MAX);
        if (v - self.playback_speed).abs() > 0.0005 {
            self.playback_speed = v;
            if let Some(p) = &self.player {
                p.send(PlayerCommand::SetSpeed(v));
            }
            self.remember_speed();
            self.update_discord_rpc();
        }
        if sync_field {
            if let Some(field) = self.speed_popup.as_mut() {
                *field = format!("{v:.2}");
            }
        }
    }

    /// The speed a track plays at: the one last set while it played, else 1.0x.
    fn track_speed(&self, track_id: i64) -> f32 {
        self.track_speeds.get(&track_id).copied().unwrap_or(1.0)
    }

    /// The playing track keeps the speed just set: it starts at that speed
    /// the next time it plays (after a restart too). 1.0x forgets it.
    fn remember_speed(&mut self) {
        if self.story_playback {
            return;
        }
        let Some(id) = self.playing_id else {
            return;
        };
        let v = self.playback_speed;
        let changed = if (v - 1.0).abs() < 0.005 {
            self.track_speeds.remove(&id).is_some()
        } else {
            self.track_speeds.insert(id, v) != Some(v)
        };
        self.track_speeds_dirty |= changed;
    }

    /// Start `track_id` at its own speed (see remember_speed), and show it in
    /// an open speed popup.
    fn apply_track_speed(&mut self, track_id: i64) {
        self.playback_speed = self.track_speed(track_id);
        if let Some(p) = &self.player {
            p.send(PlayerCommand::SetSpeed(self.playback_speed));
        }
        let v = self.playback_speed;
        if let Some(field) = self.speed_popup.as_mut() {
            *field = format!("{v:.2}");
        }
    }

    /// The audio of `id` just started: go to its saved position (a resume,
    /// the track a story interrupted, a comment's second), once.
    fn apply_pending_seek(&mut self, id: i64) {
        if self.pending_seek_track != Some(id) {
            return;
        }
        if let Some(ms) = self.pending_seek_ms.take() {
            if let Some(p) = &self.player {
                p.send(PlayerCommand::SeekMs(ms));
            }
            self.pos_ms = ms;
        }
        self.pending_seek_track = None;
    }

    pub fn load_track_speeds() -> std::collections::HashMap<i64, f32> {
        std::fs::read_to_string(crate::config::track_speeds_path())
            .ok()
            .and_then(|j| serde_json::from_str::<std::collections::HashMap<i64, f32>>(&j).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, v)| (SPEED_MIN..=SPEED_MAX).contains(v) && (v - 1.0).abs() >= 0.005)
            .collect()
    }

    fn save_volume(&mut self) {
        if std::mem::take(&mut self.volume_dirty) {
            self.settings.save();
        }
    }

    /// Written on the next tick after a change, not on every slider step.
    fn save_track_speeds(&mut self) {
        if !self.track_speeds_dirty {
            return;
        }
        self.track_speeds_dirty = false;
        if let Ok(json) = serde_json::to_string(&self.track_speeds) {
            let path = crate::config::track_speeds_path();
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &json)
                .and_then(|_| std::fs::rename(&tmp, &path))
                .is_err()
            {
                let _ = std::fs::remove_file(&tmp);
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Size of the waveform canvas in physical pixels: the window minus the
    /// player bar's padding, side blocks, gaps and time labels around it.
    fn wave_visual_px(&self) -> (u32, u32) {
        let scale = if self.window_scale > 0.1 {
            self.window_scale
        } else {
            1.0
        } * self.settings.zoom.clamp(0.7, 1.3);
        let w = (self.window_size.width - 2.0 * PB_WAVE_INSET).max(0.0);
        (
            (w * scale).round() as u32,
            (PB_WAVE_H * scale).round() as u32,
        )
    }

    /// Show `track`'s visual banner behind the waveform, if it has one. The
    /// track data usually says; a track from an older cache is looked up.
    fn load_wave_visual(&mut self, track: &Track) -> Task<Message> {
        let id = track.id;
        if self
            .wave_visual_src
            .as_ref()
            .is_some_and(|(src, _)| *src == id)
        {
            // the same track again (repeat, replay): its banner is still here
            return self.bake_wave_visual();
        }
        self.wave_visual_src = None;
        self.wave_visual = None;
        if self.story_playback {
            return Task::none();
        }
        let known = self
            .visual_urls
            .get(&id)
            .cloned()
            .or_else(|| track.visual_url().map(|u| u.map(str::to_string)));
        match known {
            Some(url) => {
                self.visual_urls.insert(id, url.clone());
                match url {
                    Some(url) => {
                        Task::perform(fetch_wave_visual(url, self.keeps_locally(id)), move |r| {
                            Message::WaveVisualLoaded(id, r)
                        })
                    }
                    None => Task::none(),
                }
            }
            None if self.settings.offline_mode => Task::none(),
            None => Task::perform(fetch_track_detail(id), move |r| {
                Message::WaveVisualResolved(
                    id,
                    r.map(|t| t.visual_url().flatten().map(str::to_string))
                        .map_err(|e| e.to_string()),
                )
            }),
        }
    }

    /// Bake the playing track's banner for the waveform's current size, off
    /// the UI thread. One bake runs at a time; the one landing starts the
    /// next if the window (or the track) changed meanwhile.
    fn bake_wave_visual(&mut self) -> Task<Message> {
        let Some((track_id, src)) = self.wave_visual_src.clone() else {
            self.wave_visual = None;
            return Task::none();
        };
        let size = self.wave_visual_px();
        if size.0 < 8 || size.1 < 8 || self.wave_visual_baking {
            return Task::none();
        }
        if self
            .wave_visual
            .as_ref()
            .is_some_and(|v| v.track_id == track_id && v.size == size)
        {
            return Task::none();
        }
        self.wave_visual_baking = true;
        let radius = PB_WAVE_RADIUS * size.1 as f32 / PB_WAVE_H;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || bake_visual(&src.0, size.0, size.1, radius))
                    .await
                    .ok()
                    .flatten()
                    .map(|px| image::Handle::from_rgba(size.0, size.1, px))
            },
            move |handle| Message::WaveVisualBaked(track_id, size, handle),
        )
    }

    /// Refetch of the user's playlists, tagged with the current generation
    /// (see Message::PlaylistsLoaded).
    fn fetch_playlists_task(&self) -> Task<Message> {
        let gen = self.playlists_gen;
        Task::perform(fetch_my_playlists(), move |r| {
            Message::PlaylistsLoaded(gen, r.map_err(|e| e.to_string()))
        })
    }

    /// A user the app has on screen somewhere (search, a profile's lists,
    /// the inspector), for Your Library's Artists before the refetch.
    fn known_user(&self, uid: i64) -> Option<UserMini> {
        let profile_lists = self.profile.iter().flat_map(|p| {
            p.followers_list
                .iter()
                .chain(&p.followings_list)
                .chain(&p.related)
        });
        self.search
            .users
            .iter()
            .chain(profile_lists)
            .chain(&self.inspector_favoriters)
            .chain(&self.inspector_reposters)
            .find(|u| u.id == uid)
            .cloned()
    }

    /// Your Library's Artists follow a follow or unfollow at once (the list
    /// is fetched again once SoundCloud has it).
    fn library_follow(&mut self, uid: i64, following: bool, user: Option<UserMini>) {
        if !following {
            self.my_followings.retain(|u| u.id != uid);
        } else if let Some(user) = user {
            if self.my_followings.iter().all(|u| u.id != uid) {
                self.my_followings.push(user);
                sort_followings(&mut self.my_followings, self.settings.artist_sort);
            }
        }
    }

    /// Refetch of the user's followed artists.
    fn fetch_followings_task(&self) -> Task<Message> {
        Task::perform(fetch_my_followings(), move |r| {
            Message::FollowingsLoaded(r.map_err(|e| e.to_string()))
        })
    }

    /// The user's own playlists: the only ones a track can be added to.
    fn own_playlists(&self) -> impl Iterator<Item = &Playlist> {
        let me = self.state.me.as_ref().map(|m| m.id);
        self.my_playlists.iter().filter(move |p| owned_by(p, me))
    }

    /// Downloads, which the storage clean-ups leave alone: the tracks, the
    /// covers of tracks and playlists, and the tracks' waveform banners.
    fn cache_keep(&self) -> CacheKeep {
        let store = &self.offline_store;
        CacheKeep {
            tracks: store.tracks.iter().map(|t| t.id).collect(),
            covers: store
                .tracks
                .iter()
                .flat_map(|t| [t.artwork_or_avatar(), t.visual_url().flatten()])
                .flatten()
                .chain(
                    store
                        .playlists
                        .iter()
                        .filter_map(|p| p.artwork_url.as_deref()),
                )
                .map(str::to_string)
                .collect(),
        }
    }

    /// Cache tracks locally turned off: the music it kept goes (downloads
    /// stay), off the UI thread; then Storage counts again.
    fn drop_streamed_audio(&self) -> Task<Message> {
        let keep = self.cache_keep();
        let playing = self.playing_id;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || remove_streamed_audio(&keep, playing))
                    .await
                    .unwrap_or(0)
            },
            Message::CacheCleared,
        )
    }

    /// Settings > Storage's numbers, counted off the UI thread.
    fn measure_storage(&self) -> Task<Message> {
        let keep = self.cache_keep();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || measure_storage(&keep))
                    .await
                    .unwrap_or((0, 0))
            },
            |(cache, downloads)| Message::StorageMeasured(cache, downloads),
        )
    }

    /// A track goes into (or out of) Liked Tracks: its heart, and the Liked
    /// Tracks list itself, which is otherwise only rebuilt by a full
    /// reload. `track` is its data when known (liking needs a row to show).
    fn set_liked(&mut self, track_id: i64, liked: bool, track: Option<Track>) {
        if liked {
            self.liked_ids.insert(track_id);
            if let Some(t) = track.or_else(|| self.find_track_anywhere(track_id)) {
                self.library.retain(|l| l.id != track_id);
                self.library.insert(0, t);
            }
        } else {
            self.liked_ids.remove(&track_id);
            self.library.retain(|l| l.id != track_id);
        }
        self.save_library_cache();
    }

    /// Saved anywhere (Liked Tracks or an own playlist). Drives the player
    /// bar button: outline "+" when not saved, filled check when saved.
    fn is_saved(&self, track_id: i64) -> bool {
        self.liked_ids.contains(&track_id)
            || self.own_playlists().any(|p| playlist_has(p, track_id))
    }

    /// Fresh playlists arrived while the popover is open: rows the user has
    /// not touched follow the server, touched rows keep the user's tick and
    /// only their baseline moves, so "Done" still writes the right diff.
    fn sync_add_popover(&mut self) {
        let me = self.state.me.as_ref().map(|m| m.id);
        let Some(pop) = self.add_popover.as_mut() else {
            return;
        };
        let tid = pop.track.id;
        for p in self.my_playlists.iter().filter(|p| owned_by(p, me)) {
            let has = playlist_has(p, tid);
            let touched = pop.picked.contains(&p.id) != pop.picked_was.contains(&p.id);
            if has {
                pop.picked_was.insert(p.id);
            } else {
                pop.picked_was.remove(&p.id);
            }
            if !touched {
                if has {
                    pop.picked.insert(p.id);
                } else {
                    pop.picked.remove(&p.id);
                }
            }
        }
    }

    /// Commits the popover's staged ticks ("Done"; "New playlist" commits
    /// them too, plus the new playlist). Local state changes first so the
    /// save button and a reopened popover agree at once; the refetch after
    /// the writes brings back the server's truth.
    fn apply_add_popover(&mut self, create: Option<String>) -> Task<Message> {
        let Some(pop) = self.add_popover.take() else {
            return Task::none();
        };
        let tid = pop.track.id;
        let like = (pop.liked != pop.liked_was).then_some(pop.liked);
        let mut add: Vec<i64> = pop.picked.difference(&pop.picked_was).copied().collect();
        let mut remove: Vec<i64> = pop.picked_was.difference(&pop.picked).copied().collect();
        add.sort_unstable();
        remove.sort_unstable();
        if like.is_none() && add.is_empty() && remove.is_empty() && create.is_none() {
            return Task::none();
        }

        let title_of = |id: i64| {
            self.my_playlists
                .iter()
                .find(|p| p.id == id)
                .map(|p| trunc(&p.title, 40))
                .unwrap_or_else(|| "playlist".to_string())
        };
        let mut added: Vec<String> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        match like {
            Some(true) => added.push("Liked Tracks".to_string()),
            Some(false) => removed.push("Liked Tracks".to_string()),
            None => {}
        }
        added.extend(add.iter().map(|&id| title_of(id)));
        removed.extend(remove.iter().map(|&id| title_of(id)));
        if let Some(t) = &create {
            added.push(trunc(t, 40));
        }

        if let Some(liked) = like {
            self.set_liked(tid, liked, Some(pop.track.clone()));
        }
        for p in self.my_playlists.iter_mut() {
            if add.contains(&p.id) {
                let ts = p.tracks.get_or_insert_with(Vec::new);
                if !ts.iter().any(|t| t.id == tid) {
                    ts.push(Track {
                        id: tid,
                        ..Default::default()
                    });
                    p.track_count = Some(p.track_count.unwrap_or(0) + 1);
                }
            } else if remove.contains(&p.id) {
                if let Some(ts) = p.tracks.as_mut() {
                    let before = ts.len();
                    ts.retain(|t| t.id != tid);
                    if ts.len() < before {
                        p.track_count = p.track_count.map(|c| c.saturating_sub(1));
                    }
                }
            }
        }

        crate::log!("save commit: track {tid} like={like:?} add={add:?} remove={remove:?} create={create:?}");
        if !add.is_empty() || !remove.is_empty() || create.is_some() {
            // same condition as SaveReport::playlists_touched
            self.saves_in_flight += 1;
            self.playlists_gen += 1;
        }
        let kind = if added.is_empty() {
            ToastKind::Info
        } else {
            ToastKind::Success
        };
        self.show_toast(save_summary(&added, &removed), kind);
        Task::perform(
            do_apply_saves(tid, like, add, remove, create),
            Message::SavesApplied,
        )
    }

    pub fn save_history(&self) {
        let history = self.history.clone();
        save_in_background(crate::config::history_path(), move || {
            serde_json::to_vec(&history).ok()
        });
    }

    pub fn load_history() -> Vec<Track> {
        if let Ok(j) = std::fs::read_to_string(crate::config::history_path()) {
            if let Ok(h) = serde_json::from_str::<Vec<Track>>(&j) {
                return h;
            }
        }
        Vec::new()
    }

    /// The playing track's comment set, for the next play (see fetch_comments).
    fn save_wave_comments(&self, track_id: i64) {
        if !self.keeps_locally(track_id) {
            return;
        }
        save_comments_cache(
            track_id,
            CommentsCache {
                items: self
                    .wave_comments
                    .iter()
                    .map(|c| (c.ts_ms, c.author.clone(), c.body.clone()))
                    .collect(),
                saved_at: crate::config::now_ms(),
            },
        );
    }

    fn load_playback_state() -> Option<PlaybackState> {
        let raw = std::fs::read_to_string(crate::config::playback_path()).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn save_playback_state(&mut self, force: bool) {
        if self.story_playback {
            return;
        }
        let Some(track) = self.queue.get(self.queue_pos).cloned() else {
            return;
        };
        let second = self.pos_ms / 1000;
        if !force && second == self.last_saved_playback_second {
            return;
        }
        self.last_saved_playback_second = second;
        if let Ok(json) = serde_json::to_string(&PlaybackState {
            track,
            pos_ms: self.pos_ms,
        }) {
            let path = crate::config::playback_path();
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &json)
                .and_then(|_| std::fs::rename(&tmp, &path))
                .is_err()
            {
                let _ = std::fs::remove_file(&tmp);
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn v_scrollable<'a>(
        &self,
        content: impl Into<Element<'a, Message>>,
    ) -> scrollable::Scrollable<'a, Message> {
        let bar = if self.settings.hide_scrollbars {
            scrollable::Scrollbar::new().width(0).scroller_width(0)
        } else {
            scrollable::Scrollbar::new().width(4).scroller_width(4)
        };
        scrollable(content).direction(scrollable::Direction::Vertical(bar))
    }

    pub fn h_scrollable<'a>(
        &self,
        content: impl Into<Element<'a, Message>>,
    ) -> scrollable::Scrollable<'a, Message> {
        let bar = if self.settings.hide_scrollbars {
            scrollable::Scrollbar::new().width(0).scroller_width(0)
        } else {
            scrollable::Scrollbar::new().width(4).scroller_width(4)
        };
        scrollable(content).direction(scrollable::Direction::Horizontal(bar))
    }

    pub fn scan_downloaded_track_ids() -> std::collections::HashSet<i64> {
        let mut ids = std::collections::HashSet::new();
        let dir = crate::config::audio_cache_dir();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("mp3") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(id) = stem.parse::<i64>() {
                            if let Ok(meta) = entry.metadata() {
                                if meta.len() > 10_000 {
                                    ids.insert(id);
                                }
                            }
                        }
                    }
                }
            }
        }
        ids
    }

    pub fn load_downloaded_playlists() -> std::collections::HashSet<String> {
        let p = crate::config::offline_playlists_path();
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(set) = serde_json::from_str::<std::collections::HashSet<String>>(&content) {
                return set;
            }
        }
        std::collections::HashSet::new()
    }

    pub fn save_downloaded_playlists(&self) {
        let ids = self.downloaded_playlist_ids.clone();
        save_in_background(crate::config::offline_playlists_path(), move || {
            serde_json::to_vec(&ids).ok()
        });
    }

    pub fn load_offline_store() -> OfflineStore {
        std::fs::read_to_string(crate::config::offline_store_path())
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default()
    }

    pub fn save_offline_store(&self) {
        let store = self.offline_store.clone();
        save_in_background(crate::config::offline_store_path(), move || {
            serde_json::to_vec(&store).ok()
        });
    }

    pub fn load_library_cache() -> Vec<Track> {
        std::fs::read_to_string(crate::config::library_cache_path())
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default()
    }

    pub fn save_library_cache(&self) {
        let library = self.library.clone();
        save_in_background(crate::config::library_cache_path(), move || {
            serde_json::to_vec(&library).ok()
        });
    }

    /// Keep a downloaded track's metadata, so offline mode can list it.
    fn remember_offline_track(&mut self, t: &Track) {
        if !self.offline_store.tracks.iter().any(|x| x.id == t.id) {
            self.offline_store.tracks.insert(0, t.clone());
        }
    }

    /// Keep the open playlist (with its track list) for offline mode.
    fn remember_offline_playlist(&mut self, id: i64) {
        let Some(cp) = self.current_playlist.as_ref().filter(|p| p.id == id) else {
            return;
        };
        let cp = cp.clone();
        self.offline_store.playlists.retain(|p| p.id != id);
        self.offline_store.playlists.insert(0, cp);
    }

    /// Your Library's mixes and stations out of a fresh Home: the mixes made
    /// for you ("Made for you", "Mixed for …") and every system playlist in
    /// "Recently Played", stations under Radio and the rest under Mixes.
    /// Radios started here stay, newest first, ahead of SoundCloud's.
    fn library_items_from_home(&mut self) {
        let mut mixes: Vec<LibraryItem> = Vec::new();
        let mut stations: Vec<LibraryItem> = Vec::new();
        for sec in &self.home {
            let HomeShelf::Playlists(pls) = &sec.shelf else {
                continue;
            };
            let played = sec.urn.contains(":recently-played");
            if !played && !sec.urn.contains(":made-for-you") && !sec.urn.contains(":your-moods") {
                continue;
            }
            for p in pls {
                if !p.id_or_urn.starts_with("soundcloud:system-playlists:") {
                    continue;
                }
                let list = if is_station_urn(&p.id_or_urn) {
                    &mut stations
                } else {
                    &mut mixes
                };
                if !list.iter().any(|i| i.urn == p.id_or_urn) {
                    // a station keeps the avatar it got when opened
                    let known = self
                        .library_radios
                        .iter()
                        .chain(&self.library_mixes)
                        .find(|i| i.urn == p.id_or_urn)
                        .and_then(|i| i.artwork_url.clone());
                    list.push(LibraryItem {
                        urn: p.id_or_urn.clone(),
                        title: p.title.clone(),
                        artwork_url: known.or_else(|| p.artwork_or_avatar().map(str::to_string)),
                        started_ms: 0,
                    });
                }
            }
        }
        // started here, or saved: they stay whatever Home shows now
        let keep = |i: &LibraryItem| i.started_ms > 0 || self.liked_system_urns.contains(&i.urn);
        let mut radios: Vec<LibraryItem> = self
            .library_radios
            .iter()
            .filter(|r| keep(r))
            .cloned()
            .collect();
        for st in stations {
            if !radios.iter().any(|r| r.urn == st.urn) {
                radios.push(st);
            }
        }
        let saved_mixes: Vec<LibraryItem> = self
            .library_mixes
            .iter()
            .filter(|m| keep(m) && mixes.iter().all(|x| x.urn != m.urn))
            .cloned()
            .collect();
        mixes.extend(saved_mixes);
        self.library_mixes = mixes;
        self.library_radios = radios;
        self.store_library_items();
    }

    /// A radio just started: first under Your Library's Radio (a dozen kept).
    fn remember_radio(&mut self, item: LibraryItem) {
        self.library_radios.retain(|r| r.urn != item.urn);
        self.library_radios.insert(0, item);
        let mut started = 0;
        self.library_radios.retain(|r| {
            started += usize::from(r.started_ms > 0);
            r.started_ms == 0 || started <= 12
        });
        self.store_library_items();
    }

    fn store_library_items(&mut self) {
        self.offline_store.mixes = self.library_mixes.clone();
        self.offline_store.radios = self.library_radios.clone();
        self.save_offline_store();
    }

    /// The tracks an entity page plays and downloads as a whole.
    fn collection_tracks(&self, c: Collection) -> &[Track] {
        match c {
            Collection::Playlist(id) => self
                .current_playlist
                .as_ref()
                .filter(|p| p.id == id)
                .or_else(|| self.offline_store.playlists.iter().find(|p| p.id == id))
                .map(|p| p.tracks.as_slice())
                .unwrap_or(&[]),
            Collection::Profile(id) => self
                .profile
                .as_ref()
                .filter(|p| p.id == id)
                .map(|p| {
                    if p.all_tracks.is_empty() {
                        &p.top_tracks
                    } else {
                        &p.all_tracks
                    }
                })
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            Collection::Liked => &self.library,
        }
    }

    /// Every track of the collection is on disk.
    fn collection_downloaded(&self, c: Collection) -> bool {
        let tracks = self.collection_tracks(c);
        let all_on_disk = !tracks.is_empty()
            && tracks
                .iter()
                .all(|t| self.downloaded_track_ids.contains(&t.id));
        all_on_disk
            || matches!(c, Collection::Playlist(id) if self.downloaded_playlist_ids.contains(&id.to_string()))
    }

    /// The queue is exactly this collection, so its Play button pauses.
    fn collection_playing(&self, c: Collection) -> bool {
        let tracks = self.collection_tracks(c);
        // the queue's own order: shuffle reorders the queue itself
        self.playing_id
            .is_some_and(|id| tracks.iter().any(|t| t.id == id))
            && self.queue_order.len() == tracks.len()
            && self.queue_order.iter().zip(tracks).all(|(a, b)| *a == b.id)
    }

    fn collection_dl_state(&self, c: Collection) -> DlState {
        if self.offline_downloading == Some(c) {
            DlState::Progress(
                self.offline_download_progress.0,
                self.offline_download_progress.1,
            )
        } else if self.collection_downloaded(c) {
            DlState::Done
        } else {
            DlState::Idle
        }
    }

    /// Play / Download / "..." of a collection page.
    fn collection_actions(&self, c: Collection) -> Element<'_, Message> {
        let save = self.save_button(c);
        let middle = match c {
            // an artist is followed, not downloaded (that is in "...")
            Collection::Profile(id) => follow_button(
                self.profile
                    .as_ref()
                    .is_some_and(|p| p.id == id && p.following),
            ),
            _ => download_button(
                self.collection_dl_state(c),
                if self.offline_downloading == Some(c) {
                    Message::CancelOfflineDownload
                } else {
                    Message::DownloadCollection(c)
                },
            ),
        };
        // Spotify's order: play, save (+), download, more
        let middle: Element<'_, Message> = match save {
            Some(save) => row![save, middle]
                .spacing(16)
                .align_y(iced::Alignment::Center)
                .into(),
            None => middle,
        };
        entity_actions(
            Message::PlayCollection(c, false),
            self.collection_playing(c) && !self.is_paused,
            middle,
            Message::OpenActionMenu(ActionMenu::Collection(c), MenuAnchor::Page),
            |b| self.with_menu(b, MenuAnchor::Page),
        )
    }

    /// Save to Your Library (+) of a playlist, mix or station page: a
    /// playlist's like, a mix's or station's save. None for your own
    /// playlists, signed out and offline.
    fn save_button(&self, c: Collection) -> Option<Element<'static, Message>> {
        let Collection::Playlist(id) = c else {
            return None;
        };
        if !self.state.authenticated || self.settings.offline_mode {
            return None;
        }
        let page = self.current_playlist.as_ref().filter(|p| p.id == id);
        let (saved, msg) = match page {
            Some(p) if is_system_playlist(&p.id_or_urn) => (
                self.liked_system_urns.contains(&p.id_or_urn),
                Message::SystemPlaylistLikeToggle(p.id_or_urn.clone()),
            ),
            _ if self.own_playlists().any(|p| p.id == id) => return None,
            _ => (
                self.liked_playlist_ids.contains(&id),
                Message::PlaylistLikeToggle(id),
            ),
        };
        const RING: f32 = 30.0;
        let glyph: Element<'static, Message> = if saved {
            // saved: the accent disc with a black check
            container(
                text(icons::CHECK)
                    .font(FA_SOLID)
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| t_color(Color::BLACK)),
            )
            .center_x(Length::Fixed(RING))
            .center_y(Length::Fixed(RING))
            .style(|_| container::Style {
                background: Some(Background::Color(ORANGE)),
                border: round(RING / 2.0),
                ..container::Style::default()
            })
            .into()
        } else {
            // not yet: an outlined ring with a plus, brighter on hover
            container(
                text(icons::PLUS)
                    .font(FA_SOLID)
                    .size(14)
                    .wrapping(text::Wrapping::None),
            )
            .center_x(Length::Fixed(RING))
            .center_y(Length::Fixed(RING))
            .style(|_| container::Style {
                border: Border {
                    radius: border::Radius::from(RING / 2.0),
                    width: 2.0,
                    color: TEXT_DIM,
                },
                ..container::Style::default()
            })
            .into()
        };
        let btn = button(
            container(glyph)
                .center_x(Length::Fixed(HEADER_BTN))
                .center_y(Length::Fixed(HEADER_BTN)),
        )
        .on_press(msg)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: match status {
                button::Status::Hovered => TEXT,
                _ => TEXT_DIM,
            },
            border: round(HEADER_BTN / 2.0),
            ..button::Style::default()
        });
        Some(header_tooltip(
            btn,
            if saved {
                "Remove from Your Library".to_string()
            } else {
                "Save to Your Library".to_string()
            },
        ))
    }

    /// Downloaded tracks whose metadata is known, newest downloads first.
    fn offline_tracks(&self) -> Vec<Track> {
        let mut seen = std::collections::HashSet::new();
        self.offline_store
            .tracks
            .iter()
            .chain(
                self.offline_store
                    .playlists
                    .iter()
                    .flat_map(|p| p.tracks.iter()),
            )
            .chain(self.library.iter())
            .filter(|t| self.downloaded_track_ids.contains(&t.id) && seen.insert(t.id))
            .cloned()
            .collect()
    }

    /// Home in offline mode: what is on disk instead of the network feed.
    fn offline_home(&self) -> Vec<HomeSection> {
        let mut out = Vec::new();
        let playlists: Vec<HomePlaylist> = self
            .offline_store
            .playlists
            .iter()
            .map(|p| HomePlaylist {
                id_or_urn: p.id.to_string(),
                title: p.title.clone(),
                subtitle: p.author.clone(),
                artwork_url: p.artwork_url.clone(),
                tracks: p.tracks.clone(),
                owner: p.author_id.map(|id| UserMini {
                    id,
                    username: p.author.clone(),
                    avatar_url: p.author_avatar.clone(),
                    ..Default::default()
                }),
            })
            .collect();
        if !playlists.is_empty() {
            out.push(HomeSection {
                title: "Downloaded playlists".into(),
                urn: String::new(),
                shelf: HomeShelf::Playlists(playlists),
            });
        }
        let tracks = self.offline_tracks();
        if !tracks.is_empty() {
            out.push(HomeSection {
                title: "Downloaded tracks".into(),
                urn: String::new(),
                shelf: HomeShelf::Tracks(tracks),
            });
        }
        out
    }

    /// Offline mode: Home lists the downloads and nothing asks the network.
    fn go_offline(&mut self) {
        self.login_error = None;
        if self.state.me.is_none() {
            // the account is still the one signed in, just unreachable
            self.state.me = self.offline_store.me.clone();
            self.state.authenticated = self.state.me.is_some();
        }
        // tracks the player cached while streaming are playable too
        self.downloaded_track_ids = App::scan_downloaded_track_ids();
        self.home = self.offline_home();
        self.home_loading = false;
    }

    /// Online (at startup or leaving offline mode): sign in, load the feed.
    fn go_online(&mut self) -> Task<Message> {
        self.home.clear();
        Task::batch(vec![
            Task::perform(init_api(), |r| match r {
                Ok(s) => Message::ApiReady(Ok(s)),
                Err(e) => Message::ApiReady(Err(e.to_string())),
            }),
            self.load_home(),
        ])
    }

    /// Fetch the Home feed; Home shows a spinner until it lands.
    fn load_home(&mut self) -> Task<Message> {
        self.home_loading = true;
        self.home_error = None;
        Task::perform(fetch_home(), |r| match r {
            Ok(c) => Message::HomeLoaded(Ok(c)),
            Err(e) => Message::HomeLoaded(Err(e.to_string())),
        })
    }

    /// Run a search (network, or the downloads offline); the Search page
    /// shows a spinner until this generation's results land.
    fn start_search(&mut self, q: String) -> Task<Message> {
        self.search_gen = self.search_gen.wrapping_add(1);
        let gen = self.search_gen;
        self.page_error = None;
        if self.settings.offline_mode {
            let res = self.offline_search(&q);
            return self.update(Message::SearchLoaded(gen, Ok((res, None))));
        }
        self.search_loading = true;
        let cid = self.cid();
        Task::perform(fetch_search(q, cid), move |r| {
            Message::SearchLoaded(gen, r.map_err(|e| e.to_string()))
        })
    }

    pub fn load_full_versions() -> std::collections::HashMap<i64, String> {
        std::fs::read_to_string(crate::config::full_versions_path())
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default()
    }

    /// A track downloaded for offline listening.
    fn is_downloaded(&self, id: i64) -> bool {
        self.offline_store.tracks.iter().any(|t| t.id == id)
    }

    /// Kept on this computer: downloaded, or played with Cache tracks
    /// locally on (its audio, waveform, banner and comments stay on disk).
    fn keeps_locally(&self, id: i64) -> bool {
        self.settings.cache_tracks || self.is_downloaded(id)
    }

    /// Unlock through YouTube Music: a Go+ track, of which SoundCloud streams
    /// only a 30 s preview, plays there in full (unless a full version
    /// cached by an earlier Wavify is on disk).
    fn plays_on_youtube(&self, t: &Track) -> bool {
        self.youtube_usable()
            && crate::alt_source::is_preview_only(t)
            && !(self.full_versions.contains_key(&t.id) && has_cached_audio(t.id))
    }

    /// How long the track plays: in full when its full version is cached
    /// (downloaded by an earlier Wavify), else what SoundCloud streams (30 s
    /// for a Go+ preview).
    fn track_ms(&self, t: &Track) -> Option<u64> {
        if self.full_versions.contains_key(&t.id) {
            t.full_duration.or(t.duration)
        } else {
            playable_ms(t)
        }
    }

    /// YouTube Music can stand in for SoundCloud. Its web player also runs
    /// signed out; a Google sign-in is for when YouTube asks for one (and
    /// Premium drops the ads).
    fn youtube_usable(&self) -> bool {
        self.settings.youtube_music && !self.settings.offline_mode
    }

    /// Find the playing track on YouTube Music and play it there.
    fn start_youtube(&mut self, gen: u64, track: Track, fallback: YtFallback) -> Task<Message> {
        self.show_toast("Looking on YouTube Music…", ToastKind::Info);
        let id = track.id;
        Task::perform(
            async move {
                crate::alt_source::find_on_youtube_music(&track)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| Message::YtFound(gen, id, r, fallback),
        )
    }

    /// A YouTube Music player event: its position is the track's, its end
    /// is the track's end, and a lost session is reported once.
    fn on_youtube_event(&mut self, ev: crate::yt_web::PageEvent) -> Task<Message> {
        use crate::yt_web::PageEvent;
        let ours = |vid: &Option<String>, me: &Self| {
            crate::yt_music::is_active()
                && me
                    .yt_video
                    .as_ref()
                    .is_some_and(|(_, v)| vid.as_deref() == Some(v.as_str()))
        };
        match ev {
            PageEvent::Status {
                vid, pos, dur, ad, ..
            } if ours(&vid, self) => {
                self.yt_ad = ad;
                if ad && self.awaiting_audio.is_some() {
                    // the player works, the song comes after the ad
                    self.awaiting_audio = Some(std::time::Instant::now());
                }
                if !ad {
                    self.pos_ms = (pos * 1000.0) as u64;
                    if dur > 0.0 {
                        self.dur_ms = (dur * 1000.0) as u64;
                    }
                    if self.pos_ms > 0 {
                        self.awaiting_audio = None;
                        self.play_failures = 0;
                    }
                }
                Task::none()
            }
            PageEvent::Ended { vid } if ours(&vid, self) && !self.is_paused => {
                crate::log!("youtube: song ended");
                self.at_end = true;
                self.update(Message::TrackEnded)
            }
            PageEvent::Account { signed_in: false } if self.yt_signed_in => {
                // the Google session ended (signed out elsewhere, expired)
                self.yt_signed_in = false;
                let _ = std::fs::remove_file(crate::yt_web::signed_in_marker());
                self.show_toast(
                    "YouTube Music session ended: sign in again in Settings",
                    ToastKind::Error,
                );
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Stop a track that plays from YouTube Music (before its player process
    /// goes away for a sign-in or sign-out).
    fn stop_youtube_track(&mut self) {
        if crate::yt_music::is_active() {
            if let Some(p) = &self.player {
                p.send(PlayerCommand::Stop);
            }
            self.yt_video = None;
            self.yt_ad = false;
            self.is_paused = true;
            self.at_end = true;
            self.update_discord_rpc();
        }
    }

    /// Resolve the playing track through a public proxy (it's unavailable
    /// in this country). `dead`: a proxy that just failed it, dropped first.
    fn start_bypass(&mut self, gen: u64, track_id: i64, dead: Option<String>) -> Task<Message> {
        self.bypass_attempts += 1;
        self.bypass_proxy = None;
        self.awaiting_audio = None;
        self.show_toast(
            "Unavailable in your region: trying through a proxy…",
            ToastKind::Info,
        );
        let quality = self
            .settings
            .audio_quality
            .clone()
            .unwrap_or_else(|| "hls".into());
        Task::perform(
            async move {
                if let Some(p) = dead {
                    crate::proxy_pool::mark_dead(&p).await;
                }
                crate::proxy_pool::resolve_via_proxies(track_id, &quality)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| Message::BypassReady(gen, track_id, r),
        )
    }

    /// Fetch the reactions coming up, as the Android app polls them: when
    /// the next second not asked for yet is within 5 s of the playhead, ask
    /// for the next minute from there (up to the track's end).
    fn poll_reactions(&mut self) -> Task<Message> {
        let Some(track_id) = self.playing_id else {
            return Task::none();
        };
        if self.reactions_inflight
            || self.is_paused
            || !self.state.authenticated
            || self.settings.offline_mode
        {
            return Task::none();
        }
        let now = self.pos_ms / 1000;
        let end = (self.dur_ms / 1000).max(1);
        let Some(gap) = (now..end).find(|s| !self.reactions_fetched.contains(s)) else {
            return Task::none();
        };
        if gap - now > REACTION_LOOKAHEAD_S {
            return Task::none();
        }
        let seconds: Vec<u64> = (gap..(gap + REACTION_WINDOW_S).min(end))
            .filter(|s| !self.reactions_fetched.contains(s))
            .collect();
        self.reactions_inflight = true;
        let asked = seconds.clone();
        Task::perform(fetch_reactions(track_id, seconds), move |r| {
            Message::ReactionsLoaded(track_id, asked.clone(), r)
        })
    }

    /// Each second the playhead enters gets its reaction floated up, once.
    /// A jump (seek, resume after a stall) starts from where it landed
    /// instead of firing everything skipped over.
    fn float_reactions(&mut self) {
        let now = std::time::Instant::now();
        self.floating
            .retain(|r| now.duration_since(r.spawned_at).as_millis() as f32 <= REACTION_FLOAT_MS);
        self.particles
            .retain(|p| now.duration_since(p.spawned_at).as_millis() as f32 <= p.life_ms);
        if self.is_paused || self.dur_ms == 0 {
            return;
        }
        let sec = self.pos_ms / 1000;
        let from = match self.reaction_sec {
            Some(last) if sec > last && sec - last <= 2 => last + 1,
            Some(last) if sec == last => return,
            _ => sec,
        };
        self.reaction_sec = Some(sec);
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for s in from..=sec {
            let Some(list) = self.reactions.get(&s) else {
                continue;
            };
            // spread over their second, each leaving from where the playhead
            // will be then
            let n = list.len().max(1) as u64;
            for (i, r) in list.iter().enumerate() {
                let at_ms = (s * 1000 + 1000 * i as u64 / n).max(self.pos_ms);
                self.floating.push(FloatingReaction {
                    spawned_at: now + std::time::Duration::from_millis(at_ms - self.pos_ms),
                    codepoint: r.codepoint.clone(),
                    x_frac: (at_ms as f32 / self.dur_ms as f32).clamp(0.0, 1.0),
                    drift: rng.gen_range(-1.0..1.0),
                });
            }
        }
    }

    /// The profile's open sub-tab is still loading.
    fn profile_tab_loading(&self) -> bool {
        self.profile
            .as_ref()
            .is_some_and(|p| self.profile_tabs_loading.contains(&(p.id, p.active_tab)))
    }

    /// Anything a page is waiting for (drives the spinner's animation).
    /// A page on screen shows a spinner (it turns on the animation tick);
    /// loads nobody sees, like the likes refetch under the cached library,
    /// don't count.
    fn spinner_visible(&self) -> bool {
        match self.tab {
            Tab::Playlist => self.loading_playlist.is_some(),
            Tab::Profile => self.loading_profile.is_some() || self.profile_tab_loading(),
            Tab::Search => self.search_loading,
            Tab::Library => self.library_loading && self.library.is_empty(),
            Tab::Track => self
                .track_page
                .as_ref()
                .is_some_and(|p| !p.pending.is_empty()),
            Tab::Home => self.home_loading && self.home.is_empty(),
            Tab::Settings => false,
        }
    }

    /// While playing: the interval that moves the waveform's playhead about
    /// a pixel (40..250 ms); None when the 250ms Tick does as well.
    fn playhead_frame_ms(&self) -> Option<u64> {
        if self.is_paused || self.playing_id.is_none() || self.dur_ms == 0 {
            return None;
        }
        let px = (self.window_size.width - 2.0 * PB_WAVE_INSET).max(1.0) as u64;
        let speed = f64::from(self.playback_speed.max(0.1));
        let ms = ((self.dur_ms / px) as f64 / speed) as u64;
        (ms < 250).then(|| ms.max(40))
    }

    /// Search in offline mode: title / artist matches among downloads.
    fn offline_search(&self, q: &str) -> SearchResults {
        let q = q.to_lowercase();
        let prefer = self.settings.prefer_artist_from_name;
        let tracks = self
            .offline_tracks()
            .into_iter()
            .filter(|t| {
                let (artist, title) = t.display_artist_and_title(prefer);
                let uploader = t.user.as_ref().map(|u| u.username.as_str()).unwrap_or("");
                [artist.as_str(), title.as_str(), uploader]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&q))
            })
            .collect();
        let playlists = self
            .offline_store
            .playlists
            .iter()
            .filter(|p| p.title.to_lowercase().contains(&q) || p.author.to_lowercase().contains(&q))
            .map(|p| Playlist {
                id: p.id,
                title: p.title.clone(),
                artwork_url: p.artwork_url.clone(),
                track_count: Some(p.tracks.len() as u64),
                is_album: Some(p.is_album),
                tracks: Some(p.tracks.clone()),
                ..Playlist::default()
            })
            .collect();
        SearchResults {
            tracks,
            users: vec![],
            playlists,
        }
    }

    /// Next track of the running collection download: one neither on disk
    /// nor tried in this run. Also recounts the progress total.
    fn next_offline_track(&mut self) -> Option<Track> {
        // A Home or search preview holds only part of a playlist: take in
        // the tracks its full load added, while it's still the open page.
        if let Some(c) = self.offline_downloading {
            let planned: std::collections::HashSet<i64> =
                self.offline_plan.iter().map(|t| t.id).collect();
            let added: Vec<Track> = self
                .collection_tracks(c)
                .iter()
                .filter(|t| !planned.contains(&t.id))
                .cloned()
                .collect();
            self.offline_plan.extend(added);
        }
        let (done, tried) = (&self.downloaded_track_ids, &self.offline_attempted);
        let mut pending = self
            .offline_plan
            .iter()
            .filter(|t| !done.contains(&t.id) && !tried.contains(&t.id));
        let next = pending.next().cloned();
        let left = usize::from(next.is_some()) + pending.count();
        self.offline_download_progress.1 = self.offline_download_progress.0 + left;
        next
    }

    pub fn save_stories(&self) {
        let stories = self.stories.clone();
        save_in_background(crate::config::stories_path(), move || {
            serde_json::to_vec(&stories).ok()
        });
    }

    pub fn load_stories() -> Vec<StoryItem> {
        if let Ok(j) = std::fs::read_to_string(crate::config::stories_path()) {
            if let Ok(mut s) = serde_json::from_str::<Vec<StoryItem>>(&j) {
                // SoundCloud expires artist-shortcut updates after 7 days.
                let cutoff = crate::config::now_ms().saturating_sub(STORY_TTL_MS);
                s.retain(|st| st.created_at_ms == 0 || st.created_at_ms >= cutoff);
                if !s.is_empty() {
                    return s;
                }
            }
        }
        Vec::new()
    }

    pub fn save_stories_read(&self) {
        let read = self.stories_read.clone();
        save_in_background(crate::config::stories_read_path(), move || {
            serde_json::to_vec(&read).ok()
        });
    }

    pub fn load_stories_read() -> std::collections::HashSet<i64> {
        if let Ok(j) = std::fs::read_to_string(crate::config::stories_read_path()) {
            if let Ok(s) = serde_json::from_str(&j) {
                return s;
            }
        }
        std::collections::HashSet::new()
    }

    /// Open story `idx`: request its SoundCloud read receipt and autoplay its track (SoundCloud
    /// plays a preview snippet; `Tick` advances to the next story when the
    /// preview window elapses).
    fn open_story(&mut self, idx: usize) -> Task<Message> {
        if idx >= self.stories.len() {
            return Task::none();
        }
        self.active_story_index = Some(idx);
        let mut tasks = vec![self.sync_story_window()];
        let story = self.stories[idx].clone();
        tasks.push(self.play_story_track(story.track.clone()));
        // Older builds marked stories read optimistically. Opening one now
        // clears that local state until the server confirms the receipt.
        if self.stories_read.remove(&story.track_id) {
            self.save_stories_read();
        }
        let key = StoryReceiptKey {
            user_id: story.user_id,
            track_id: story.track_id,
            created_ms: story.created_at_ms,
        };
        if self.state.authenticated && self.story_receipts_pending.insert(key) {
            tasks.push(Task::perform(
                mark_story_read(key.user_id, key.created_ms),
                move |result| Message::StoryReceiptDone(key, 0, result),
            ));
        }
        Task::batch(tasks)
    }

    /// Stories are rendered as a centered in-window overlay; the host window
    /// never changes size or mode.
    fn sync_story_window(&mut self) -> Task<Message> {
        // Stories are an in-window overlay. Keeping the existing window size
        // preserves the app context behind the dimmed/blurred presentation.
        Task::none()
    }

    fn close_story(&mut self) -> Task<Message> {
        self.active_story_index = None;
        if self.story_playback {
            let return_state = self.story_return_state.take();
            if let Some(p) = &self.player {
                p.send(PlayerCommand::Stop);
            }
            // the story's stream may still be resolving: it mustn't start
            // (paused, under an empty bar) once the story is gone
            self.play_gen = self.play_gen.wrapping_add(1);
            self.story_playback = false;
            self.pending_seek_ms = None;
            self.pending_seek_track = None;
            self.playing_id = None;
            self.playing_title.clear();
            self.playing_artist.clear();
            self.queue.clear();
            self.queue_pos = 0;
            self.is_paused = true;
            self.at_end = false;
            self.pos_ms = 0;
            self.dur_ms = 0;
            self.wave_bars = std::sync::Arc::new(Vec::new());
            self.wave_comments.clear();
            self.update_discord_rpc();
            if let Some(saved) = return_state {
                self.queue = saved.queue;
                if self.queue.is_empty() {
                    self.queue_pos = 0;
                } else {
                    self.queue_pos = saved.queue_pos.min(self.queue.len() - 1);
                    self.pending_seek_ms = Some(saved.pos_ms);
                    self.pending_seek_track = Some(self.queue[self.queue_pos].id);
                }
                let task = if self.queue.is_empty() {
                    Task::none()
                } else {
                    self.play_index(self.queue_pos)
                };
                self.is_paused = saved.was_paused;
                self.update_discord_rpc();
                return task;
            }
        }
        self.sync_story_window()
    }
}

/// Stories expire after 7 days, like SoundCloud artist shortcuts.
const STORY_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// No audio this long after the play command means the player gave up on the
/// track (its decoder waits at most 15s for data) and nothing will play.
const STALL_MS: u64 = 20_000;
/// A YouTube Music song still silent this long gets a new player process
/// once, before STALL_MS gives up on it.
const YT_STALL_MS: u64 = 10_000;

/// Length of the audio that actually streams. For preview-only (SNIP, Go+)
/// tracks `duration` is the ~30s preview and `full_duration` the whole song;
/// otherwise the two are equal.
/// Streamed audio past this is trimmed at startup, oldest first.
const AUDIO_CACHE_MAX: u64 = 2 * 1024 * 1024 * 1024;
/// Cover art past this is trimmed at startup, oldest first (a cover that
/// is shown again downloads again).
const ARTWORK_CACHE_MAX: u64 = 300 * 1024 * 1024;
/// Comment sets older than this are dropped at startup (they refresh daily).
const COMMENTS_CACHE_DAYS: u64 = 30;

/// What the storage clean-ups leave alone: the downloads. Their audio,
/// waveforms and comments go by track id, their covers (and waveform
/// banners) by URL.
struct CacheKeep {
    tracks: std::collections::HashSet<i64>,
    covers: Vec<String>,
}

/// A cache file: path, size, last written, whether a download's, kind.
type CacheFile = (
    std::path::PathBuf,
    u64,
    std::time::SystemTime,
    bool,
    &'static str,
);

/// Every file of the caches (streamed audio, waveforms and comment sets,
/// each named by its track id; cover art, named by its URL's hash), with
/// its size and age, and whether it belongs to a download: those are never
/// trimmed or cleared.
fn cache_files(keep: &CacheKeep) -> Vec<CacheFile> {
    // a download's cover in every size a tile may ask for
    let mut covers = std::collections::HashSet::new();
    for url in &keep.covers {
        covers.insert(crate::config::cached_artwork_path(url));
        for px in [100, 300, 500] {
            if let Some(v) = art_variant_url(url, px) {
                covers.insert(crate::config::cached_artwork_path(&v));
            }
        }
    }
    let mut files = Vec::new();
    for (dir, kind) in [
        (crate::config::audio_cache_dir(), "audio"),
        (crate::config::waveform_cache_dir(), "waveform"),
        (crate::config::comments_cache_dir(), "comments"),
        (crate::config::track_info_dir(), "info"),
        (crate::config::artwork_cache_dir(), "artwork"),
    ] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let path = entry.path();
            let kept = if kind == "artwork" {
                covers.contains(&path)
            } else {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .is_some_and(|id| keep.tracks.contains(&id))
            };
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            files.push((path, meta.len(), modified, kept, kind));
        }
    }
    files
}

/// Settings > Storage: (bytes of cache, bytes of downloads).
fn measure_storage(keep: &CacheKeep) -> (u64, u64) {
    cache_files(keep)
        .iter()
        .fold((0, 0), |(cache, downloads), (_, size, _, kept, _)| {
            if *kept {
                (cache, downloads + size)
            } else {
                (cache + size, downloads)
            }
        })
}

/// "Clear cache": everything but the downloads. Bytes freed.
fn clear_cache(keep: &CacheKeep) -> u64 {
    cache_files(keep)
        .into_iter()
        .filter(|(_, _, _, kept, _)| !kept)
        .filter(|(path, ..)| std::fs::remove_file(path).is_ok())
        .map(|(_, size, ..)| size)
        .sum()
}

/// At startup, off the UI thread: streamed audio over AUDIO_CACHE_MAX and
/// cover art over ARTWORK_CACHE_MAX go, oldest first, and comment sets
/// older than COMMENTS_CACHE_DAYS.
fn trim_cache(keep: &CacheKeep, cache_tracks: bool) {
    // Cache tracks locally off: no music is kept but the downloads
    if !cache_tracks {
        remove_streamed_audio(keep, None);
    }
    let mut files = cache_files(keep);
    files.retain(|(_, _, _, kept, _)| !kept);
    let old = std::time::SystemTime::now()
        - std::time::Duration::from_secs(COMMENTS_CACHE_DAYS * 24 * 60 * 60);
    let mut freed = 0u64;
    for (path, size, modified, _, kind) in &files {
        if *kind == "comments" && *modified < old && std::fs::remove_file(path).is_ok() {
            freed += size;
        }
    }
    for (kind, max) in [("audio", AUDIO_CACHE_MAX), ("artwork", ARTWORK_CACHE_MAX)] {
        let mut group: Vec<_> = files.iter().filter(|f| f.4 == kind).collect();
        group.sort_by(|a, b| b.2.cmp(&a.2)); // newest first
        let mut total = 0u64;
        for (path, size, ..) in group {
            total += size;
            if total > max && std::fs::remove_file(path).is_ok() {
                freed += size;
            }
        }
    }
    if freed > 0 {
        crate::log!("cache: trimmed {} MB", freed / (1024 * 1024));
    }
}

/// The music streamed and kept (Cache tracks locally), and the tracks'
/// details, all but the downloads' and the playing track's (its file is
/// open). Bytes freed.
fn remove_streamed_audio(keep: &CacheKeep, playing: Option<i64>) -> u64 {
    let freed: u64 = cache_files(keep)
        .into_iter()
        .filter(|(_, _, _, kept, kind)| !kept && matches!(*kind, "audio" | "info"))
        .filter(|(path, ..)| {
            playing.is_none_or(|id| {
                path.file_stem().and_then(|s| s.to_str()) != Some(id.to_string().as_str())
            })
        })
        .filter(|(path, ..)| std::fs::remove_file(path).is_ok())
        .map(|(_, size, ..)| size)
        .sum();
    if freed > 0 {
        crate::log!(
            "cache: {} MB of streamed music removed",
            freed / (1024 * 1024)
        );
    }
    freed
}

/// "1.2 GB", "340 MB", "12 KB".
fn fmt_bytes(n: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let n = n as f64;
    if n >= 1024.0 * MB {
        format!("{:.1} GB", n / (1024.0 * MB))
    } else if n >= MB {
        format!("{:.0} MB", n / MB)
    } else {
        format!("{:.0} KB", (n / 1024.0).ceil())
    }
}

/// The track's audio is in the disk cache (downloaded, or cached by a play).
fn has_cached_audio(track_id: i64) -> bool {
    crate::config::cached_audio_path(track_id)
        .metadata()
        .is_ok_and(|m| m.len() > 10_000)
}

fn playable_ms(t: &Track) -> Option<u64> {
    t.duration.or(t.full_duration)
}

fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Downsample raw waveform samples to the bar count drawn on the canvas
/// (240 bars). Done once per track load instead of on every rendered frame.
fn compute_wave_bars(wave: &[u8]) -> Vec<f32> {
    let target_bars = 240usize;
    let mut bars: Vec<f32> = Vec::with_capacity(target_bars);
    if !wave.is_empty() {
        let n_samples = wave.len();
        let bucket = (n_samples + target_bars - 1) / target_bars;
        let mut i = 0usize;
        while i < n_samples {
            let end = (i + bucket).min(n_samples);
            let peak = wave[i..end].iter().copied().max().unwrap_or(0);
            bars.push((peak as f32 / 140.0).clamp(0.03, 1.0));
            i = end;
        }
    }
    bars
}

fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// The main window's size and position when it opens: most of the work area
/// (the taskbar left out), centred in it, in logical pixels. It opens like
/// that straight away, with no splash growing into it.
fn initial_window() -> (Size, Point) {
    let (wx, wy, pw, ph) = screen::get_work_area();
    // Until winit makes the process DPI-aware (when it opens the window) the
    // work area and the screen DPI both come in 96-DPI units; were it aware
    // already, both would be physical. Either way this gives logical pixels.
    let scale = screen::dpi_scale();
    let (x, y, w, h) = (wx / scale, wy / scale, pw / scale, ph / scale);
    let width = (w * 0.86).clamp(860.0, 1400.0).min(w - 40.0).max(400.0);
    let height = (h * 0.88).clamp(540.0, 860.0).min(h - 40.0).max(300.0);
    (
        Size::new(width, height),
        Point::new(x + (w - width) / 2.0, y + (h - height) / 2.0),
    )
}

/// Wavify's icon in RGBA (64px) for the windows' own icons (taskbar button,
/// Alt+Tab); the exe carries every size (build.rs).
pub fn app_icon_rgba() -> Option<(Vec<u8>, u32, u32)> {
    let img = ::image::load_from_memory(include_bytes!("../assets/icon/wavify-64.png"))
        .ok()?
        .into_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

/// The same icon for the WebView windows (sign-in, captcha).
pub fn webview_window_icon() -> Option<wry::application::window::Icon> {
    app_icon_rgba()
        .and_then(|(rgba, w, h)| wry::application::window::Icon::from_rgba(rgba, w, h).ok())
}

pub fn run(settings: Settings, start_minimized: bool) {
    let (window_size, window_pos) = initial_window();
    // sizes inside the app are in zoomed units (see Pref::Zoom)
    let zoom = settings.zoom.clamp(0.7, 1.3);
    let app_size = Size::new(window_size.width / zoom, window_size.height / zoom);
    if settings.open_at_login != crate::config::OpenAtLogin::No {
        // the entry follows the exe if it moved
        let _ = crate::config::set_open_at_login(settings.open_at_login);
    }
    let icon =
        app_icon_rgba().and_then(|(rgba, w, h)| iced::window::icon::from_rgba(rgba, w, h).ok());
    let _ = iced::application("Wavify", App::update, App::view)
        .default_font(iced::Font::with_name("Segoe UI"))
        .font(include_bytes!("../fonts/fa-solid-900.ttf").as_slice())
        .font(include_bytes!("../fonts/fa-regular-400.ttf").as_slice())
        .font(include_bytes!("../fonts/fa-brands-400.ttf").as_slice())
        // Segoe UI Emoji — the only font here with colour emoji glyphs. Without
        // registering it the canvas renders emoji as invisible tofus (system
        // font fallback doesn't apply to embedded canvas fonts).
        .font(include_bytes!("../fonts/seguiemj.ttf").as_slice())
        .subscription(App::subscription)
        .scale_factor(|app: &App| f64::from(app.settings.zoom.clamp(0.7, 1.3)))
        .window(iced::window::Settings {
            size: window_size,
            position: iced::window::Position::Specific(window_pos),
            decorations: false,
            icon,
            // Alt+F4 and the taskbar's Close come to WindowClose, which saves
            // the playback position and speeds first
            exit_on_close_request: false,
            ..Default::default()
        })
        // MSAA for canvas geometry (volume knob, waveform, markers); without
        // it every circle and thin line is rasterised with hard stair-steps.
        .antialiasing(true)
        .run_with(move || {
            let init_vol = settings.volume.unwrap_or(0.7).clamp(0.0, 1.0);
            // A saved 0 (volume turned all the way down) starts muted, so M
            // unmutes to a usable level instead of "muting" silence.
            let start_muted = init_vol <= 0.005;
            let offline_store = App::load_offline_store();
            let resume_state = App::load_playback_state();
            let app = App {
                login_error: None,
                cookie_input: String::new(),
                proxy_draft: settings.proxy.clone().unwrap_or_default(),
                confirm_remove_downloads: false,
                start_minimized,
                crossfade_next: false,
                crossfaded_from: None,
                profile_banner: None,
                tab: Tab::Home,
                nav_history: vec![],
                nav_future: vec![],
                nav_restoring: false,
                search_query: String::new(),
                state: ApiStateClone {
                    client_id: settings
                        .client_id_override
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| CLIENT_ID.to_string()),
                    me: None,
                    authenticated: false,
                    can_write: false,
                },
                search: SearchResults {
                    tracks: vec![],
                    users: vec![],
                    playlists: vec![],
                },
                home: vec![],
                current_playlist: None,
                track_page: None,
                library: vec![],
                queue: vec![],
                queue_pos: 0,
                liked_ids: Default::default(),
                my_following_ids: Default::default(),
                playing_id: None,
                playing_title: String::new(),
                playing_artist: String::new(),
                is_paused: true,
                story_playback: false,
                story_return_state: None,
                pending_seek_ms: None,
                pending_seek_track: None,
                last_saved_playback_second: 0,
                volume: init_vol,
                loading_playlist: None,
                loading_profile: None,
                search_gen: 0,
                search_loading: false,
                library_loading: false,
                home_loading: false,
                home_error: None,
                profile_tabs_loading: Default::default(),
                page_error: None,
                login_pending: false,
                player: None,
                prefetched_stream: None,
                prefetched_at: None,
                prefetch_in_progress: None,
                play_gen: 0,
                bypass_proxy: None,
                bypass_attempts: 0,
                full_versions: App::load_full_versions(),
                yt_video: None,
                yt_ad: false,
                yt_signed_in: crate::yt_music::signed_in(),
                yt_restarted: false,
                at_end: false,
                awaiting_audio: None,
                play_failures: 0,
                related_seed: None,
                queue_order: Vec::new(),
                artwork: std::collections::HashMap::new(),
                artwork_sizes: std::collections::HashMap::new(),
                artwork_bytes: 0,
                art_wanted: std::cell::RefCell::new(std::collections::HashMap::new()),
                art_inflight: std::collections::HashSet::new(),
                art_failed: std::collections::HashMap::new(),
                art_touched: std::cell::RefCell::new(std::collections::HashSet::new()),
                wave_bars: std::sync::Arc::new(Vec::new()),
                last_pos_poll: std::time::Instant::now(),
                show_queue: false,
                radio_request: None,
                radio_gen: 0,
                discord_rpc: Some(DiscordRpcHandle::start()),
                pos_ms: 0,
                dur_ms: 0,
                hover_frac: None,
                wave_context_frac: None,
                wave_context_comment: String::new(),
                wave_comments: vec![],
                reactions: Default::default(),
                reactions_fetched: Default::default(),
                reactions_inflight: false,
                reaction_sec: None,
                floating: vec![],
                particles: vec![],
                last_posted_reaction: None,
                comments_next: None,
                comments_loading: false,
                comments_pages: 0,
                wave_context_track: None,
                profile: None,
                reposted_ids: Default::default(),
                liked_playlists: vec![],
                liked_playlist_ids: Default::default(),
                liked_system_urns: Default::default(),
                search_next: None,
                inspector_track: None,
                inspector_tab: InspectorTab::Comments,
                inspector_favoriters: vec![],
                inspector_reposters: vec![],
                inspector_comments: vec![],
                inspector_tasks_pending: 0,
                add_popover: None,
                speed_popup: None,
                speed_wheel_acc: 0.0,
                playlists_gen: 0,
                saves_in_flight: 0,
                library_collapsed: false,
                sidebar_create_mode: false,
                sidebar_create_title: String::new(),
                my_playlists: vec![],
                my_followings: {
                    let mut f = offline_store.my_followings.clone();
                    sort_followings(&mut f, settings.artist_sort);
                    f
                },
                delete_playlist_confirm: None,
                action_menu: None,
                menu_anchor: None,
                add_popover_anchor: MenuAnchor::PlayerBar,
                history: App::load_history(),
                shuffle: false,
                repeat: RepeatMode::Off,
                last_wheel_skip: None,
                toast: None,
                is_muted: start_muted,
                prev_volume: if start_muted { 0.5 } else { init_vol.max(0.01) },
                show_user_menu: false,
                playback_speed: 1.0,
                stories: App::load_stories(),
                stories_read: App::load_stories_read(),
                story_receipts_pending: Default::default(),
                image_viewer: None,
                art_colors: std::collections::HashMap::new(),
                stories_from_stream: false,
                stories_expanded: false,
                active_story_index: None,
                downloaded_track_ids: App::scan_downloaded_track_ids(),
                downloaded_playlist_ids: App::load_downloaded_playlists(),
                offline_downloading: None,
                offline_batch_cancel: None,
                offline_single: Default::default(),
                offline_single_cancel: Default::default(),
                offline_store,
                offline_download_progress: (0, 0),
                offline_plan: Vec::new(),
                offline_attempted: Default::default(),
                offline_failed: 0,
                settings,
                anim_start: std::time::Instant::now(),
                window_id: None,
                window_scale: 1.0,
                window_size: app_size,
                pb_hover: None,
                row_artist_hover: None,
                hovered_track_row: None,
                expanded_description_track: None,
                wave_color_t: 1.0,
                vol_color_t: 1.0,
                visual_urls: Default::default(),
                wave_visual_src: None,
                wave_visual: None,
                wave_visual_baking: false,
                track_speeds: App::load_track_speeds(),
                track_speeds_dirty: false,
                volume_dirty: false,
                settings_search: String::new(),
                update_release: None,
                update_checking: false,
                update_check_manual: false,
                update_installing: false,
                remind_after_new_version: true,
                storage: None,
                library_mixes: Vec::new(),
                library_radios: Vec::new(),
                library_filter: None,
                artist_lookup: None,
                artist_lookup_gen: 0,
                list_windows: Default::default(),
            };
            let mut a = app;
            // Last known library and sidebar: shown at once, replaced when
            // the network answers, and all there is in offline mode.
            a.library = App::load_library_cache();
            a.liked_ids = a.library.iter().map(|t| t.id).collect();
            a.my_playlists = a.offline_store.my_playlists.clone();
            a.liked_playlists = a.offline_store.liked_playlists.clone();
            a.library_mixes = a.offline_store.mixes.clone();
            a.library_radios = a.offline_store.radios.clone();
            let keep = a.cache_keep();
            let cache_tracks = a.settings.cache_tracks;
            std::thread::spawn(move || trim_cache(&keep, cache_tracks));
            let update_task = if a.settings.check_updates {
                Task::perform(crate::updater::check_latest(), |result| {
                    Message::UpdatesChecked(result.map_err(|e| e.to_string()), false)
                })
            } else {
                Task::none()
            };
            let startup = if a.settings.offline_mode {
                a.go_offline();
                iced::window::get_latest().map(Message::InitWindowId)
            } else {
                Task::batch(vec![
                    iced::window::get_latest().map(Message::InitWindowId),
                    a.go_online(),
                ])
            };
            let startup = Task::batch([startup, update_task]);
            a.player = crate::player::spawn().ok();
            if let Some(p) = &a.player {
                p.send(PlayerCommand::SetVolume(init_vol));
            }
            a.apply_audio_prefs();
            if let Some(saved) = resume_state {
                a.pending_seek_ms = Some(saved.pos_ms);
                a.pending_seek_track = Some(saved.track.id);
                let (artist, title) = saved
                    .track
                    .display_artist_and_title(a.settings.prefer_artist_from_name);
                a.queue_pos = a.set_queue(vec![saved.track.clone()], 0);
                a.playing_id = Some(saved.track.id);
                a.playing_title = title;
                a.playing_artist = artist;
                a.pos_ms = saved.pos_ms;
                a.dur_ms = a.track_ms(&saved.track).unwrap_or(0);
                a.is_paused = true;
                a.at_end = true;
                // Keep the saved position selected without starting audio;
                // PlayerToggle consumes pending_seek_ms on the first play.
                a.apply_track_speed(saved.track.id);
                let visual = a.load_wave_visual(&saved.track);
                return (a, Task::batch([startup, visual]));
            }
            (a, startup)
        });
}

// ---------- background workers (own clients) ----------

fn worker_env() -> Settings {
    Settings::load()
}

fn shared_bridge() -> std::sync::Arc<wavify::sc_web::ScBridge> {
    static BRIDGE: std::sync::OnceLock<std::sync::Arc<wavify::sc_web::ScBridge>> =
        std::sync::OnceLock::new();
    BRIDGE.get_or_init(wavify::sc_web::ScBridge::new).clone()
}

async fn make_api() -> Result<(Api, String, Auth)> {
    let settings = worker_env();
    let auth = Auth::new(&settings)?;
    let access = auth.access_opt().await;
    let api = Api::new(auth.http().clone(), &settings)
        .with_access(access)
        .with_bridge(Some(shared_bridge()));
    let cid = settings
        .client_id_override
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string());
    Ok((api, cid, auth))
}

async fn init_api() -> Result<ApiStateClone> {
    let settings = worker_env();
    let auth = Auth::new(&settings)?;
    let access = auth.access_opt().await;
    let authenticated = access.is_some();
    let api = Api::new(auth.http().clone(), &settings)
        .with_access(access.clone())
        .with_bridge(Some(shared_bridge()));
    let (me, can_write) = match access {
        Some(tok) => (api.me(&tok).await.ok(), authenticated),
        None => (None, false),
    };
    Ok(ApiStateClone {
        client_id: settings
            .client_id_override
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| CLIENT_ID.to_string()),
        me,
        authenticated,
        can_write,
    })
}

fn item_track_ids(item: &serde_json::Value) -> Vec<i64> {
    let mut ids = Vec::new();
    if let Some(track) = item.get("track") {
        if let Some(id) = track.get("id").and_then(|x| x.as_i64()) {
            ids.push(id);
        }
    }
    if let Some(tracks) = item.get("tracks").and_then(|x| x.as_array()) {
        ids.extend(
            tracks
                .iter()
                .filter_map(|track| track.get("id").and_then(|x| x.as_i64())),
        );
    }
    if item.get("kind").and_then(|x| x.as_str()) == Some("track") {
        if let Some(id) = item.get("id").and_then(|x| x.as_i64()) {
            ids.push(id);
        }
    }
    ids.retain(|id| *id != 0);
    ids
}

fn group_items<'a>(group: &'a serde_json::Value) -> Option<&'a Vec<serde_json::Value>> {
    group
        .get("items")
        .and_then(|i| i.get("collection"))
        .and_then(|c| c.as_array())
        .or_else(|| group.get("collection").and_then(|c| c.as_array()))
}

async fn fetch_playlist_detail(id_or_urn: String, cid: String) -> Result<PlaylistDetail> {
    let (api, _, _) = make_api().await?;
    let mut p = api.fetch_playlist_any(&id_or_urn, &cid).await?;
    let tracks = p.tracks.take().unwrap_or_default();
    let author = p
        .user
        .as_ref()
        .map(|u| clean_username(&u.username).to_string())
        .unwrap_or_else(|| "SoundCloud".into());
    let author_avatar = p.user.as_ref().and_then(|u| u.avatar_url.clone());
    let author_id = p.user.as_ref().map(|u| u.id).filter(|id| *id != 0);
    let permalink_url = p.permalink_url.clone();
    let artwork_url = p
        .artwork()
        .map(str::to_string)
        .or_else(|| tracks.first().and_then(|t| t.artwork_url.clone()));
    Ok(PlaylistDetail {
        id: if is_system_playlist(&id_or_urn) {
            playlist_id_for(&id_or_urn)
        } else {
            p.id
        },
        id_or_urn,
        title: if p.title.is_empty() {
            "Playlist".into()
        } else {
            p.title
        },
        description: p.description,
        artwork_url,
        author,
        author_avatar,
        author_id,
        permalink_url,
        track_count: p.track_count.map(|c| c as usize).unwrap_or(tracks.len()),
        tracks,
        is_album: p.is_album.unwrap_or(false),
    })
}

async fn fetch_home() -> Result<Vec<HomeSection>> {
    let (api, cid, _) = make_api().await?;
    let v = api
        .mixed_selections(&cid)
        .await
        .unwrap_or(serde_json::Value::Null);
    let mut sections = vec![];
    let Some(groups) = v.get("collection").and_then(|c| c.as_array()) else {
        return Ok(sections);
    };

    for group in groups {
        let title = group
            .get("title")
            .or_else(|| group.get("short_title"))
            .and_then(|t| t.as_str())
            .unwrap_or("Home")
            .to_string();
        let urn = group
            .get("urn")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string();
        let Some(items) = group_items(group) else {
            continue;
        };

        let mut playlist_items = Vec::new();
        let mut track_items = Vec::new();

        for item in items {
            let kind = item.get("kind").and_then(|x| x.as_str()).unwrap_or("");
            if matches!(kind, "playlist" | "system-playlist") {
                let id_or_urn = item
                    .get("urn")
                    .and_then(|u| u.as_str())
                    .or_else(|| item.get("id").and_then(|id| id.as_str()))
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        item.get("id")
                            .and_then(|id| id.as_i64())
                            .map(|id| id.to_string())
                            .unwrap_or_default()
                    });
                let item_ids = item_track_ids(item);
                playlist_items.push((
                    id_or_urn,
                    item.get("title")
                        .or_else(|| item.get("short_title"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("Playlist")
                        .to_string(),
                    item.get("description")
                        .and_then(|x| x.as_str())
                        .unwrap_or(if kind == "system-playlist" {
                            "SoundCloud mix"
                        } else {
                            "Playlist"
                        })
                        .to_string(),
                    item.get("calculated_artwork_url")
                        .or_else(|| item.get("artwork_url"))
                        .and_then(|x| x.as_str())
                        .map(str::to_string),
                    item_ids,
                    // a user's playlist names its owner; a mix is SoundCloud's
                    (kind == "playlist")
                        .then(|| item.get("user"))
                        .flatten()
                        .and_then(|u| serde_json::from_value::<UserMini>(u.clone()).ok())
                        .filter(|u| u.id != 0),
                ));
            } else if kind == "track" {
                if let Some(id) = item.get("id").and_then(|x| x.as_i64()) {
                    if id != 0 {
                        track_items.push(id);
                    }
                }
            }
        }

        if !playlist_items.is_empty() {
            let mut ids = Vec::new();
            for (_, _, _, _, item_ids, _) in &playlist_items {
                for id in item_ids.iter().take(2) {
                    ids.push(*id);
                }
            }
            ids.sort_unstable();
            ids.dedup();
            let tracks = if !ids.is_empty() {
                api.tracks_by_ids(&ids, &cid).await.unwrap_or_default()
            } else {
                vec![]
            };
            let by_id: std::collections::HashMap<i64, Track> =
                tracks.into_iter().map(|track| (track.id, track)).collect();
            let playlists = playlist_items
                .into_iter()
                .take(10)
                .map(
                    |(id_or_urn, p_title, subtitle, artwork_url, item_ids, owner)| {
                        let preview_tracks = item_ids
                            .into_iter()
                            .filter_map(|id| by_id.get(&id).cloned())
                            .collect();
                        HomePlaylist {
                            id_or_urn,
                            title: p_title,
                            subtitle,
                            artwork_url,
                            tracks: preview_tracks,
                            owner,
                        }
                    },
                )
                .collect::<Vec<_>>();
            if !playlists.is_empty() {
                sections.push(HomeSection {
                    title,
                    urn,
                    shelf: HomeShelf::Playlists(playlists),
                });
            }
        } else if !track_items.is_empty() {
            let take: Vec<i64> = track_items.into_iter().take(20).collect();
            let tracks = api.tracks_by_ids(&take, &cid).await.unwrap_or_default();
            if !tracks.is_empty() {
                sections.push(HomeSection {
                    title,
                    urn,
                    shelf: HomeShelf::Tracks(tracks),
                });
            }
        }
    }

    let mut seen_titles = std::collections::HashSet::new();
    sections.retain(|sec| seen_titles.insert(sec.title.clone()));
    Ok(sections)
}

pub fn extract_tracks_artwork(tracks: &[Track]) -> Vec<String> {
    // a set, not a scan of the list: a library has thousands of covers
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    for track in tracks {
        let avatar = track
            .user
            .as_ref()
            .and_then(|usr| usr.avatar_url.as_deref());
        for u in [track.artwork_url.as_deref(), avatar].into_iter().flatten() {
            if !u.is_empty() && seen.insert(u) {
                urls.push(u.to_string());
            }
        }
    }
    urls
}

fn artwork_urls(
    home: &[HomeSection],
    current_playlist: Option<&PlaylistDetail>,
    search: &SearchResults,
    me: Option<&Me>,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut urls = Vec::new();
    let mut add = |url: Option<&str>| {
        if let Some(url) = url.filter(|url| !url.is_empty()) {
            if seen.insert(url.to_string()) {
                urls.push(url.to_string());
            }
        }
    };
    for section in home {
        match &section.shelf {
            HomeShelf::Playlists(playlists) => {
                for playlist in playlists {
                    add(playlist.artwork_or_avatar());
                    for track in &playlist.tracks {
                        add(track.artwork_or_avatar());
                    }
                }
            }
            HomeShelf::Tracks(tracks) => {
                for track in tracks {
                    add(track.artwork_or_avatar());
                }
            }
        }
    }
    if let Some(detail) = current_playlist {
        add(detail
            .artwork_url
            .as_deref()
            .or(detail.author_avatar.as_deref()));
        for track in &detail.tracks {
            add(track.artwork_or_avatar());
        }
    }
    for track in &search.tracks {
        add(track.artwork_or_avatar());
    }
    for user in &search.users {
        add(user.avatar_url.as_deref());
    }
    for playlist in &search.playlists {
        add(playlist.artwork_or_avatar());
    }
    add(me.and_then(|me| me.avatar_url.as_deref()));
    urls
}

/// Shared HTTP client for artwork downloads, rebuilt only when the proxy
/// setting changes (building a reqwest client per batch costs a TLS setup).
fn art_http_client() -> Option<reqwest::Client> {
    static CLIENT: std::sync::Mutex<Option<(String, reqwest::Client)>> =
        std::sync::Mutex::new(None);

    let settings = crate::config::Settings::load();
    if settings.offline_mode {
        return None;
    }
    let proxy = settings.proxy.unwrap_or_default();
    let mut guard = CLIENT.lock().ok()?;
    if let Some((key, client)) = guard.as_ref() {
        if *key == proxy {
            return Some(client.clone());
        }
    }

    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8))
        .pool_idle_timeout(std::time::Duration::from_secs(60));

    if !proxy.is_empty() {
        if let Ok(p) = reqwest::Proxy::all(&proxy) {
            builder = builder.proxy(p);
        }
    }

    // Anycast edge IPs for d36lkcxq7qra7v.cloudfront.net (*.sndcdn.com) to bypass ISP DNS sinkhole (127.197.1.x)
    let cf_addrs: [std::net::SocketAddr; 4] = [
        "143.204.238.36:443".parse().unwrap(),
        "18.239.69.112:443".parse().unwrap(),
        "3.173.219.38:443".parse().unwrap(),
        "99.84.181.63:443".parse().unwrap(),
    ];
    builder = builder
        .resolve_to_addrs("i1.sndcdn.com", &cf_addrs)
        .resolve_to_addrs("i2.sndcdn.com", &cf_addrs)
        .resolve_to_addrs("i3.sndcdn.com", &cf_addrs)
        .resolve_to_addrs("i4.sndcdn.com", &cf_addrs);

    let client = builder.build().ok()?;
    *guard = Some((proxy, client.clone()));
    Some(client)
}

async fn download_art(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    download_art_keep(client, url, true).await
}

/// Download an image; `keep`: into the artwork cache too.
async fn download_art_keep(client: &reqwest::Client, url: &str, keep: bool) -> Option<Vec<u8>> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?.to_vec();
    if bytes.is_empty() {
        return None;
    }
    if keep {
        let path = crate::config::cached_artwork_path(url);
        let _ = tokio::fs::write(&path, &bytes).await;
    }
    Some(bytes)
}

/// Warm the disk cache for a set of artwork URLs (no decoding, no UI memory).
/// The demand-driven pipeline (`fetch_artwork_masked`) turns cached files into
/// displayable handles only for images actually rendered on screen.
/// Returns whether anything was really downloaded: the "network works"
/// signal (pure disk hits prove nothing).
async fn fetch_artwork(urls: Vec<String>) -> bool {
    let mut needed = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for url in urls {
        if !seen.insert(url.clone()) {
            continue;
        }
        let path = crate::config::cached_artwork_path(&url);
        if !matches!(tokio::fs::metadata(&path).await, Ok(m) if m.len() > 0) {
            needed.push(url);
        }
    }
    if needed.is_empty() {
        return false;
    }
    let Some(client) = art_http_client() else {
        return false;
    };

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    futures_util::future::join_all(needed.into_iter().map(|url| {
        let client = client.clone();
        let sem = sem.clone();
        async move {
            let _permit = sem.acquire().await.ok()?;
            download_art(&client, &url).await
        }
    }))
    .await
    .iter()
    .any(Option::is_some)
}

/// Artwork request: (source URL, circular mask?, display size in px).
type ArtKey = (String, bool, u16);
/// One processed artwork: request, width, height, RGBA, header tint.
type ProcessedArt = (ArtKey, u32, u32, Vec<u8>, Color);

/// Tiles are decoded at exactly the size they're drawn: the GPU samples the
/// texture without mipmaps, so minifying a 512px decode onto a 44px tile
/// aliases (grainy thumbnails) and upscaling a 100px one blurs.
fn art_px(size: f32) -> u16 {
    size.round().clamp(8.0, 1024.0) as u16
}

/// Spotify's image corners: --encore-corner-radius-base (4px) on list
/// thumbnails, a touch more on cards and headers.
fn art_radius(px: u16) -> f32 {
    if px <= 64 {
        4.0
    } else {
        6.0
    }
}

/// SoundCloud serves artwork and avatars in fixed sizes picked by a filename
/// suffix (`-large` is only 100px). Pick the smallest variant that covers the
/// tile; `None` when the URL isn't a recognised sndcdn size URL.
fn art_variant_url(url: &str, px: u16) -> Option<String> {
    const SIZES: [&str; 12] = [
        "large", "t500x500", "t300x300", "t200x200", "t120x120", "t67x67", "small", "badge",
        "tiny", "mini", "crop", "original",
    ];
    if !url.contains("sndcdn.com") {
        return None;
    }
    let dot = url.rfind('.')?;
    let (stem, ext) = url.split_at(dot);
    let dash = stem.rfind('-')?;
    if !SIZES.contains(&&stem[dash + 1..]) {
        return None;
    }
    let want = if px <= 100 {
        "large"
    } else if px <= 300 {
        "t300x300"
    } else {
        "t500x500"
    };
    let v = format!("{}-{}{}", &stem[..dash], want, ext);
    (v != url).then_some(v)
}

/// Average colour of an artwork, muted the way Spotify tints entity headers:
/// keep the hue, push saturation up a little and pull luminance into a band
/// that stays readable behind white text.
fn hero_color(rgba: &[u8], w: u32, h: u32) -> Color {
    let (mut r, mut g, mut b, mut n) = (0f32, 0f32, 0f32, 0f32);
    let step = (w.max(h) / 48).max(1) as usize;
    let (wu, hu) = (w as usize, h as usize);
    let mut y = 0usize;
    while y < hu {
        let mut x = 0usize;
        while x < wu {
            let i = (y * wu + x) * 4;
            if i + 3 < rgba.len() && rgba[i + 3] > 128 {
                r += rgba[i] as f32;
                g += rgba[i + 1] as f32;
                b += rgba[i + 2] as f32;
                n += 1.0;
            }
            x += step;
        }
        y += step;
    }
    if n == 0.0 {
        return Color::from_rgb(0.32, 0.32, 0.34);
    }
    let (mut r, mut g, mut b) = (r / n / 255.0, g / n / 255.0, b / n / 255.0);
    let mid = (r.max(g).max(b) + r.min(g).min(b)) * 0.5;
    for c in [&mut r, &mut g, &mut b] {
        *c = (mid + (*c - mid) * 1.4).clamp(0.0, 1.0);
    }
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let k = if lum > 0.001 {
        lum.clamp(0.14, 0.40) / lum
    } else {
        1.0
    };
    Color::from_rgb((r * k).min(1.0), (g * k).min(1.0), (b * k).min(1.0))
}

/// The header band follows the content island's rounded top corners.
fn hero_border() -> Border {
    Border {
        radius: border::Radius {
            top_left: 8.0,
            top_right: 8.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        },
        width: 0.0,
        color: Color::TRANSPARENT,
    }
}

/// A profile without a banner: SoundCloud's two-tone gradient, from the
/// avatar's colour to a darker, shifted one.
fn banner_colors(c: Color) -> (Color, Color) {
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let from = Color::from_rgb(
        mix(c.r, 1.0, 0.15),
        mix(c.g, 1.0, 0.15),
        mix(c.b, 1.0, 0.15),
    );
    // the channels turned round shift the hue
    let to = Color::from_rgb(
        mix(c.b, 0.0, 0.45),
        mix(c.r, 0.0, 0.45),
        mix(c.g, 0.0, 0.45),
    );
    (from, to)
}

/// Covers a picture's two top corners with the page's background, so it
/// takes the rounded corners of the panel it sits in.
struct TopCorners {
    radius: f32,
    color: Color,
}

impl canvas::Program<Message> for TopCorners {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let r = self.radius;
        let w = bounds.width;
        let left = canvas::Path::new(|b| {
            b.move_to(Point::new(0.0, r));
            b.arc_to(Point::ORIGIN, Point::new(r, 0.0), r);
            b.line_to(Point::ORIGIN);
            b.close();
        });
        let right = canvas::Path::new(|b| {
            b.move_to(Point::new(w - r, 0.0));
            b.arc_to(Point::new(w, 0.0), Point::new(w, r), r);
            b.line_to(Point::new(w, 0.0));
            b.close();
        });
        frame.fill(&left, self.color);
        frame.fill(&right, self.color);
        vec![frame.into_geometry()]
    }
}

/// Spotify's entity header: the artwork tint at the top fading into the page.
fn hero_gradient(c: Color) -> Background {
    Background::Gradient(iced::Gradient::Linear(
        iced::gradient::Linear::new(std::f32::consts::PI)
            .add_stop(0.0, c)
            .add_stop(1.0, BG_MUTED),
    ))
}

/// "1 hr 17 min" / "43 min", the way Spotify writes entity durations.
fn fmt_duration_long(ms: u64) -> String {
    let mins = ms / 60_000;
    if mins >= 60 {
        format!("{} hr {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    }
}

/// Spotify's primary entity action: a filled circle, --encore-control-size-larger.
fn hero_play_button(msg: Message, playing: bool) -> Element<'static, Message> {
    button(
        container(
            text(if playing { icons::PAUSE } else { icons::PLAY })
                .font(FA_SOLID)
                .size(18)
                .style(|_| t_color(Color::BLACK)),
        )
        .center_x(Length::Fixed(56.0))
        .center_y(Length::Fixed(56.0)),
    )
    .on_press(msg)
    .padding(0)
    .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => Color::from_rgb(1.0, 0.45, 0.1),
            _ => ORANGE,
        })),
        text_color: Color::BLACK,
        border: round(28.0),
        ..button::Style::default()
    })
    .into()
}

/// What an entity page's Download button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DlState {
    Idle,
    /// A collection run: tracks done / total.
    Progress(usize, usize),
    /// A single track on its way.
    Busy,
    Done,
}

/// Square hit box of the icon-only header buttons (Download, "...").
const HEADER_BTN: f32 = 40.0;

/// Spotify's download icon: a ring with a down arrow; while downloading the
/// ring fills clockwise in orange, and once done it is a solid green disc
/// with the arrow cut out.
struct DownloadGlyph {
    state: DlState,
}

impl canvas::Program<Message> for DownloadGlyph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // drawn on a 16-unit grid, like SaveGlyph
        let k = bounds.width.min(bounds.height) / 16.0;
        let p = |x: f32, y: f32| Point::new(x * k, y * k);
        let pen = |color: Color| {
            canvas::Stroke::default()
                .with_color(color)
                .with_width(1.5 * k)
                .with_line_cap(canvas::LineCap::Round)
                .with_line_join(canvas::LineJoin::Round)
        };
        let arrow = |frame: &mut canvas::Frame, color: Color| {
            frame.stroke(&canvas::Path::line(p(8.0, 4.25), p(8.0, 11.0)), pen(color));
            let head = canvas::Path::new(|b| {
                b.move_to(p(5.0, 8.25));
                b.line_to(p(8.0, 11.25));
                b.line_to(p(11.0, 8.25));
            });
            frame.stroke(&head, pen(color));
        };
        let ring = canvas::Path::circle(p(8.0, 8.0), 7.25 * k);
        match self.state {
            DlState::Idle => {
                frame.stroke(&ring, pen(TEXT_DIM));
                arrow(&mut frame, TEXT_DIM);
            }
            DlState::Progress(done, total) => {
                frame.stroke(
                    &ring,
                    pen(Color {
                        a: 0.25,
                        ..TEXT_DIM
                    }),
                );
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                if frac > 0.0 {
                    let start = -std::f32::consts::FRAC_PI_2;
                    let arc = canvas::Path::new(|b| {
                        b.arc(canvas::path::Arc {
                            center: p(8.0, 8.0),
                            radius: 7.25 * k,
                            start_angle: iced::Radians(start),
                            end_angle: iced::Radians(start + frac.min(1.0) * std::f32::consts::TAU),
                        });
                    });
                    frame.stroke(&arc, pen(ORANGE));
                }
                arrow(&mut frame, ORANGE);
            }
            DlState::Busy => {
                frame.stroke(
                    &ring,
                    pen(Color {
                        a: 0.25,
                        ..TEXT_DIM
                    }),
                );
                arrow(&mut frame, ORANGE);
            }
            DlState::Done => {
                frame.fill(&canvas::Path::circle(p(8.0, 8.0), 8.0 * k), GREEN_LIKE);
                arrow(&mut frame, Color::BLACK);
            }
        }
        vec![frame.into_geometry()]
    }
}

/// Icon-only Download button; its state is spelled out in a tooltip.
fn download_button(state: DlState, msg: Message) -> Element<'static, Message> {
    const GLYPH: f32 = 28.0;
    let label = match state {
        DlState::Idle => "Download".to_string(),
        DlState::Progress(done, total) => format!("Cancel download ({done}/{total})"),
        DlState::Busy => "Cancel download".to_string(),
        DlState::Done => "Downloaded".to_string(),
    };
    let is_downloading = matches!(state, DlState::Progress(_, _) | DlState::Busy);
    let glyph: Element<'static, Message> = if is_downloading {
        text(icons::XMARK)
            .font(FA_SOLID)
            .size(16)
            .wrapping(text::Wrapping::None)
            .into()
    } else {
        canvas(DownloadGlyph { state })
            .width(Length::Fixed(GLYPH))
            .height(Length::Fixed(GLYPH))
            .into()
    };
    let mut btn = button(
        container(glyph)
            .center_x(Length::Fixed(HEADER_BTN))
            .center_y(Length::Fixed(HEADER_BTN)),
    )
    .padding(0)
    .style(|_, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
            _ => None,
        },
        border: round(HEADER_BTN / 2.0),
        ..button::Style::default()
    });
    if matches!(
        state,
        DlState::Idle | DlState::Progress(_, _) | DlState::Busy
    ) {
        btn = btn.on_press(msg);
    }
    header_tooltip(btn, label)
}

/// Small dark label above a header button, like Spotify's hover hints.
fn header_tooltip<'a>(
    content: impl Into<Element<'a, Message>>,
    label: String,
) -> Element<'a, Message> {
    iced::widget::tooltip(
        content,
        container(
            text(label)
                .size(14)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
        )
        .padding(Padding::from([4, 8]))
        .style(|_| panel(BG_HOVER, 4.0)),
        iced::widget::tooltip::Position::Top,
    )
    .gap(6)
    .into()
}

/// Follow / Following pill of an artist or user page; one fixed width, so
/// toggling never moves the "..." button beside it.
fn follow_button(following: bool) -> Element<'static, Message> {
    const FOLLOW_W: f32 = 120.0;
    const FOLLOW_H: f32 = 32.0;
    button(
        container(
            text(if following { "Following" } else { "Follow" })
                .size(16)
                .wrapping(text::Wrapping::None),
        )
        .center_x(Length::Fixed(FOLLOW_W))
        .center_y(Length::Fixed(FOLLOW_H)),
    )
    .on_press(Message::FollowToggle)
    .padding(0)
    .clip(true)
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered);
        button::Style {
            background: None,
            text_color: if hovered { TEXT } else { TEXT_DIM },
            border: Border {
                radius: border::Radius::from(FOLLOW_H / 2.0),
                width: 1.0,
                color: if hovered { TEXT } else { TEXT_MUTED },
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// The action bar every entity page (track, playlist, artist, Liked Tracks)
/// shows on its header band: exactly three buttons — Play, then Download
/// (Follow on an artist page), then "...", which opens the side panel
/// holding every other action.
fn entity_actions<'a>(
    play: Message,
    playing: bool,
    middle: Element<'a, Message>,
    more: Message,
    with_menu: impl FnOnce(Element<'a, Message>) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let more_btn = button(
        container(
            text(icons::ELLIPSIS)
                .font(FA_SOLID)
                .size(22)
                .wrapping(text::Wrapping::None),
        )
        .center_x(Length::Fixed(HEADER_BTN))
        .center_y(Length::Fixed(HEADER_BTN)),
    )
    .on_press(more)
    .padding(0)
    .style(|_, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
            _ => None,
        },
        text_color: match status {
            button::Status::Hovered => TEXT,
            _ => TEXT_DIM,
        },
        border: round(HEADER_BTN / 2.0),
        ..button::Style::default()
    });
    // One fixed height (the 56px play circle); anything that can't fit a
    // very narrow window is cut, never wrapped.
    row![
        hero_play_button(play, playing),
        middle,
        with_menu(more_btn.into())
    ]
    .spacing(16)
    .height(Length::Fixed(56.0))
    .align_y(iced::Alignment::Center)
    .clip(true)
    .into()
}

/// Spotify shrinks the entity title as it gets longer (clamp(2rem,8vw,6rem)).
fn hero_title_size(title: &str) -> u16 {
    match title.chars().count() {
        0..=10 => 68,
        11..=18 => 54,
        19..=30 => 40,
        _ => 30,
    }
}

/// Allocation-free identity of an (url, shape) request for the touched-set.
fn art_key_hash(url: &str, circle: bool, px: u16) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    circle.hash(&mut h);
    px.hash(&mut h);
    h.finish()
}

/// SoundCloud artwork and avatar URLs end in a size suffix (`-large.jpg`,
/// `-t500x500.jpg`, …). Swap it for `-original` to get the full-resolution photo.
fn full_art_url(url: &str) -> String {
    let Some(dot) = url.rfind('.') else {
        return url.to_string();
    };
    let (stem, ext) = url.split_at(dot);
    match stem.rfind('-') {
        Some(dash) => format!("{}-original{}", &stem[..dash], ext),
        None => url.to_string(),
    }
}

/// Load an artwork at the highest resolution available, for the full-photo
/// viewer: no mask and no downscale, unlike the tile pipeline. Falls back to
/// the URL as given when the `-original` variant does not exist.
async fn fetch_full_image(url: String) -> Result<(String, u32, u32, Vec<u8>), String> {
    let client = art_http_client().ok_or_else(|| "no http client".to_string())?;
    let mut candidates = vec![full_art_url(&url)];
    if candidates[0] != url {
        candidates.push(url.clone());
    }
    for cand in candidates {
        let Some(bytes) = download_art(&client, &cand).await else {
            continue;
        };
        let decoded = tokio::task::spawn_blocking(move || {
            ::image::load_from_memory(&bytes).ok().map(|im| {
                let rgba = im.into_rgba8();
                (rgba.width(), rgba.height(), rgba.into_raw())
            })
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some((w, h, px)) = decoded {
            return Ok((url, w, h, px));
        }
    }
    Err("could not load image".into())
}

/// Decode -> centre-crop to square -> resample to the exact display size ->
/// bake the rounded/circular alpha mask into the pixels. iced containers do
/// not clip their children, so rounding has to live in the bitmap itself;
/// masking at display resolution gives every tile the same crisp corner.
fn process_art_bytes(bytes: &[u8], circle: bool, px: u16) -> Option<(u32, u32, Vec<u8>, Color)> {
    let decoded = ::image::load_from_memory(bytes).ok()?;
    let mut rgba = decoded.into_rgba8();

    let (w, h) = rgba.dimensions();
    let side = w.min(h);
    if side == 0 {
        return None;
    }
    if w != h {
        let x = (w - side) / 2;
        let y = (h - side) / 2;
        rgba = ::image::imageops::crop_imm(&rgba, x, y, side, side).to_image();
    }
    let tint = hero_color(rgba.as_raw(), side, side);

    let dim = px as u32;
    if side != dim {
        // Catmull-Rom widens its kernel when shrinking, so this is a proper
        // area filter (no aliasing) and a clean upscale for small sources.
        rgba =
            ::image::imageops::resize(&rgba, dim, dim, ::image::imageops::FilterType::CatmullRom);
    }

    let dimf = dim as f32;
    let radius = if circle {
        dimf / 2.0
    } else {
        art_radius(px).min(dimf / 2.0)
    };
    let center = dimf / 2.0;
    // Distance from the inner rectangle whose corners are the radius centres;
    // a one-pixel ramp antialiases the edge at 1:1 display scale.
    let inner = center - radius;
    for (x, y, p) in rgba.enumerate_pixels_mut() {
        let dx = ((x as f32 + 0.5) - center).abs() - inner;
        let dy = ((y as f32 + 0.5) - center).abs() - inner;
        if dx <= 0.0 && dy <= 0.0 {
            continue;
        }
        let dist = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
        if dist > radius - 1.0 {
            let a = (radius - dist).clamp(0.0, 1.0);
            p.0[3] = (p.0[3] as f32 * a) as u8;
        }
    }

    Some((dim, dim, rgba.into_raw(), tint))
}

/// A track's visual banner (SoundCloud serves the upload as `-original`),
/// from the artwork cache or the network, decoded and scaled to at most
/// 2560px wide: more than any waveform needs.
async fn fetch_wave_visual(url: String, keep: bool) -> Result<VisualPixels, String> {
    let path = crate::config::cached_artwork_path(&url);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => {
            let client = art_http_client().ok_or_else(|| "offline".to_string())?;
            download_art_keep(&client, &url, keep)
                .await
                .ok_or_else(|| "download failed".to_string())?
        }
    };
    let decoded = tokio::task::spawn_blocking(move || {
        let img = ::image::load_from_memory(&bytes).ok()?;
        let img = if img.width() > 2560 {
            img.resize(2560, u32::MAX, ::image::imageops::FilterType::Triangle)
        } else {
            img
        };
        Some(img.into_rgba8())
    })
    .await
    .map_err(|e| e.to_string())?;
    match decoded {
        Some(img) if img.width() > 0 && img.height() > 0 => {
            Ok(VisualPixels(std::sync::Arc::new(img)))
        }
        _ => {
            // not an image (an error page saved as one): fetch it anew next time
            let _ = tokio::fs::remove_file(&path).await;
            Err("not an image".into())
        }
    }
}

/// The banner for a `w` x `h` px waveform: the middle band of the picture,
/// scaled to cover the box, darkened to PB_WAVE_DIM so the bars stay
/// readable over it, with `radius` corners baked into the alpha (an iced
/// image can't be clipped round).
fn bake_visual(src: &::image::RgbaImage, w: u32, h: u32, radius: f32) -> Option<Vec<u8>> {
    let (sw, sh) = src.dimensions();
    if w == 0 || h == 0 || sw == 0 || sh == 0 {
        return None;
    }
    // the largest window of the box's aspect ratio, centred
    let aspect = w as f32 / h as f32;
    let (cw, ch) = if sw as f32 / sh as f32 > aspect {
        (((sh as f32 * aspect).round() as u32).clamp(1, sw), sh)
    } else {
        (sw, ((sw as f32 / aspect).round() as u32).clamp(1, sh))
    };
    let band = ::image::imageops::crop_imm(src, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
    let mut out = ::image::imageops::resize(&band, w, h, ::image::imageops::FilterType::CatmullRom);

    let (wf, hf) = (w as f32, h as f32);
    let r = radius.min(wf / 2.0).min(hf / 2.0);
    for (x, y, p) in out.enumerate_pixels_mut() {
        for c in &mut p.0[..3] {
            *c = (*c as f32 * PB_WAVE_DIM).round() as u8;
        }
        // distance into a corner's square, from its arc's centre
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let dx = (r - px).max(px - (wf - r));
        let dy = (r - py).max(py - (hf - r));
        if dx > 0.0 && dy > 0.0 {
            let a = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            p.0[3] = (p.0[3] as f32 * a).round() as u8;
        }
    }
    Some(out.into_raw())
}

/// Demand-driven artwork pipeline: for every request load the right-sized
/// SoundCloud variant (disk cache, then network), falling back to the URL as
/// given, and return masked RGBA ready for `Handle::from_rgba`.
/// Returns (processed, all_requested).
async fn fetch_artwork_masked(reqs: Vec<ArtKey>) -> (Vec<ProcessedArt>, Vec<ArtKey>) {
    if reqs.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let requested = reqs.clone();
    let client = art_http_client();

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let processed: Vec<ProcessedArt> =
        futures_util::future::join_all(reqs.into_iter().map(|(url, circle, px)| {
            let client = client.clone();
            let sem = sem.clone();
            async move {
                let _permit = sem.acquire().await.ok()?;
                let mut candidates = Vec::with_capacity(2);
                if let Some(v) = art_variant_url(&url, px) {
                    candidates.push(v);
                }
                candidates.push(url.clone());

                for cand in candidates {
                    let path = crate::config::cached_artwork_path(&cand);
                    if let Ok(bytes) = tokio::fs::read(&path).await {
                        if !bytes.is_empty() {
                            let r = tokio::task::spawn_blocking(move || {
                                process_art_bytes(&bytes, circle, px)
                            })
                            .await
                            .ok()
                            .flatten();
                            if let Some((w, h, rgba, tint)) = r {
                                return Some(((url, circle, px), w, h, rgba, tint));
                            }
                            // Corrupt cache entry (e.g. an error page saved as
                            // an image): drop it and try the network.
                            let _ = tokio::fs::remove_file(&path).await;
                        }
                    }
                    if let Some(c) = client.as_ref() {
                        if let Some(bytes) = download_art(c, &cand).await {
                            let r = tokio::task::spawn_blocking(move || {
                                process_art_bytes(&bytes, circle, px)
                            })
                            .await
                            .ok()
                            .flatten();
                            if let Some((w, h, rgba, tint)) = r {
                                return Some(((url, circle, px), w, h, rgba, tint));
                            }
                        }
                    }
                }
                None
            }
        }))
        .await
        .into_iter()
        .flatten()
        .collect();

    (processed, requested)
}

/// Cookie-paste login that leaves the saved cookie jar as it was when the
/// paste doesn't authorize: authorize_from_paste stores the paste as the jar
/// before trying it, so a wrong paste would replace a working jar.
fn authorize_paste_keeping_jar(raw: &str) -> Result<()> {
    let path = crate::config::cookies_path();
    let prev = std::fs::read(&path).ok();
    let res = crate::cookie_auth::authorize_from_paste(raw);
    if res.is_err() {
        let _ = match prev {
            Some(bytes) => std::fs::write(&path, bytes),
            None => std::fs::remove_file(&path),
        };
    }
    res
}

/// Spawn `wavify.exe --login` child (embedded WebView2 window) and wait for
/// its result: token.json on success, auth_error.json on failure.
/// Put back the token set aside by a sign-in that didn't finish.
fn restore_token_backup() {
    let tok = crate::config::token_path();
    let bak = tok.with_extension("json.bak");
    if bak.exists() && !tok.exists() {
        let _ = std::fs::rename(bak, tok);
    }
}

async fn spawn_login_child() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let err_path = crate::config::auth_error_path();
    let tok_path = crate::config::token_path();

    // clean previous state
    let _ = std::fs::remove_file(&err_path);
    let _ = std::fs::remove_file(&tok_path);

    let _ = std::process::Command::new(&exe)
        .arg("--login")
        .spawn()
        .map_err(|e| format!("failed to spawn login window: {e}"))?;

    // wait up to 10 minutes (user may be solving a captcha)
    for _ in 0..600 {
        if tok_path.exists() {
            return Ok(());
        }
        if err_path.exists() {
            let msg = std::fs::read_to_string(&err_path).unwrap_or_default();
            return Err(format!("login window: {msg}"));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err("login timed out (10 min)".into())
}

async fn fetch_search(q: String, cid: String) -> Result<(SearchResults, Option<String>)> {
    let (api, _, _) = make_api().await?;
    let q_trimmed = q.trim();
    if q_trimmed.contains("soundcloud.com/") {
        if let Ok(resolved) = api.resolve(q_trimmed, &cid).await {
            match resolved {
                crate::api::ResolvedEntity::Track(track) => {
                    return Ok((
                        SearchResults {
                            tracks: vec![track],
                            users: Vec::new(),
                            playlists: Vec::new(),
                        },
                        None,
                    ));
                }
                crate::api::ResolvedEntity::User(user) => {
                    return Ok((
                        SearchResults {
                            tracks: Vec::new(),
                            users: vec![user],
                            playlists: Vec::new(),
                        },
                        None,
                    ));
                }
                crate::api::ResolvedEntity::Playlist(playlist) => {
                    return Ok((
                        SearchResults {
                            tracks: Vec::new(),
                            users: Vec::new(),
                            playlists: vec![playlist],
                        },
                        None,
                    ));
                }
                crate::api::ResolvedEntity::Unknown(_) => {}
            }
        }
    }
    // "#Hip-Hop & Rap": tracks with that genre or tag, as soundcloud.com's
    // tag pages list them
    if let Some(tag) = q.strip_prefix('#').map(str::trim).filter(|t| !t.is_empty()) {
        let t = api.search_tracks_tagged(tag, &cid, 30).await?;
        return Ok((
            SearchResults {
                tracks: t.collection,
                users: Vec::new(),
                playlists: Vec::new(),
            },
            t.next_href,
        ));
    }
    let (t, u, p) = tokio::try_join!(
        api.search_tracks(&q, &cid, 30),
        api.search_users(&q, &cid, 10),
        api.search_playlists(&q, &cid, 10),
    )?;
    Ok((
        SearchResults {
            tracks: t.collection,
            users: u.collection,
            playlists: p.collection,
        },
        t.next_href,
    ))
}

/// The profile of an artist known only by name ("Artist - Title" read from a
/// track's name): a user SoundCloud finds for it whose username or permalink
/// is that name. Credits of several artists ("A x B", "A & B", "A feat. B")
/// try the whole credit, then each artist in turn.
async fn lookup_artist_profile(name: String) -> Result<Option<i64>> {
    let (api, cid, _) = make_api().await?;
    let full = clean_username(&name).to_string();
    let mut names = vec![full.clone()];
    for part in split_artist_credit(&full) {
        if !names.contains(&part) {
            names.push(part);
        }
    }
    for n in names.iter().take(4) {
        let users = api.search_users(n, &cid, 20).await?;
        let key = n.to_lowercase();
        let slug: String = key.chars().filter(|c| c.is_alphanumeric()).collect();
        let found = users.collection.into_iter().find(|u| {
            clean_username(&u.username).to_lowercase() == key
                || u.permalink.as_deref().is_some_and(|p| {
                    let p = p.to_lowercase();
                    p == key
                        || p.chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                            == slug
                })
        });
        if let Some(u) = found {
            return Ok(Some(u.id));
        }
    }
    Ok(None)
}

/// "A x B & C feat. D" -> ["A", "B", "C", "D"]; a single artist -> [it].
fn split_artist_credit(credit: &str) -> Vec<String> {
    const SEPS: [&str; 16] = [
        " x ", " X ", " & ", " + ", ", ", " w/ ", " W/ ", " feat. ", " Feat. ", " FEAT. ", " ft. ",
        " Ft. ", " FT. ", " feat ", " ft ", " vs. ",
    ];
    let mut parts = vec![credit.to_string()];
    for sep in SEPS {
        parts = parts
            .iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    parts
        .into_iter()
        .map(|p| {
            p.trim()
                .trim_matches(|c| c == '(' || c == ')')
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

async fn fetch_library() -> Result<Vec<Track>> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    crate::log!("library: got access token, querying me…");
    let me = api.me(&access).await?;
    crate::log!("library: me.id = {}", me.id);
    // paginate through ALL likes (200 per page, follows next_href)
    api.all_user_likes(me.id, &cid, 200).await
}

pub fn extract_stories_from_home(sections: &[HomeSection]) -> Vec<StoryItem> {
    let mut stories = Vec::new();
    let mut seen_users = std::collections::HashSet::new();

    for sec in sections {
        match &sec.shelf {
            HomeShelf::Tracks(tracks) => {
                for tr in tracks {
                    if let Some(user) = &tr.user {
                        if seen_users.insert(user.id) {
                            stories.push(StoryItem {
                                user_id: user.id,
                                username: user.username.clone(),
                                avatar_url: user.avatar_url.clone(),
                                track_id: tr.id,
                                track_title: tr.title.clone(),
                                artwork_url: tr.artwork_url.clone(),
                                duration_ms: tr.duration.unwrap_or(0),
                                created_at_ms: crate::config::now_ms(),
                                reposted: false,
                                server_read: false,
                                track: tr.clone(),
                            });
                            if stories.len() >= 6 {
                                return stories;
                            }
                        }
                    }
                }
            }
            HomeShelf::Playlists(playlists) => {
                for pl in playlists {
                    for tr in &pl.tracks {
                        if let Some(user) = &tr.user {
                            if seen_users.insert(user.id) {
                                stories.push(StoryItem {
                                    user_id: user.id,
                                    username: user.username.clone(),
                                    avatar_url: user.avatar_url.clone(),
                                    track_id: tr.id,
                                    track_title: tr.title.clone(),
                                    artwork_url: tr.artwork_url.clone(),
                                    duration_ms: tr.duration.unwrap_or(0),
                                    created_at_ms: crate::config::now_ms(),
                                    reposted: false,
                                    server_read: false,
                                    track: tr.clone(),
                                });
                                if stories.len() >= 6 {
                                    return stories;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    stories
}

async fn resolve_stream_from_track(
    track: Track,
    quality: String,
) -> Result<(i64, crate::api::StreamSource)> {
    let id = track.id;
    let (api, cid, _) = make_api().await?;
    let src = if track.media.is_some() {
        match api.resolve_stream_track(&track, &cid, &quality).await {
            Ok(s) => s,
            Err(e) => {
                crate::log!(
                    "resolve_stream_track error: {e}, falling back to fresh resolve_stream"
                );
                api.resolve_stream(id, &cid, &quality).await?
            }
        }
    } else {
        api.resolve_stream(id, &cid, &quality).await?
    };
    Ok((id, src))
}

async fn fetch_wave(track_id: i64) -> Result<(i64, Vec<u8>)> {
    let (api, cid, _) = make_api().await?;
    let samples = api.waveform(track_id, &cid).await?;
    Ok((track_id, samples))
}

/// A cached comments page-set: one JSON per track holding every fetched page
/// plus the next_href so a later run can resume or refresh.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct CommentsCache {
    /// (ts_ms, author, body) — everything needed for wave dots + reactions.
    items: Vec<(u64, String, String)>,
    /// ms since epoch when this cache was written.
    saved_at: u64,
}

fn load_comments_cache(track_id: i64) -> Option<CommentsCache> {
    let path = crate::config::cached_comments_path(track_id);
    let raw = std::fs::read(&path).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn save_comments_cache(track_id: i64, cache: CommentsCache) {
    save_in_background(crate::config::cached_comments_path(track_id), move || {
        serde_json::to_vec(&cache).ok()
    });
}

/// Waveform comments are paged 200 at a time; this many pages at most
/// (2000 comments: the waveform can't show more apart anyway).
const COMMENTS_MAX_PAGES: u32 = 10;
/// A cached comment set older than this is fetched anew when online.
const COMMENTS_TTL_MS: u64 = 24 * 60 * 60 * 1000;

async fn fetch_comments(track_id: i64, offline: bool) -> Result<CommentsPage> {
    // Cache-first: a repeat play loads at once instead of re-fetching every
    // page. A day-old set is fetched anew, falling back to it if that fails.
    let cache = load_comments_cache(track_id);
    let fresh = cache
        .as_ref()
        .is_some_and(|c| crate::config::now_ms().saturating_sub(c.saved_at) < COMMENTS_TTL_MS);
    let from_cache = |c: CommentsCache| {
        crate::log!(
            "comments cache hit: track {track_id}, {} items",
            c.items.len()
        );
        CommentsPage {
            track_id,
            comments: c
                .items
                .into_iter()
                .map(|(ts_ms, author, body)| Comment {
                    timestamp: Some(ts_ms as i64),
                    user: Some(UserMini {
                        id: 0,
                        username: author,
                        ..Default::default()
                    }),
                    body,
                    ..Default::default()
                })
                .collect(),
            next: None,
            cached: true,
        }
    };
    if fresh || offline {
        if let Some(c) = cache {
            return Ok(from_cache(c));
        }
    }
    let fetched = async {
        let (api, cid, _) = make_api().await?;
        api.comments(track_id, &cid, 200).await
    }
    .await;
    match (fetched, cache) {
        (Ok((comments, next)), _) => Ok(CommentsPage {
            track_id,
            comments,
            next,
            cached: false,
        }),
        (Err(_), Some(stale)) => Ok(from_cache(stale)),
        (Err(e), None) => Err(e),
    }
}

async fn fetch_comments_next(next: String, track_id: i64) -> Result<CommentsPage> {
    let (api, _, _) = make_api().await?;
    let (comments, next) = api.comments_next(&next).await?;
    Ok(CommentsPage {
        track_id,
        comments,
        next,
        cached: false,
    })
}

async fn fetch_related(track_id: i64) -> Result<Vec<Track>> {
    let (api, cid, _) = make_api().await?;
    let list = api.related(track_id, &cid).await?;
    Ok(list.collection)
}

async fn fetch_track_detail(track_id: i64) -> Result<Track> {
    let (api, cid, _) = make_api().await?;
    api.track(track_id, &cid).await
}

async fn fetch_track_related_pair(track_id: i64) -> Result<(i64, Vec<Track>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.related(track_id, &cid).await?;
    Ok((track_id, list.collection))
}

async fn fetch_profile(user_id: i64) -> Result<ProfileDetail> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access_opt().await;
    let u = api.user_full(user_id, &cid).await?;
    let mut top = api.user_top_tracks(user_id, &cid).await.unwrap_or_default();
    if top.is_empty() {
        // fall back to all posted tracks
        top = api
            .user_all_tracks(user_id, &cid, 20)
            .await
            .unwrap_or_default();
    }
    let related = api
        .user_related_artists(user_id, &cid)
        .await
        .unwrap_or_default();
    let reposts = api.user_reposts(user_id, &cid).await.unwrap_or_default();
    // profiles show the first page only (the Overview caps it anyway)
    let playlists = api
        .user_playlists_posted(user_id, &cid, 1)
        .await
        .unwrap_or_default();
    let web_profiles = api
        .user_web_profiles(user_id, &cid)
        .await
        .unwrap_or_default();
    let followers = u
        .get("followers_count")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let followings = u
        .get("followings_count")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let track_count = u.get("track_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let likes_count = u.get("likes_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let playlist_count = u
        .get("playlist_count")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let username = u
        .get("username")
        .and_then(|x| x.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let avatar_url = u
        .get("avatar_url")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let permalink_url = u
        .get("permalink_url")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let banner_url = u
        .get("visuals")
        .filter(|v| v.get("enabled").and_then(|e| e.as_bool()) != Some(false))
        .and_then(|v| v.pointer("/visuals/0/visual_url"))
        .and_then(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .map(str::to_string);
    let field = |key: &str| {
        u.get(key)
            .and_then(|x| x.as_str())
            .map(|x| x.trim().to_string())
            .unwrap_or_default()
    };
    let full_name = field("full_name");
    let location = match (field("city"), field("country_code")) {
        (city, cc) if !city.is_empty() && !cc.is_empty() => format!("{city}, {cc}"),
        (city, cc) => format!("{city}{cc}"),
    };
    let me = match &access {
        Some(tok) => api.me(tok).await.ok(),
        None => None,
    };
    let me_id = me.map(|m| m.id).unwrap_or(0);
    let following = match (&access, me_id) {
        (Some(tok), id) if id != 0 => api.is_following(tok, id, user_id).await,
        _ => false,
    };
    Ok(ProfileDetail {
        id: user_id,
        username,
        avatar_url,
        permalink_url,
        followers,
        followings,
        track_count,
        likes_count,
        playlist_count,
        following,
        top_tracks: top,
        all_tracks: vec![],
        reposts,
        playlists,
        likes: vec![],
        followers_list: vec![],
        followings_list: vec![],
        related,
        web_profiles,
        active_tab: ProfileSubTab::Overview,
        banner_url,
        full_name,
        location,
    })
}

/// A profile's banner, ready to draw: decoded (from the artwork cache or
/// the network) and cut down to 1920px wide, which covers the header on
/// any screen without the GPU having to shrink a huge photo.
async fn fetch_banner(url: String) -> Result<image::Handle, String> {
    let VisualPixels(img) = fetch_wave_visual(url, true).await?;
    tokio::task::spawn_blocking(move || {
        let (w, h) = img.dimensions();
        let img = if w > 1920 {
            let nh = ((u64::from(h) * 1920) / u64::from(w)).max(1) as u32;
            ::image::imageops::resize(&*img, 1920, nh, ::image::imageops::FilterType::Triangle)
        } else {
            (*img).clone()
        };
        let (w, h) = img.dimensions();
        image::Handle::from_rgba(w, h, img.into_raw())
    })
    .await
    .map_err(|e| e.to_string())
}

async fn fetch_profile_followers(user_id: i64) -> Result<(i64, Vec<UserMini>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.user_followers(user_id, &cid, 50).await?;
    Ok((user_id, list))
}

async fn fetch_profile_followings(user_id: i64) -> Result<(i64, Vec<UserMini>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.user_followings(user_id, &cid, 50).await?;
    Ok((user_id, list))
}

async fn fetch_profile_likes(user_id: i64) -> Result<(i64, Vec<Track>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.user_liked_tracks(user_id, &cid, 50).await?;
    Ok((user_id, list))
}

async fn fetch_profile_tracks(user_id: i64) -> Result<(i64, Vec<Track>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.user_all_tracks(user_id, &cid, 50).await?;
    Ok((user_id, list))
}

async fn fetch_track_favoriters(track_id: i64) -> Result<(i64, Vec<UserMini>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.track_favoriters(track_id, &cid, 50).await?;
    Ok((track_id, list))
}

async fn fetch_track_reposters(track_id: i64) -> Result<(i64, Vec<UserMini>)> {
    let (api, cid, _) = make_api().await?;
    let list = api.track_reposters(track_id, &cid, 50).await?;
    Ok((track_id, list))
}

async fn download_track_to_disk(
    track: Track,
    cid: String,
    prefer_artist_from_name: bool,
) -> Result<String, String> {
    let downloads_dir = directories::UserDirs::new()
        .and_then(|u| u.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let target_dir = downloads_dir.join("Wavify");
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let (artist, title) = track.display_artist_and_title(prefer_artist_from_name);
    let safe_artist = artist.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let safe_title = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let name = format!("{} - {}", safe_artist.trim(), safe_title.trim());

    let cached = crate::config::cached_audio_path(track.id);
    if cached.exists() && cached.metadata().map(|m| m.len() > 10000).unwrap_or(false) {
        // the cache holds whatever the source streamed: MP3, or AAC in MP4
        let mut head = [0u8; 8];
        let is_mp4 = std::fs::File::open(&cached)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head))
            .is_ok()
            && (&head[4..8] == b"ftyp" || &head[4..8] == b"styp");
        let ext = if is_mp4 { "m4a" } else { "mp3" };
        let filename = format!("{name}.{ext}");
        std::fs::copy(&cached, target_dir.join(&filename)).map_err(|e| e.to_string())?;
        return Ok(filename);
    }
    let filename = format!("{name}.mp3");
    let dest_path = target_dir.join(&filename);

    let (api, client_cid, _) = make_api().await.map_err(|e| e.to_string())?;
    let effective_cid = if cid.is_empty() { client_cid } else { cid };
    let src = if track.media.is_some() {
        api.resolve_stream_track(&track, &effective_cid, "mp3")
            .await
            .map_err(|e| e.to_string())?
    } else {
        api.resolve_stream(track.id, &effective_cid, "mp3")
            .await
            .map_err(|e| e.to_string())?
    };

    let http = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .build()
        .map_err(|e| e.to_string())?;

    let data = match src {
        crate::api::StreamSource::Single(url) => {
            let resp = http.get(&url).send().await.map_err(|e| e.to_string())?;
            resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
        }
        crate::api::StreamSource::Chunks { init, chunks } => {
            let mut all_bytes = Vec::new();
            if let Some(i_url) = init {
                if let Ok(resp) = http.get(&i_url).send().await {
                    if let Ok(b) = resp.bytes().await {
                        all_bytes.extend_from_slice(&b);
                    }
                }
            }
            for c_url in chunks {
                let resp = http.get(&c_url).send().await.map_err(|e| e.to_string())?;
                let b = resp.bytes().await.map_err(|e| e.to_string())?;
                all_bytes.extend_from_slice(&b);
            }
            all_bytes
        }
    };

    if data.len() < 5000 {
        return Err("Audio stream was too small or empty".into());
    }

    std::fs::write(&dest_path, &data).map_err(|e| e.to_string())?;
    let _ = std::fs::write(&cached, &data);

    Ok(filename)
}

/// Download a track's audio (plus waveform and artwork) into the offline
/// cache. `full`: a Go+ track with only its preview here, whose full
/// version is taken from another source when one has it; Ok(Some(source))
/// then says where it came from.
async fn wait_for_download_cancel(cancel: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    while !cancel.load(std::sync::atomic::Ordering::Acquire) {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
}

async fn cancellable_download<T, E: std::fmt::Display>(
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    future: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, String> {
    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Download canceled".into());
    }
    tokio::select! {
        result = future => result.map_err(|e| e.to_string()),
        _ = wait_for_download_cancel(cancel.clone()) => Err("Download canceled".into()),
    }
}

/// Download a track for offline: its audio (at the download quality from
/// Settings, "highest" or "lowest"), waveform and cover.
async fn cache_track_offline(
    track: Track,
    cid: String,
    quality: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Download canceled".into());
    }
    let cached = crate::config::cached_audio_path(track.id);
    if !(cached.exists() && cached.metadata().map(|m| m.len() > 10000).unwrap_or(false)) {
        let (api, client_cid, _) = cancellable_download(&cancel, make_api()).await?;
        let effective_cid = if cid.is_empty() {
            client_cid
        } else {
            cid.clone()
        };
        let src = if track.media.is_some() {
            match cancellable_download(
                &cancel,
                api.resolve_stream_track(&track, &effective_cid, &quality),
            )
            .await
            {
                Ok(s) => s,
                Err(e) if e == "Download canceled" => return Err(e),
                Err(_) => {
                    cancellable_download(
                        &cancel,
                        api.resolve_stream(track.id, &effective_cid, &quality),
                    )
                    .await?
                }
            }
        } else {
            cancellable_download(
                &cancel,
                api.resolve_stream(track.id, &effective_cid, &quality),
            )
            .await?
        };

        // the same route as playback (the proxy from Settings, if any)
        let http = crate::api::build_http(&worker_env()).map_err(|e| e.to_string())?;

        let data = match src {
            crate::api::StreamSource::Single(url) => {
                // one file for the whole track: the API client's 30 s limit
                // would fail every download slower than that
                let request = http
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(900))
                    .send();
                let resp = cancellable_download(&cancel, request).await?;
                cancellable_download(&cancel, resp.bytes()).await?.to_vec()
            }
            crate::api::StreamSource::Chunks { init, chunks } => {
                let mut all_bytes = Vec::new();
                if let Some(i_url) = init {
                    if let Ok(resp) = cancellable_download(&cancel, http.get(&i_url).send()).await {
                        if let Ok(b) = cancellable_download(&cancel, resp.bytes()).await {
                            all_bytes.extend_from_slice(&b);
                        }
                    }
                }
                for c_url in chunks {
                    let resp = cancellable_download(&cancel, http.get(&c_url).send()).await?;
                    let b = cancellable_download(&cancel, resp.bytes()).await?;
                    all_bytes.extend_from_slice(&b);
                }
                all_bytes
            }
        };

        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Download canceled".into());
        }
        if data.len() < 5000 {
            return Err("Audio stream was too small".into());
        }
        static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let tmp = cached.with_file_name(format!(
            "{}.{}.download.tmp",
            track.id,
            TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::write(&tmp, &data).map_err(|e| e.to_string())?;
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            let _ = std::fs::remove_file(&tmp);
            return Err("Download canceled".into());
        }
        if let Err(e) = std::fs::rename(&tmp, &cached) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
    }

    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Download canceled".into());
    }

    // Also cache waveform
    let wave_cached = crate::config::cached_waveform_path(track.id);
    if !wave_cached.exists() {
        if let Ok((api, client_cid, _)) = make_api().await {
            let effective_cid = if cid.is_empty() { client_cid } else { cid };
            if let Ok(samples) = api.waveform(track.id, &effective_cid).await {
                let _ = std::fs::write(&wave_cached, &samples);
            }
        }
    }

    // Also cache artwork if available, through the artwork client: the proxy
    // from Settings, and timeouts, so a stuck cover can't hold the download
    // (download_art never caches an error page as artwork)
    if let Some(art_url) = track.artwork_or_avatar() {
        let art_path = crate::config::cached_artwork_path(art_url);
        if !art_path.exists() {
            if let Some(client) = art_http_client() {
                let _ = download_art(&client, art_url).await;
            }
        }
    }

    Ok(())
}

async fn fetch_inspector_comments(track_id: i64) -> Result<(i64, Vec<Comment>)> {
    let (api, cid, _) = make_api().await?;
    let (list, _) = api.comments(track_id, &cid, 60).await?;
    Ok((track_id, list))
}

/// A write, and once more if SoundCloud's check was just passed for it
/// (see Api::try_check): the second try goes through the bridge first,
/// which the check now vouches for.
async fn after_check<T, F, Fut>(f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    match f().await {
        Err(e) if e.to_string().contains(crate::api::CHECK_PASSED) => {
            crate::log!("SoundCloud's check passed: sending the write again");
            f().await
        }
        r => r,
    }
}

async fn do_repost(track_id: i64, reposted: bool) -> Result<()> {
    after_check(|| do_repost_once(track_id, reposted)).await
}

async fn do_repost_once(track_id: i64, reposted: bool) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    if reposted {
        api.unrepost_track(&access, me.id, track_id).await
    } else {
        api.repost_track(&access, me.id, track_id).await
    }
}

async fn do_follow(user_id: i64, follow: bool) -> Result<()> {
    after_check(|| do_follow_once(user_id, follow)).await
}

async fn do_follow_once(user_id: i64, follow: bool) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    if follow {
        api.unfollow_user(&access, user_id).await
    } else {
        api.follow_user(&access, user_id).await
    }
}

async fn do_radio(track_id: i64) -> Result<Vec<Track>> {
    let (api, cid, _) = make_api().await?;
    api.station_for_track(track_id, &cid).await
}

async fn do_artist_radio(artist_id: i64) -> Result<Vec<Track>> {
    let (api, cid, _) = make_api().await?;
    api.station_for_artist(artist_id, &cid).await
}

async fn fetch_user_flags() -> Result<(
    std::collections::HashSet<i64>,
    std::collections::HashSet<i64>,
    std::collections::HashSet<i64>,
    std::collections::HashSet<String>,
)> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    let reposts = api.my_repost_ids(&access).await;
    let followings = api.my_following_ids(&access, me.id).await;
    let saved_system = api.my_liked_system_playlists(&access).await;
    // derive liked-playlist ids from the real liked list (the /ids form is 404)
    let liked_list = api
        .user_liked_playlists(me.id, &cid)
        .await
        .unwrap_or_default();
    let liked: std::collections::HashSet<i64> = liked_list
        .into_iter()
        .map(|p| p.id)
        .filter(|id| *id != 0)
        .collect();
    Ok((reposts, liked, followings, saved_system))
}

async fn do_system_playlist_like(urn: String, liked: bool) -> Result<()> {
    after_check(|| do_system_playlist_like_once(urn.clone(), liked)).await
}

async fn do_system_playlist_like_once(urn: String, liked: bool) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    api.like_system_playlist(&access, me.id, &urn, liked).await
}

async fn fetch_liked_playlists() -> Result<Vec<Playlist>> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    api.user_liked_playlists(me.id, &cid).await
}

/// Parse "2026-09-17T04:19:24Z"-style ISO timestamps into epoch milliseconds
/// (0 = unparseable). Days-from-civil algorithm, no chrono dependency.
fn parse_iso_ms(s: &str) -> u64 {
    if s.len() < 19 {
        return 0;
    }
    let num = |r: std::ops::Range<usize>| -> i64 {
        s.get(r).and_then(|x| x.parse::<i64>().ok()).unwrap_or(-1)
    };
    let (y, mo, d) = (num(0..4), num(5..7), num(8..10));
    let (h, mi, se) = (num(11..13), num(14..16), num(17..19));
    if y < 1970
        || !(1..=12).contains(&mo)
        || !(1..=31).contains(&d)
        || !(0..24).contains(&h)
        || !(0..60).contains(&mi)
        || !(0..62).contains(&se)
    {
        return 0;
    }
    let y_adj = y - if mo <= 2 { 1 } else { 0 };
    let era = (if y_adj >= 0 { y_adj } else { y_adj - 399 }) / 400;
    let yoe = y_adj - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    ((days * 86400 + h * 3600 + mi * 60 + se).max(0) as u64) * 1000
}

/// Format an epoch-ms timestamp the way SoundCloud read receipts expect:
/// "yyyy/MM/dd HH:mm:ss +0000" (UTC). Civil-from-days algorithm, no chrono.
fn fmt_receipt_ts(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02} +0000",
        y,
        m,
        d,
        sod / 3600,
        (sod / 60) % 60,
        sod % 60
    )
}

/// Send the SoundCloud receipt and return failures so the UI can retry.
async fn mark_story_read(user_id: i64, created_ms: u64) -> Result<(), String> {
    let (api, _cid, auth) = make_api().await.map_err(|e| e.to_string())?;
    let access = auth.access().await.map_err(|e| e.to_string())?;
    let urn = format!("soundcloud:users:{user_id}");
    let ts = fmt_receipt_ts(if created_ms > 0 {
        created_ms
    } else {
        crate::config::now_ms()
    });
    api.artist_shortcut_mark_read(&access, &urn, &ts)
        .await
        .map_err(|e| e.to_string())
}

/// SoundCloud stories, the way the official client does it:
/// artist shortcuts first (server order + read state), stream-feed fallback.
async fn fetch_stories() -> Result<Vec<StoryItem>> {
    match fetch_shortcut_stories().await {
        Ok(list) if !list.is_empty() => {
            crate::log!("stories: {} from artist shortcuts", list.len());
            return Ok(list);
        }
        Ok(_) => crate::log!("stories: shortcuts empty, trying stream feed"),
        Err(e) => crate::log!("stories: shortcuts failed ({e}), trying stream feed"),
    }
    fetch_stream_stories().await
}

/// Primary source: GET /you/artist_shortcuts (strip of followed artists with
/// unread state), then each artist's latest track story, resolved to full
/// api-v2 tracks in a single tracks_by_ids batch.
async fn fetch_shortcut_stories() -> Result<Vec<StoryItem>> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    let v = api.artist_shortcuts(&access).await?;

    struct Shortcut {
        user_id: i64,
        urn: String,
        username: String,
        avatar_url: Option<String>,
        has_read: bool,
    }

    let mut shortcuts: Vec<Shortcut> = Vec::new();
    if let Some(items) = v.get("items").and_then(|c| c.as_array()) {
        for it in items.iter().take(12) {
            let user = it.get("user");
            let urn = it
                .get("user_urn")
                .and_then(|x| x.as_str())
                .or_else(|| user.and_then(|u| u.get("urn")).and_then(|x| x.as_str()))
                .unwrap_or("");
            let user_id: i64 = urn
                .rsplit(':')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if user_id == 0 {
                continue;
            }
            shortcuts.push(Shortcut {
                user_id,
                urn: urn.to_string(),
                username: user
                    .and_then(|u| u.get("username"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                avatar_url: user
                    .and_then(|u| u.get("avatar_url"))
                    .and_then(|x| x.as_str())
                    .map(str::to_string),
                has_read: it
                    .get("has_read")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            });
        }
    }
    if shortcuts.is_empty() {
        return Ok(Vec::new());
    }

    // Latest track story per artist, fetched concurrently.
    let story_lists = futures_util::future::join_all(
        shortcuts
            .iter()
            .map(|s| api.artist_shortcut_stories(&access, &s.urn)),
    )
    .await;

    let cutoff = crate::config::now_ms().saturating_sub(STORY_TTL_MS);
    struct Pending {
        shortcut_idx: usize,
        track_id: i64,
        created_ms: u64,
        reposted: bool,
    }
    let mut pending: Vec<Pending> = Vec::new();
    for (idx, res) in story_lists.into_iter().enumerate() {
        let Ok(sv) = res else { continue };
        let Some(list) = sv.get("stories").and_then(|s| s.as_array()) else {
            continue;
        };
        for story in list {
            let (post, reposted) = if let Some(p) = story.get("track_post").filter(|p| !p.is_null())
            {
                (p, false)
            } else if let Some(p) = story.get("track_repost").filter(|p| !p.is_null()) {
                (p, true)
            } else {
                continue; // playlist stories aren't playable here
            };
            let track_urn = post
                .get("track")
                .and_then(|t| t.get("urn"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let track_id: i64 = track_urn
                .rsplit(':')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if track_id == 0 {
                continue;
            }
            let created_ms = post
                .get("created_at")
                .and_then(|c| c.as_str())
                .map(parse_iso_ms)
                .unwrap_or(0);
            if created_ms != 0 && created_ms < cutoff {
                continue;
            }
            pending.push(Pending {
                shortcut_idx: idx,
                track_id,
                created_ms,
                reposted,
            });
        }
    }
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    // An artist can have several: theirs stay together, in the artists'
    // order, oldest first (Next goes on through them, then to the next
    // artist), at most 10 each.
    pending.sort_by_key(|p| (p.shortcut_idx, p.created_ms));
    let mut per_artist: std::collections::HashMap<usize, usize> = Default::default();
    pending.retain(|p| {
        let n = per_artist.entry(p.shortcut_idx).or_default();
        *n += 1;
        *n <= 10
    });

    let ids: Vec<i64> = pending.iter().map(|p| p.track_id).collect();
    let tracks = api.tracks_by_ids(&ids, &cid).await.unwrap_or_default();
    let by_id: std::collections::HashMap<i64, Track> =
        tracks.into_iter().map(|t| (t.id, t)).collect();

    let mut stories = Vec::new();
    for p in pending {
        let Some(track) = by_id.get(&p.track_id) else {
            continue;
        };
        let s = &shortcuts[p.shortcut_idx];
        stories.push(StoryItem {
            user_id: s.user_id,
            username: s.username.clone(),
            avatar_url: s.avatar_url.clone(),
            track_id: track.id,
            track_title: track.title.clone(),
            artwork_url: track.artwork_or_avatar().map(str::to_string),
            duration_ms: track.duration.unwrap_or(0),
            created_at_ms: p.created_ms,
            reposted: p.reposted,
            server_read: s.has_read,
            track: track.clone(),
        });
    }
    Ok(stories)
}

/// Fallback source: latest posts and reposts from the /stream follow feed —
/// up to 10 stories per artist (theirs together, oldest first), 12 artists,
/// 7-day expiry.
async fn fetch_stream_stories() -> Result<Vec<StoryItem>> {
    let (api, cid, _auth) = make_api().await?;
    let v = api.stream_feed(&cid, 60).await?;
    let cutoff = crate::config::now_ms().saturating_sub(STORY_TTL_MS);
    let mut seen_users = std::collections::HashSet::new();
    let mut stories = Vec::new();
    if let Some(items) = v.get("collection").and_then(|c| c.as_array()) {
        for item in items {
            let kind = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if kind != "track" && kind != "track-repost" {
                continue;
            }
            let Some(track) = item
                .get("track")
                .cloned()
                .and_then(|t| serde_json::from_value::<Track>(t).ok())
            else {
                continue;
            };
            if track.id == 0 {
                continue;
            }
            let created_ms = item
                .get("created_at")
                .and_then(|c| c.as_str())
                .map(parse_iso_ms)
                .unwrap_or(0);
            if created_ms != 0 && created_ms < cutoff {
                continue;
            }
            // The story belongs to the followed artist who posted/reposted it.
            let poster = item.get("user");
            let user_id = poster
                .and_then(|u| u.get("id"))
                .and_then(|x| x.as_i64())
                .unwrap_or(0);
            let username = poster
                .and_then(|u| u.get("username"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let avatar_url = poster
                .and_then(|u| u.get("avatar_url"))
                .and_then(|x| x.as_str())
                .map(str::to_string);
            if user_id == 0 {
                continue;
            }
            if seen_users.insert(user_id) && seen_users.len() > 12 {
                seen_users.remove(&user_id);
                continue;
            }
            if !seen_users.contains(&user_id)
                || stories
                    .iter()
                    .filter(|s: &&StoryItem| s.user_id == user_id)
                    .count()
                    >= 10
            {
                continue;
            }
            stories.push(StoryItem {
                user_id,
                username,
                avatar_url,
                track_id: track.id,
                track_title: track.title.clone(),
                artwork_url: track.artwork_url.clone(),
                duration_ms: track.duration.unwrap_or(0),
                created_at_ms: created_ms,
                reposted: kind == "track-repost",
                server_read: false,
                track,
            });
        }
    }
    // the feed is newest first: each artist's together, oldest first, the
    // artists in the order they last posted
    let order: Vec<i64> = {
        let mut seen = std::collections::HashSet::new();
        stories
            .iter()
            .filter(|s| seen.insert(s.user_id))
            .map(|s| s.user_id)
            .collect()
    };
    stories.sort_by_key(|s| {
        (
            order
                .iter()
                .position(|u| *u == s.user_id)
                .unwrap_or(usize::MAX),
            s.created_at_ms,
        )
    });
    Ok(stories)
}

async fn do_playlist_like(playlist_id: i64, liked: bool) -> Result<()> {
    after_check(|| do_playlist_like_once(playlist_id, liked)).await
}

async fn do_playlist_like_once(playlist_id: i64, liked: bool) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    if liked {
        api.unlike_playlist(&access, me.id, playlist_id).await
    } else {
        api.like_playlist(&access, me.id, playlist_id).await
    }
}

async fn fetch_my_playlists() -> Result<Vec<Playlist>> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    // every page: the save button and the picker need all own playlists
    api.user_playlists_posted(me.id, &cid, 20).await
}

async fn do_create_playlist(title: String, track_id: Option<i64>) -> Result<String> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    let pl = api
        .create_playlist(&access, &title, "public", track_id)
        .await?;
    Ok(format!("Created playlist '{}'", pl.title))
}

async fn fetch_my_followings() -> Result<Vec<UserMini>> {
    let (api, cid, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    api.user_followings(me.id, &cid, 200).await
}

async fn do_delete_playlist(playlist_id: i64) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    api.delete_playlist(&access, playlist_id).await
}

/// The speed popup's value field.
fn new_playlist_input_id() -> text_input::Id {
    text_input::Id::new("new-playlist-name")
}

fn speed_input_id() -> text_input::Id {
    text_input::Id::new("speed-input")
}

/// "1.5", "1,5", "0.25" -> speed; None for empty or malformed text.
fn parse_speed(s: &str) -> Option<f32> {
    s.trim()
        .replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Wheel delta in notches: one line per notch, ~40px of touchpad travel.
fn wheel_notches(d: mouse::ScrollDelta) -> f32 {
    match d {
        mouse::ScrollDelta::Lines { y, .. } => y,
        mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
    }
}

/// Whether a playlist's (possibly stub) track list holds the track.
fn playlist_has(p: &Playlist, track_id: i64) -> bool {
    p.tracks
        .as_ref()
        .is_some_and(|ts| ts.iter().any(|t| t.id == track_id))
}

/// Own playlist check; a playlist without owner info is assumed own (it came
/// from the user's own playlist list).
fn owned_by(p: &Playlist, me: Option<i64>) -> bool {
    match (me, p.user.as_ref()) {
        (Some(me), Some(u)) => u.id == me,
        _ => true,
    }
}

/// Spotify-style one-line summary of what a save changed.
fn save_summary(added: &[String], removed: &[String]) -> String {
    match (added.len(), removed.len()) {
        (0, 0) => "Saved.".to_string(),
        (1, 0) => format!("Added to {}.", added[0]),
        (0, 1) => format!("Removed from {}.", removed[0]),
        (n, 0) => format!("Added to {n} playlists."),
        (0, n) => format!("Removed from {n} playlists."),
        (a, r) => format!("Added to {a} and removed from {r} playlists."),
    }
}

/// Writes an "Add to playlist" commit: the Liked Tracks change first, then
/// playlist adds and removes, then the new playlist. Every step runs even if
/// an earlier one failed; the failures come back together.
async fn do_apply_saves(
    track_id: i64,
    like: Option<bool>,
    add: Vec<i64>,
    remove: Vec<i64>,
    create: Option<String>,
) -> SaveReport {
    let mut rep = SaveReport {
        track_id,
        like,
        like_ok: true,
        playlists_touched: !add.is_empty() || !remove.is_empty() || create.is_some(),
        errors: Vec::new(),
    };
    if let Some(want) = like {
        // do_like takes the current state: `true` means "unlike"
        if let Err(e) = do_like(track_id, !want).await {
            rep.like_ok = false;
            rep.errors.push(format!("Liked Tracks: {e}"));
        }
    }
    for pid in add {
        if let Err(e) = do_add_to_playlist(pid, track_id).await {
            rep.errors.push(e.to_string());
        }
    }
    for pid in remove {
        if let Err(e) = do_remove_from_playlist(pid, track_id).await {
            rep.errors.push(e.to_string());
        }
    }
    if let Some(title) = create {
        if let Err(e) = do_create_playlist(title, Some(track_id)).await {
            rep.errors.push(e.to_string());
        }
    }
    rep
}

/// Liked Tracks cover: the purple tile with a white heart (sidebar, popover).
/// One Your Library row: cover, then the title over a 12px meta line; the
/// whole row is the button (56px: a 44px cover and 6px around it).
fn library_row<'a>(
    cover: Element<'a, Message>,
    title: &str,
    downloaded: bool,
    meta: &str,
    msg: Message,
    compact: bool,
) -> Element<'a, Message> {
    // Every text is cut to its own pixel budget and kept on one line, so a
    // long name or owner can never push a row taller than the others.
    const TEXT_W: f32 = 162.0; // 228 row - 12 padding - 44 cover - 10 gap
    const COMPACT_W: f32 = 212.0; // 228 row - 16 padding
    const CHECK_W: f32 = 18.0; // 13px glyph + 5 gap
    let check = || -> Element<'a, Message> {
        text(icons::CIRCLE_DOWN)
            .font(FA_SOLID)
            .size(13)
            .wrapping(text::Wrapping::None)
            .style(|_| t_color(DOWNLOADED))
            .into()
    };
    let style = |_: &iced::Theme, status: button::Status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(Background::Color(BG_HOVER)),
            _ => None,
        },
        border: round(4.0),
        ..button::Style::default()
    };
    if compact {
        // Spotify's compact layout: no covers, the name and what it is on
        // one 32px line; the name keeps most of it
        let room = COMPACT_W - if downloaded { CHECK_W } else { 0.0 };
        let name = trunc_px(title, text_px(title, 14.0).min(room - 56.0), 14.0);
        let meta_w = room - text_px(&name, 14.0) - 6.0;
        let mut line = row![].spacing(6).align_y(iced::Alignment::Center);
        if downloaded {
            line = line.push(check());
        }
        line = line.push(
            text(name)
                .size(14)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
        );
        if meta_w > 24.0 {
            line = line.push(
                text(trunc_px(&format!("• {meta}"), meta_w, 12.0))
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            );
        }
        return button(
            container(line)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(msg)
        .padding(pad4(0.0, 8.0, 0.0, 8.0))
        .width(Length::Fill)
        .height(Length::Fixed(32.0))
        .clip(true)
        .style(style)
        .into();
    }
    let mut title_line = row![].spacing(5).align_y(iced::Alignment::Center);
    if downloaded {
        title_line = title_line.push(check());
    }
    title_line = title_line.push(
        text(trunc_px(
            title,
            TEXT_W - if downloaded { CHECK_W } else { 0.0 },
            16.0,
        ))
        .size(16)
        .wrapping(text::Wrapping::None)
        .style(|_| bright()),
    );
    button(
        row![
            cover,
            column![
                title_line,
                text(trunc_px(meta, TEXT_W, 12.0))
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            ]
            .spacing(2)
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .on_press(msg)
    .padding(6)
    .width(Length::Fill)
    .height(Length::Fixed(56.0))
    .clip(true)
    .style(style)
    .into()
}

fn liked_tracks_tile<'a>(size: f32) -> Element<'a, Message> {
    container(
        text(icons::HEART)
            .font(FA_SOLID)
            .size((size * 0.41).round())
            .style(|_| text::Style {
                color: Some(Color::WHITE),
            }),
    )
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.33, 0.18, 0.72))),
        border: round(4.0),
        ..container::Style::default()
    })
    .into()
}

/// Playlist edits are a read-modify-PUT of the whole track list, so two at
/// once (two quick commits to one playlist) would each PUT their own read and
/// one track would silently vanish. Every edit holds this across read + PUT.
static PLAYLIST_WRITES: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

async fn do_add_to_playlist(playlist_id: i64, track_id: i64) -> Result<String> {
    after_check(|| do_add_to_playlist_once(playlist_id, track_id)).await
}

async fn do_add_to_playlist_once(playlist_id: i64, track_id: i64) -> Result<String> {
    let _serial = PLAYLIST_WRITES.lock().await;
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    api.add_track_to_playlist(&access, playlist_id, track_id)
        .await?;
    Ok("Added to playlist".to_string())
}

async fn do_remove_from_playlist(playlist_id: i64, track_id: i64) -> Result<String> {
    after_check(|| do_remove_from_playlist_once(playlist_id, track_id)).await
}

async fn do_remove_from_playlist_once(playlist_id: i64, track_id: i64) -> Result<String> {
    let _serial = PLAYLIST_WRITES.lock().await;
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    api.remove_track_from_playlist(&access, playlist_id, track_id)
        .await?;
    Ok("Removed from playlist".to_string())
}

async fn fetch_reactions(track_id: i64, seconds: Vec<u64>) -> Result<Vec<WaveReaction>, String> {
    let (api, _, auth) = make_api().await.map_err(|e| e.to_string())?;
    let access = auth.access_opt().await;
    api.track_reactions(access.as_deref(), track_id, &seconds, REACTION_PER_SECOND)
        .await
        .map_err(|e| e.to_string())
}

async fn do_add_reaction(track_id: i64, second: u64, codepoint: String) -> Result<(), String> {
    after_check(|| async {
        do_add_reaction_once(track_id, second, codepoint.clone())
            .await
            .map_err(anyhow::Error::msg)
    })
    .await
    .map_err(|e| e.to_string())
}

async fn do_add_reaction_once(track_id: i64, second: u64, codepoint: String) -> Result<(), String> {
    let (api, _, auth) = make_api().await.map_err(|e| e.to_string())?;
    let access = auth.access().await.map_err(|e| e.to_string())?;
    api.add_track_reaction(&access, track_id, second, &codepoint)
        .await
        .map_err(|e| e.to_string())
}

async fn do_post_comment(track_id: i64, text: String, ts_ms: u64) -> Result<()> {
    after_check(|| do_post_comment_once(track_id, text.clone(), ts_ms)).await
}

async fn do_post_comment_once(track_id: i64, text: String, ts_ms: u64) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    api.post_comment(&access, track_id, &text, ts_ms).await?;
    Ok(())
}

async fn fetch_search_next(next: String) -> Result<(Vec<Track>, Option<String>)> {
    let (api, _, _) = make_api().await?;
    let list = api.search_tracks_next(&next).await?;
    Ok((list.collection, list.next_href.filter(|n| !n.is_empty())))
}

async fn do_like(track_id: i64, liked: bool) -> Result<()> {
    after_check(|| do_like_once(track_id, liked)).await
}

async fn do_like_once(track_id: i64, liked: bool) -> Result<()> {
    let (api, _, auth) = make_api().await?;
    let access = auth.access().await?;
    let me = api.me(&access).await?;
    let me_id = me.id;
    if me_id == 0 {
        anyhow::bail!("no me id");
    }
    if liked {
        api.unlike_track(&access, me_id, track_id).await
    } else {
        api.like_track(&access, me_id, track_id).await
    }
}

impl App {
    fn cid(&self) -> String {
        self.state.client_id.clone()
    }

    fn update_discord_rpc(&self) {
        let Some(rpc) = &self.discord_rpc else { return };
        if !self.settings.discord_rpc || self.settings.private_session() {
            rpc.clear();
            return;
        }
        if self.story_playback {
            rpc.clear();
            return;
        }
        // A paused track is not "listening": the presence goes away until
        // playback resumes (which updates it again with the live position).
        if self.is_paused {
            rpc.clear();
            return;
        }
        if let Some(track_id) = self.playing_id {
            let track = self.find_track_anywhere(track_id);
            let artwork_url = track.as_ref().and_then(|t| {
                t.artwork_or_avatar()
                    .map(|s| s.replace("-large.jpg", "-t500x500.jpg"))
            });
            let permalink_url = track.as_ref().and_then(|t| t.permalink_url.clone());
            // Discord's bar runs at wall-clock speed: scaled to the playback speed
            rpc.update_with_speed(
                ActivityData {
                    title: self.playing_title.clone(),
                    artist: self.playing_artist.clone(),
                    artwork_url,
                    permalink_url,
                    pos_ms: self.pos_ms,
                    dur_ms: self.dur_ms,
                },
                self.playback_speed,
            );
        } else {
            rpc.clear();
        }
    }

    fn play_index(&mut self, idx: usize) -> Task<Message> {
        if idx >= self.queue.len() {
            return Task::none();
        }
        // Offline only downloaded tracks can play: step over the rest in the
        // direction of travel (then the other way) instead of failing each
        // one with a toast, which also stopped after ten in a row.
        if self.settings.offline_mode && !has_cached_audio(self.queue[idx].id) {
            let playable = |i: &usize| has_cached_audio(self.queue[*i].id);
            let ahead = || (idx + 1..self.queue.len()).find(playable);
            let behind = || (0..idx).rev().find(playable);
            // Forward stops at the end of the queue (Repeat All wraps to
            // its first download): searching backwards from there replayed
            // the last download for ever.
            let found = if idx < self.queue_pos {
                behind().or_else(ahead)
            } else if self.repeat == RepeatMode::All {
                ahead().or_else(|| (0..idx).find(playable))
            } else {
                ahead()
            };
            return match found {
                Some(i) => self.play_index(i),
                None => {
                    if self.at_end {
                        // the last download finished: show it stopped
                        self.is_paused = true;
                        self.update_discord_rpc();
                    }
                    self.show_toast("No downloaded tracks to play here", ToastKind::Info);
                    Task::none()
                }
            };
        }
        self.queue_pos = idx;
        let track = self.queue[idx].clone();
        // a new track: it may crossfade into the next one in turn
        self.crossfaded_from = None;
        // Record in history (most recent first, deduplicated by id, capped
        // at 100); a private session keeps it out
        if !self.settings.private_session() {
            self.history.retain(|t| t.id != track.id);
            self.history.insert(0, track.clone());
            if self.history.len() > 100 {
                self.history.truncate(100);
            }
            self.save_history();
        }
        self.playing_id = Some(track.id);
        let (display_artist, display_title) =
            track.display_artist_and_title(self.settings.prefer_artist_from_name);
        self.playing_title = display_title;
        self.playing_artist = display_artist;
        self.is_paused = false;
        self.at_end = false;
        self.awaiting_audio = None;
        // the new track's length first: the resume point is clamped to it,
        // not to the length of the track that was playing
        self.dur_ms = self.track_ms(&track).unwrap_or(0);
        let resume_ms = self
            .pending_seek_ms
            .filter(|_| self.pending_seek_track == Some(track.id))
            .unwrap_or(0);
        self.pos_ms = if self.dur_ms > 0 {
            resume_ms.min(self.dur_ms)
        } else {
            resume_ms
        };
        self.play_gen = self.play_gen.wrapping_add(1);
        let gen = self.play_gen;
        if let Some(p) = &self.player {
            // Stop the old track now, like open_story: while the new one
            // resolves it would keep playing, and its end would set `ended`
            // and skip past the track just picked. A crossfade lets it fade
            // out under this one instead.
            if std::mem::take(&mut self.crossfade_next) {
                let ms = u64::from(self.settings.crossfade_secs) * 1000;
                p.send(PlayerCommand::FadeOut(ms));
            } else {
                p.send(PlayerCommand::Stop);
            }
            p.ended.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        // every track starts at its own speed: the one last set for it, or 1.0x
        self.apply_track_speed(track.id);
        self.update_discord_rpc();
        crate::log!(
            "play: [{}] {} — {} (id {}, {}ms, quality={})",
            idx + 1,
            track.title,
            track
                .user
                .as_ref()
                .map(|u| u.username.as_str())
                .unwrap_or("?"),
            track.id,
            self.dur_ms,
            self.settings.audio_quality.as_deref().unwrap_or("hls")
        );
        // reset wave/comments, load fresh ones
        self.wave_bars = std::sync::Arc::new(Vec::new());
        self.wave_comments.clear();
        self.reactions.clear();
        self.reactions_fetched.clear();
        self.reactions_inflight = false;
        self.reaction_sec = None;
        self.floating.clear();
        self.comments_next = None;
        self.comments_loading = false;
        self.comments_pages = 0;
        // the comment/reaction panel belonged to the track that's leaving
        self.wave_context_frac = None;
        self.wave_context_track = None;
        self.last_posted_reaction = None;
        let quality = self
            .settings
            .audio_quality
            .clone()
            .unwrap_or_else(|| "hls".into());

        let cached = crate::config::cached_audio_path(track.id);
        // Only a kept track plays from disk: a download, anything with Cache
        // tracks locally on, and offline whatever is there.
        let may_use_cache = self.keeps_locally(track.id) || self.settings.offline_mode;
        if self.settings.cache_tracks && !self.settings.offline_mode {
            // the track's details go with its audio
            let info = track.clone();
            save_in_background(crate::config::cached_track_info_path(track.id), move || {
                serde_json::to_vec(&info).ok()
            });
        }
        let is_cached_valid = may_use_cache
            && if cached.exists() && cached.metadata().map(|m| m.len() > 10000).unwrap_or(false) {
                if let Ok(file) = std::fs::File::open(&cached) {
                    use std::io::Read as _;
                    let mut header = [0u8; 8];
                    if (&file).read_exact(&mut header).is_ok() {
                        // What the player's decoder sniffs for: MP3, or fMP4
                        // (AAC/HLS downloads), whose box type sits at bytes 4..8
                        // after the box size.
                        &header[..3] == b"ID3"
                            || header[0] == 0xFF
                            || &header[4..8] == b"ftyp"
                            || &header[4..8] == b"styp"
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
        // Offline mode plays only downloaded tracks and asks the network for
        // nothing (no resolve, waveform, comments or prefetch).
        let offline = self.settings.offline_mode;
        if offline && !is_cached_valid {
            return self.skip_unplayable("not downloaded (offline mode)");
        }
        self.bypass_proxy = None;
        self.bypass_attempts = 0;
        self.yt_video = None;
        self.yt_ad = false;
        // A Go+ track plays in full on YouTube Music, not as the preview a
        // play without it may have cached.
        let on_youtube = self.plays_on_youtube(&track);
        let is_cached_valid = is_cached_valid && !on_youtube;
        let blocked_here = track.policy.as_deref() == Some("BLOCK");
        let stream_task = if is_cached_valid {
            crate::log!(
                "instant start: using local cached audio for track {}",
                track.id
            );
            Task::done(Message::CachedAudioReady(gen, track.id, cached))
        } else if blocked_here && self.settings.bypass_unavailable {
            self.start_bypass(gen, track.id, None)
        } else if blocked_here && self.youtube_usable() {
            self.start_youtube(gen, track.clone(), YtFallback::Skip)
        } else if on_youtube {
            self.start_youtube(gen, track.clone(), YtFallback::Preview)
        } else if let Some((p_id, p_src)) = self.prefetched_stream.take().filter(|_| {
            self.prefetched_at
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(180))
        }) {
            if p_id == track.id {
                crate::log!("instant start: using prefetched stream for track {p_id}");
                Task::done(Message::StreamReady(gen, Ok((p_id, p_src))))
            } else {
                let q = quality.clone();
                let tr = track.clone();
                Task::perform(resolve_stream_from_track(tr, q), move |r| match r {
                    Ok(t) => Message::StreamReady(gen, Ok(t)),
                    Err(e) => Message::StreamReady(gen, Err(e.to_string())),
                })
            }
        } else {
            let q = quality.clone();
            let tr = track.clone();
            Task::perform(resolve_stream_from_track(tr, q), move |r| match r {
                Ok(t) => Message::StreamReady(gen, Ok(t)),
                Err(e) => Message::StreamReady(gen, Err(e.to_string())),
            })
        };
        let cached_wave = crate::config::cached_waveform_path(track.id);
        let wave_task = if cached_wave.exists() {
            if let Ok(bytes) = std::fs::read(&cached_wave) {
                Task::done(Message::WaveLoaded(Ok((track.id, bytes))))
            } else {
                Task::perform(fetch_wave(track.id), |r| match r {
                    Ok(x) => Message::WaveLoaded(Ok(x)),
                    Err(e) => Message::WaveLoaded(Err(e.to_string())),
                })
            }
        } else if offline {
            Task::none()
        } else {
            Task::perform(fetch_wave(track.id), |r| match r {
                Ok(x) => Message::WaveLoaded(Ok(x)),
                Err(e) => Message::WaveLoaded(Err(e.to_string())),
            })
        };

        let mut tasks = vec![stream_task, wave_task, self.load_wave_visual(&track)];
        // comments are cache-first: offline only a cached set is loaded
        if !offline || crate::config::cached_comments_path(track.id).exists() {
            tasks.push(Task::perform(fetch_comments(track.id, offline), |r| {
                Message::CommentsLoaded(r.map_err(|e| e.to_string()))
            }));
        }
        if let Some(art) = track.artwork_or_avatar() {
            // warm the disk cache; the player-bar tile decodes on demand
            tasks.push(Task::perform(
                fetch_artwork(vec![art.to_string()]),
                Message::ArtworkLoaded,
            ));
        }
        Task::batch(tasks)
    }

    pub fn play_track(&mut self, track: Track) -> Task<Message> {
        self.story_playback = false;
        self.pending_seek_ms = None;
        self.pending_seek_track = None;
        let pos = self.set_queue(vec![track], 0);
        self.play_index(pos)
    }

    fn play_story_track(&mut self, track: Track) -> Task<Message> {
        if !self.story_playback {
            self.story_return_state = self.queue.get(self.queue_pos).map(|_| StoryReturnState {
                queue: self.queue.clone(),
                queue_pos: self.queue_pos,
                pos_ms: self.pos_ms,
                was_paused: self.is_paused,
            });
        }
        self.story_playback = true;
        // not set_queue: the queue's order (and shuffle) must survive for
        // when the story closes and the saved queue comes back
        self.queue = vec![track.clone()];
        let midpoint = self.track_ms(&track).unwrap_or(0) / 2;
        self.pending_seek_ms = Some(midpoint);
        self.pending_seek_track = Some(track.id);
        self.play_index(0)
    }

    /// Shuffle's "Fewer repeats": tracks by one artist moved apart, so the
    /// same artist doesn't play twice in a row where the list allows it.
    /// The first track stays put (it's playing, or about to).
    fn spread_artists(&self, tracks: &mut [Track]) {
        if self.settings.shuffle_style != crate::config::ShuffleStyle::FewerRepeats {
            return;
        }
        let artist = |t: &Track| t.user.as_ref().map(|u| u.id);
        for i in 1..tracks.len() {
            let prev = artist(&tracks[i - 1]);
            if prev.is_none() || artist(&tracks[i]) != prev {
                continue;
            }
            if let Some(j) = (i + 1..tracks.len()).find(|&j| artist(&tracks[j]) != prev) {
                tracks.swap(i, j);
            }
        }
    }

    /// Replace the play queue, to start at `start`; returns the index to
    /// play. In shuffle the start goes first and the rest follow in a
    /// random order, so Next Up lists them as they'll play.
    fn set_queue(&mut self, mut tracks: Vec<Track>, start: usize) -> usize {
        self.queue_order = tracks.iter().map(|t| t.id).collect();
        let start = start.min(tracks.len().saturating_sub(1));
        if self.shuffle && tracks.len() > 1 {
            use rand::seq::SliceRandom;
            tracks.swap(0, start);
            tracks[1..].shuffle(&mut rand::thread_rng());
            self.spread_artists(&mut tracks);
            self.queue = tracks;
            return 0;
        }
        self.queue = tracks;
        start
    }

    /// Shuffle turned on: what's still to come plays in a random order.
    /// What already played stays behind it, for Prev.
    fn shuffle_upcoming(&mut self) {
        use rand::seq::SliceRandom;
        let from = (self.queue_pos + 1).min(self.queue.len());
        self.queue[from..].shuffle(&mut rand::thread_rng());
        // from the playing track on, so the first shuffled one isn't its
        // artist's either
        let mut upcoming = std::mem::take(&mut self.queue);
        let start = self.queue_pos.min(upcoming.len());
        self.spread_artists(&mut upcoming[start..]);
        self.queue = upcoming;
    }

    /// Shuffle turned off: the queue goes back to its own order and goes on
    /// from the playing track. Tracks added since stay at the end.
    fn unshuffle(&mut self) {
        let mut rank = std::collections::HashMap::new();
        for (i, id) in self.queue_order.iter().enumerate() {
            rank.entry(*id).or_insert(i);
        }
        let mut items: Vec<(usize, Track)> = std::mem::take(&mut self.queue)
            .into_iter()
            .enumerate()
            .collect();
        // stable: tracks with the same rank keep their order
        items.sort_by_key(|(_, t)| rank.get(&t.id).copied().unwrap_or(usize::MAX));
        self.queue_pos = items
            .iter()
            .position(|(i, _)| *i == self.queue_pos)
            .unwrap_or(0);
        self.queue = items.into_iter().map(|(_, t)| t).collect();
    }

    /// Repeat All in shuffle, at the end of a round: the next round plays in
    /// a new order, which doesn't start with the track that just ended.
    fn reshuffle_round(&mut self) {
        use rand::seq::SliceRandom;
        let cur = self.queue.get(self.queue_pos).map(|t| t.id);
        self.queue.shuffle(&mut rand::thread_rng());
        if self.queue.len() > 1 && self.queue.first().map(|t| t.id) == cur {
            let last = self.queue.len() - 1;
            self.queue.swap(0, last);
        }
        let mut round = std::mem::take(&mut self.queue);
        self.spread_artists(&mut round);
        self.queue = round;
        self.queue_pos = self
            .queue
            .iter()
            .position(|t| Some(t.id) == cur)
            .unwrap_or(0);
    }

    /// The list a `track_row` click came from: the one on the open page that
    /// holds `id`. (A fixed search > playlist > library priority used to
    /// queue a list the track wasn't even in, and play its first track.)
    fn row_context(&self, id: i64) -> Option<Vec<Track>> {
        let has = |l: &Vec<Track>| l.iter().any(|t| t.id == id);
        let list: Option<&Vec<Track>> = match self.tab {
            Tab::Search => Some(&self.search.tracks),
            Tab::Library => Some(&self.library),
            Tab::Playlist => self.current_playlist.as_ref().map(|p| &p.tracks),
            Tab::Home => self.home.iter().find_map(|sec| match &sec.shelf {
                HomeShelf::Tracks(tracks) if has(tracks) => Some(tracks),
                _ => None,
            }),
            Tab::Profile => self.profile.as_ref().and_then(|p| match p.active_tab {
                ProfileSubTab::Overview => [&p.top_tracks, &p.reposts].into_iter().find(|l| has(l)),
                ProfileSubTab::Tracks if p.all_tracks.is_empty() => Some(&p.top_tracks),
                ProfileSubTab::Tracks => Some(&p.all_tracks),
                ProfileSubTab::Likes => Some(&p.likes),
                _ => None,
            }),
            Tab::Track => self.track_page.as_ref().map(|p| &p.related_tracks),
            Tab::Settings => None,
        };
        list.filter(|l| has(l)).cloned()
    }

    /// The current track can't be played: skip it, unless every track in the
    /// queue (or 10) failed in a row, or it's a story — then stop and say so,
    /// instead of showing it as playing over silence.
    fn skip_unplayable(&mut self, why: &str) -> Task<Message> {
        self.awaiting_audio = None;
        // nothing is loaded, whether we skip or stop
        self.at_end = true;
        self.play_failures += 1;
        let title = trunc(&self.playing_title, 24);
        if self.play_failures >= self.queue.len().min(10) {
            self.play_failures = 0;
            self.is_paused = true;
            // and mean it: a stream still resolving (or a slow decoder) must
            // not start under a bar that shows stopped
            self.play_gen = self.play_gen.wrapping_add(1);
            if let Some(p) = &self.player {
                p.send(PlayerCommand::Stop);
            }
            self.update_discord_rpc();
            self.show_toast(format!("Can't play '{title}': {why}"), ToastKind::Error);
            return Task::none();
        }
        self.show_toast(format!("Can't play '{title}', skipping"), ToastKind::Error);
        self.update(Message::NextTrack)
    }

    fn find_track_anywhere(&self, id: i64) -> Option<Track> {
        if let Some(detail) = &self.current_playlist {
            if let Some(t) = detail.tracks.iter().find(|t| t.id == id) {
                return Some(t.clone());
            }
        }
        for sec in &self.home {
            match &sec.shelf {
                HomeShelf::Playlists(playlists) => {
                    for p in playlists {
                        if let Some(t) = p.tracks.iter().find(|t| t.id == id) {
                            return Some(t.clone());
                        }
                    }
                }
                HomeShelf::Tracks(tracks) => {
                    if let Some(t) = tracks.iter().find(|t| t.id == id) {
                        return Some(t.clone());
                    }
                }
            }
        }
        let profile = self.profile.iter().flat_map(|p| {
            p.top_tracks
                .iter()
                .chain(&p.all_tracks)
                .chain(&p.reposts)
                .chain(&p.likes)
        });
        let track_page = self
            .track_page
            .iter()
            .flat_map(|p| std::iter::once(&p.track).chain(&p.related_tracks));
        self.queue
            .iter()
            .chain(self.library.iter())
            .chain(self.search.tracks.iter())
            .chain(profile)
            .chain(track_page)
            .find(|t| t.id == id)
            .cloned()
    }

    fn push_nav(&mut self, next: Route) {
        self.show_user_menu = false;
        self.close_page_menu();
        self.clear_page_hover();
        // the user went somewhere: a name lookup still running mustn't
        // yank them to a profile later
        self.artist_lookup = None;
        let here = self.current_route();
        if !self.nav_restoring && !here.same(&next) {
            self.nav_history.push(here);
            if self.nav_history.len() > 100 {
                self.nav_history.remove(0);
            }
            self.nav_future.clear();
        }
        self.tab = next.tab();
    }

    /// The page changes: the hover highlight of a row or link that goes
    /// with it never gets its "mouse left", and would stay lit on whatever
    /// takes its place (the same track on the next page).
    fn clear_page_hover(&mut self) {
        self.hovered_track_row = None;
        self.row_artist_hover = None;
        if matches!(
            self.pb_hover,
            Some(PbLink::PlaylistAuthor | PbLink::Owner(_))
        ) {
            self.pb_hover = None;
        }
    }

    /// The page on screen, as a history step.
    fn current_route(&self) -> Route {
        match self.tab {
            Tab::Playlist => match &self.current_playlist {
                Some(p) => Route::Playlist(p.id_or_urn.clone()),
                None => Route::Tab(Tab::Playlist),
            },
            Tab::Profile => match self.loading_profile.or(self.profile.as_ref().map(|p| p.id)) {
                Some(id) => Route::Profile(id),
                None => Route::Tab(Tab::Profile),
            },
            Tab::Track => match &self.track_page {
                Some(p) => Route::Track(Box::new(p.track.clone())),
                None => Route::Tab(Tab::Track),
            },
            ref tab => Route::Tab(tab.clone()),
        }
    }

    /// Back / Forward: show `route` again, reopening its playlist, profile
    /// or track page unless it's the one still loaded.
    fn restore_route(&mut self, route: Route) -> Task<Message> {
        self.close_page_menu();
        self.clear_page_hover();
        self.artist_lookup = None;
        let reopen = match route {
            Route::Playlist(urn)
                if self
                    .current_playlist
                    .as_ref()
                    .is_none_or(|p| p.id_or_urn != urn) =>
            {
                Message::OpenPlaylist(urn)
            }
            Route::Profile(id)
                if self.loading_profile != Some(id)
                    && self.profile.as_ref().is_none_or(|p| p.id != id) =>
            {
                Message::OpenProfile(id)
            }
            Route::Track(track)
                if self
                    .track_page
                    .as_ref()
                    .is_none_or(|p| p.track.id != track.id) =>
            {
                Message::OpenTrackPage(track)
            }
            route => {
                self.tab = route.tab();
                return Task::none();
            }
        };
        self.nav_restoring = true;
        let task = self.update(reopen);
        self.nav_restoring = false;
        task
    }

    /// A page's "..." menu belongs to that page: leaving it closes the menu.
    fn close_page_menu(&mut self) {
        if matches!(self.action_menu, Some(ActionMenu::Collection(_)))
            || self.menu_anchor == Some(MenuAnchor::Page)
        {
            self.action_menu = None;
            self.menu_anchor = None;
        }
    }
}

impl App {
    fn subscription(&self) -> Subscription<Message> {
        // Player shortcuts: keys no widget took (a focused text field keeps
        // its own typing), matched by physical key so they work on any
        // keyboard layout (M types "ь" on a Russian one).
        let key_sub = iced::event::listen_with(|event, status, _window| match (event, status) {
            (
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                }),
                iced::event::Status::Ignored,
            ) => shortcut(&key, &physical_key, modifiers),
            _ => None,
        });
        // Media keys from anywhere, even with another window in front.
        let media_sub = crate::media_keys::subscription().map(|k| match k {
            crate::media_keys::MediaKey::Stop => Message::PlayerPause,
            crate::media_keys::MediaKey::PlayPause => Message::PlayerToggle,
            crate::media_keys::MediaKey::Next => Message::NextTrack,
            crate::media_keys::MediaKey::Previous => Message::PrevTrack,
        });

        // Escape consumed by a focused field / hovered slider never reaches
        // the shortcuts; bar popups still close on it.
        let escape_sub = iced::event::listen_with(|event, status, _window| match (event, status) {
            (
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                }),
                iced::event::Status::Captured,
            ) => Some(Message::EscapePopups),
            _ => None,
        });

        let yt_sub = crate::yt_music::subscription().map(Message::Yt);
        let mut subs = vec![key_sub, media_sub, escape_sub, yt_sub];
        let wave_target = if self.is_paused || self.playing_id.is_none() {
            0.0
        } else {
            1.0
        };
        let vol_target = if self.is_muted { 0.0 } else { 1.0 };
        let animating_wave = (self.wave_color_t - wave_target).abs() > 0.005;
        let animating_vol = (self.vol_color_t - vol_target).abs() > 0.005;
        // the toast moves only while it fades in (0.22 s) and out (from 2.8 s;
        // a little early, since the next re-check may be a Tick away)
        let animating_toast = self.toast.as_ref().is_some_and(|t| {
            let s = t.created_at.elapsed().as_secs_f32();
            s < 0.3 || s > 2.55
        });
        // reactions queued for later seconds aren't on screen yet
        let now = std::time::Instant::now();
        let animating_reactions =
            !self.particles.is_empty() || self.floating.iter().any(|r| r.spawned_at <= now);
        let minimized = self.window_size.width < 1.0 || self.window_size.height < 1.0;

        // Every tick rebuilds the whole UI, so frames come only as fast as
        // something on screen changes. 60fps while something animates;
        // while playing, just often enough to move the playhead a pixel at
        // a time (a long track in a narrow window needs no more than the
        // 250ms Tick); 25fps for a visible spinner. Minimized, only the
        // Tick, which still advances the queue.
        let frame_ms = if minimized {
            None
        } else if animating_wave || animating_vol || animating_toast || animating_reactions {
            Some(16)
        } else {
            self.playhead_frame_ms()
                .or(self.spinner_visible().then_some(40))
        };
        if let Some(ms) = frame_ms {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(ms)).map(|_| Message::AnimTick),
            );
        }
        subs.push(iced::time::every(std::time::Duration::from_millis(250)).map(|_| Message::Tick));
        // reactions are fetched a minute ahead while playing (see poll_reactions)
        if !self.is_paused
            && self.playing_id.is_some()
            && self.state.authenticated
            && !self.settings.offline_mode
        {
            subs.push(
                iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| Message::ReactionsPoll),
            );
        }
        // maximize / restore / snap / edge-drag all report here
        subs.push(iced::window::resize_events().map(|(_, size)| Message::WindowResized(size)));
        subs.push(iced::window::close_requests().map(|_| Message::WindowClose));
        Subscription::batch(subs)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        if crate::console::debug_enabled() {
            debug_message(&msg);
        }
        match msg {
            // Account actions write to SoundCloud: say so offline instead of
            // flipping local state that the failed request then leaves wrong.
            Message::LikeTrack(_)
            | Message::LikeCurrent
            | Message::TrackRepostToggle(_)
            | Message::PlaylistLikeToggle(_)
            | Message::FollowToggle
            | Message::FollowUserToggle(..)
            | Message::SaveCurrentClicked
            | Message::SaveTrackClicked(..)
            | Message::OpenAddPopover(..)
            | Message::OpenAddPopoverCurrent
            | Message::OpenTrackInspector(_)
            | Message::WaveContextPostComment
            | Message::WaveContextReact(_)
            | Message::TrackPagePostComment
            | Message::TrackPageReact(_)
            | Message::SidebarCreatePlaylist
            | Message::RequestDeletePlaylist(..)
            | Message::DeletePlaylist(_)
            | Message::ArtistClicked(_)
                if self.settings.offline_mode =>
            {
                self.show_toast("Not available in offline mode", ToastKind::Info);
                Task::none()
            }
            // An account's data landing after it signed out (these start only
            // once a sign-in is confirmed): not shown, not saved back to disk.
            Message::LibraryLoaded(Ok(_))
            | Message::FollowingsLoaded(Ok(_))
            | Message::LikedPlaylistsLoaded(Ok(_))
            | Message::UserFlagsLoaded(Ok(_))
            | Message::StoriesLoaded(Ok(_))
                if !self.state.authenticated =>
            {
                self.library_loading = false;
                Task::none()
            }
            Message::ApiReady(Ok(state)) => {
                self.state = state;
                crate::log!(
                    "api ready: authenticated={}, client_id={}…",
                    self.state.authenticated,
                    &self.state.client_id.chars().take(8).collect::<String>()
                );
                if let Some(me) = &self.state.me {
                    crate::log!("logged in as @{}", me.username);
                    self.offline_store.me = Some(me.clone());
                    self.save_offline_store();
                }
                self.login_error = if self.state.authenticated {
                    self.state
                        .me
                        .as_ref()
                        .map(|m| format!("Logged in as {}", clean_username(&m.username)))
                } else {
                    None
                };
                let artwork_task = Task::perform(
                    fetch_artwork(artwork_urls(
                        &self.home,
                        self.current_playlist.as_ref(),
                        &self.search,
                        self.state.me.as_ref(),
                    )),
                    Message::ArtworkLoaded,
                );
                if self.state.authenticated {
                    crate::log!("fetching full likes library…");
                    self.library_loading = true;
                    return Task::batch(vec![
                        Task::perform(fetch_library(), |r| match r {
                            Ok(t) => Message::LibraryLoaded(Ok(t)),
                            Err(e) => Message::LibraryLoaded(Err(e.to_string())),
                        }),
                        Task::perform(fetch_user_flags(), |r| match r {
                            Ok(x) => Message::UserFlagsLoaded(Ok(x)),
                            Err(e) => Message::UserFlagsLoaded(Err(e.to_string())),
                        }),
                        Task::perform(fetch_liked_playlists(), |r| match r {
                            Ok(list) => Message::LikedPlaylistsLoaded(Ok(list)),
                            Err(e) => Message::LikedPlaylistsLoaded(Err(e.to_string())),
                        }),
                        self.fetch_playlists_task(),
                        self.fetch_followings_task(),
                        Task::perform(fetch_stories(), |r| match r {
                            Ok(s) => Message::StoriesLoaded(Ok(s)),
                            Err(e) => Message::StoriesLoaded(Err(e.to_string())),
                        }),
                        artwork_task,
                    ]);
                }
                artwork_task
            }
            Message::ApiReady(Err(e)) => {
                crate::log!("api ready FAILED: {e}");
                self.login_error = Some(e);
                Task::none()
            }
            // a feed that was loading when offline mode went on: its tracks
            // wouldn't play, Home stays the downloads
            Message::HomeLoaded(_) if self.settings.offline_mode => Task::none(),
            Message::HomeLoaded(Ok(sections)) => {
                self.home_loading = false;
                crate::log!("home loaded: {} sections", sections.len());
                // Weak fallback only: real stories come from the follow-feed
                // stream (StoriesLoaded). Never overwrite those.
                if !self.stories_from_stream && self.stories.is_empty() {
                    let extracted = extract_stories_from_home(&sections);
                    if !extracted.is_empty() {
                        self.stories = extracted;
                        self.save_stories();
                    }
                }
                self.home = sections;
                if self.home.is_empty() {
                    self.home_error = Some("The feed came back empty".into());
                } else {
                    self.library_items_from_home();
                }
                let mut urls = artwork_urls(
                    &self.home,
                    self.current_playlist.as_ref(),
                    &self.search,
                    self.state.me.as_ref(),
                );
                for s in &self.stories {
                    if let Some(u) = s.avatar_url.as_ref() {
                        urls.push(u.clone());
                    }
                    if let Some(u) = s.artwork_url.as_ref() {
                        urls.push(u.clone());
                    }
                }
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::HomeLoaded(Err(e)) => {
                self.home_loading = false;
                crate::log!("home FAILED: {e}");
                self.home_error = Some(format!("Couldn't load the feed: {e}"));
                Task::none()
            }
            Message::ReloadHome => self.load_home(),
            Message::LibraryLoaded(Ok(tracks)) => {
                self.library_loading = false;
                crate::log!("library loaded: {} liked tracks", tracks.len());
                self.library = tracks;
                // Rebuilt, not merged: this is every like of the account, and
                // merging kept unliked tracks and a previous account's likes.
                self.liked_ids = self.library.iter().map(|t| t.id).collect();
                self.save_library_cache();
                let urls = extract_tracks_artwork(&self.library);
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::LibraryLoaded(Err(e)) => {
                self.library_loading = false;
                self.login_error = Some(format!("library: {e}"));
                Task::none()
            }
            Message::ArtworkLoaded(network_ok) => {
                // Disk cache warmed. Only a real download proves the network is
                // back, so only then let previously failed requests retry early.
                if network_ok && !self.art_failed.is_empty() {
                    self.art_failed.clear();
                }
                self.pump_art()
            }
            Message::ArtworkReady((processed, requested)) => {
                let mut done: std::collections::HashSet<u64> = std::collections::HashSet::new();
                for ((url, circle, px), w, h, rgba, tint) in processed {
                    let key = art_key_hash(&url, circle, px);
                    done.insert(key);
                    let bytes = w as usize * h as usize * 4;
                    self.artwork
                        .insert(key, image::Handle::from_rgba(w, h, rgba));
                    if let Some(old) = self.artwork_sizes.insert(key, bytes) {
                        self.artwork_bytes -= old;
                    }
                    self.artwork_bytes += bytes;
                    self.art_colors.insert(url, tint);
                }
                // Requests that produced nothing (404, decode error) are parked
                // with a cooldown (see pump_art) before they may retry.
                for (url, circle, px) in &requested {
                    let key = art_key_hash(url, *circle, *px);
                    self.art_inflight.remove(&key);
                    if !done.contains(&key) {
                        self.art_failed.insert(key, std::time::Instant::now());
                    }
                }
                // Bound the decoded-pixel cache, but never evict art the views
                // drew recently: evicting on-screen tiles would just refetch
                // and re-decode them forever.
                // Held to 48 MB of pixels (tiles from 44px thumbnails to 500px
                // covers); what's gone decodes again from the disk cache.
                const ART_BUDGET: usize = 48 * 1024 * 1024;
                if self.artwork_bytes > ART_BUDGET {
                    let touched = self.art_touched.borrow().clone();
                    let mut victims: Vec<(u64, usize)> = self
                        .artwork_sizes
                        .iter()
                        .filter(|(k, _)| !touched.contains(k))
                        .map(|(k, b)| (*k, *b))
                        .collect();
                    // the biggest first: fewest tiles to decode again
                    victims.sort_by(|a, b| b.1.cmp(&a.1));
                    for (k, bytes) in victims {
                        if self.artwork_bytes <= ART_BUDGET {
                            break;
                        }
                        self.artwork.remove(&k);
                        self.artwork_sizes.remove(&k);
                        self.artwork_bytes -= bytes;
                    }
                    self.art_touched.borrow_mut().clear();
                }
                Task::none()
            }
            Message::StoriesLoaded(Ok(stories)) => {
                if !stories.is_empty() {
                    crate::log!("stories: {} artist updates loaded", stories.len());
                    // Merge server-side read state into the local read set.
                    for s in &stories {
                        if s.server_read {
                            self.stories_read.insert(s.track_id);
                        }
                    }
                    // Keep an open story overlay pointing at the same story
                    // after the list is replaced (or close it if it's gone).
                    let open_track = self
                        .active_story_index
                        .and_then(|i| self.stories.get(i))
                        .map(|s| s.track_id);
                    self.stories = stories;
                    if let Some(tid) = open_track {
                        self.active_story_index =
                            self.stories.iter().position(|s| s.track_id == tid);
                    }
                    self.stories_from_stream = true;
                    self.save_stories();
                    let mut urls = Vec::new();
                    for s in &self.stories {
                        if let Some(u) = s.avatar_url.as_ref() {
                            urls.push(u.clone());
                        }
                        if let Some(u) = s.artwork_url.as_ref() {
                            urls.push(u.clone());
                        }
                    }
                    // the open story may have gone from the new list: then
                    // it closes (and its preview stops) instead of playing on
                    // hidden, with the player bar blank
                    let window = if open_track.is_some() && self.active_story_index.is_none() {
                        self.close_story()
                    } else {
                        self.sync_story_window()
                    };
                    return Task::batch([
                        Task::perform(fetch_artwork(urls), Message::ArtworkLoaded),
                        window,
                    ]);
                } else {
                    crate::log!(
                        "stories: no artist updates found (followed artists have no recent posts)"
                    );
                }
                Task::none()
            }
            Message::StoriesLoaded(Err(e)) => {
                crate::log!("stories: follow feed unavailable ({e}), keeping fallback");
                Task::none()
            }
            Message::StoryReceiptDone(key, attempt, result) => {
                if !self.story_receipts_pending.contains(&key) {
                    return Task::none();
                }
                match story_receipt_outcome(attempt, result.is_ok()) {
                    StoryReceiptOutcome::Complete => {
                        self.story_receipts_pending.remove(&key);
                        self.stories_read.insert(key.track_id);
                        if let Some(story) = self.stories.iter_mut().find(|story| {
                            story.user_id == key.user_id
                                && story.track_id == key.track_id
                                && story.created_at_ms == key.created_ms
                        }) {
                            story.server_read = true;
                        }
                        self.save_stories_read();
                        Task::none()
                    }
                    StoryReceiptOutcome::Retry => {
                        if let Err(error) = result {
                            crate::log!("story read receipt failed; retrying once: {error}");
                        }
                        Task::perform(
                            async move {
                                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                                mark_story_read(key.user_id, key.created_ms).await
                            },
                            move |result| Message::StoryReceiptDone(key, attempt + 1, result),
                        )
                    }
                    StoryReceiptOutcome::Failed => {
                        self.story_receipts_pending.remove(&key);
                        self.stories_read.remove(&key.track_id);
                        self.save_stories_read();
                        let error = result.err().unwrap_or_else(|| "unknown error".into());
                        crate::log!("story read receipt failed after retry: {error}");
                        self.show_toast(
                            "Couldn't sync story as viewed with SoundCloud",
                            ToastKind::Error,
                        );
                        Task::none()
                    }
                }
            }
            Message::CloseStoryThen(then) => {
                let close = self.close_story();
                Task::batch([close, self.update(*then)])
            }
            Message::OpenImageViewer(url) => {
                self.image_viewer = Some(ImageViewer {
                    url: url.clone(),
                    handle: None,
                });
                Task::perform(fetch_full_image(url), Message::FullImageLoaded)
            }
            Message::CloseImageViewer => {
                self.image_viewer = None;
                Task::none()
            }
            Message::FullImageLoaded(Ok((url, w, h, px))) => {
                if let Some(v) = self.image_viewer.as_mut() {
                    // Ignore a load that finished after the user opened another photo.
                    if v.url == url {
                        v.handle = Some(image::Handle::from_rgba(w, h, px));
                    }
                }
                Task::none()
            }
            Message::FullImageLoaded(Err(e)) => {
                crate::log!("full image failed: {e}");
                // don't leave an empty viewer over the app (one already
                // showing a photo is a later request's, and stays)
                if self
                    .image_viewer
                    .as_ref()
                    .is_some_and(|v| v.handle.is_none())
                {
                    self.image_viewer = None;
                    self.show_toast("Couldn't load the image", ToastKind::Error);
                }
                Task::none()
            }
            Message::WindowResized(size) => {
                if size.width < 1.0 || size.height < 1.0 {
                    // minimized: the decoded covers go (they come back from
                    // the disk cache when shown)
                    self.artwork.clear();
                    self.artwork_sizes.clear();
                    self.artwork_bytes = 0;
                    crate::log!("minimized: decoded artwork released");
                }
                self.window_size = size;
                // the waveform's width follows the window
                self.bake_wave_visual()
            }
            Message::Noop => Task::none(),
            Message::SearchChanged(q) => {
                self.search_query = q;
                Task::none()
            }
            Message::SearchTag(tag) => {
                let q = format!("#{tag}");
                crate::log!("search: \"{q}\"");
                self.search_query = q.clone();
                self.push_nav(Route::Tab(Tab::Search));
                self.start_search(q)
            }
            Message::SearchSubmit => {
                let q = self.search_query.trim().to_string();
                if q.is_empty() {
                    return Task::none();
                }
                crate::log!("search: \"{q}\"");
                self.push_nav(Route::Tab(Tab::Search));
                self.start_search(q)
            }
            Message::SearchLoaded(gen, _) if gen != self.search_gen => Task::none(),
            Message::SearchLoaded(_, Ok((res, next))) => {
                self.search_loading = false;
                self.page_error = None;
                crate::log!(
                    "search done: {} tracks, {} users, {} playlists",
                    res.tracks.len(),
                    res.users.len(),
                    res.playlists.len()
                );
                self.search_next = next;
                self.search = res;
                let urls = artwork_urls(
                    &self.home,
                    self.current_playlist.as_ref(),
                    &self.search,
                    self.state.me.as_ref(),
                );
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::SearchLoaded(_, Err(e)) => {
                self.search_loading = false;
                let q = self.search_query.trim().to_string();
                self.page_error = Some((
                    Tab::Search,
                    format!("Search failed: {e}"),
                    Box::new(Message::SearchRetry(q)),
                ));
                Task::none()
            }
            Message::SearchRetry(q) => self.start_search(q),
            Message::ToggleQueue => {
                self.show_queue = !self.show_queue;
                Task::none()
            }
            Message::PlayQueueTrack(idx) => self.play_index(idx),
            Message::ClearUpcomingQueue => {
                if self.queue_pos + 1 < self.queue.len() {
                    self.queue.truncate(self.queue_pos + 1);
                }
                Task::none()
            }
            Message::OpenPlaylist(id_or_urn) => {
                self.push_nav(Route::Playlist(id_or_urn.clone()));
                let preview = self
                    .home
                    .iter()
                    .find_map(|sec| match &sec.shelf {
                        HomeShelf::Playlists(pls) => pls
                            .iter()
                            .find(|p| p.id_or_urn == id_or_urn)
                            .map(|p| PlaylistDetail {
                                id: playlist_id_for(&p.id_or_urn),
                                id_or_urn: p.id_or_urn.clone(),
                                title: p.title.clone(),
                                description: Some(p.subtitle.clone()),
                                artwork_url: p.artwork_url.clone(),
                                // the owner until the full load; a mix is SoundCloud's
                                author: p
                                    .owner
                                    .as_ref()
                                    .map(|u| clean_username(&u.username).to_string())
                                    .unwrap_or_else(|| "SoundCloud".into()),
                                author_avatar: p.owner.as_ref().and_then(|u| u.avatar_url.clone()),
                                author_id: p.owner.as_ref().map(|u| u.id),
                                permalink_url: None,
                                track_count: p.tracks.len(),
                                tracks: p.tracks.clone(),
                                is_album: false,
                            }),
                        _ => None,
                    })
                    .or_else(|| {
                        // search results, then the sidebar's own and liked playlists
                        self.search
                            .playlists
                            .iter()
                            .chain(&self.my_playlists)
                            .chain(&self.liked_playlists)
                            .find(|p| p.id_or_urn() == id_or_urn)
                            .map(|p| PlaylistDetail {
                                id: p.id,
                                id_or_urn: p.id_or_urn(),
                                title: p.title.clone(),
                                description: p.description.clone(),
                                artwork_url: p.artwork().map(str::to_string),
                                author: p
                                    .user
                                    .as_ref()
                                    .map(|u| u.username.clone())
                                    .unwrap_or_default(),
                                author_avatar: p.user.as_ref().and_then(|u| u.avatar_url.clone()),
                                author_id: p.user.as_ref().map(|u| u.id).filter(|id| *id != 0),
                                permalink_url: p.permalink_url.clone(),
                                track_count: p.track_count.map(|c| c as usize).unwrap_or(0),
                                tracks: p.tracks.clone().unwrap_or_default(),
                                is_album: p.is_album.unwrap_or(false),
                            })
                    });
                // A downloaded copy has every track; a preview may not.
                let wanted = if is_system_playlist(&id_or_urn) {
                    0
                } else {
                    parse_trailing_id(&id_or_urn)
                };
                let stored = self
                    .offline_store
                    .playlists
                    .iter()
                    .find(|p| p.id_or_urn == id_or_urn || (wanted != 0 && p.id == wanted))
                    .cloned();
                if self.settings.offline_mode {
                    self.loading_playlist = None;
                    self.current_playlist = stored.or(preview);
                    if self.current_playlist.is_none() {
                        self.show_toast("This playlist isn't downloaded", ToastKind::Info);
                    }
                    return Task::none();
                }
                // The preview's header shows at once; its (partial) track
                // list waits for the full load. Never the last playlist's.
                self.current_playlist = preview.or(stored);
                self.loading_playlist = Some(id_or_urn.clone());
                self.page_error = None;
                let cid = self.cid();
                let req = id_or_urn.clone();
                Task::perform(fetch_playlist_detail(id_or_urn, cid), move |r| {
                    Message::PlaylistLoaded(req.clone(), r.map_err(|e| e.to_string()))
                })
            }
            Message::PlaylistLoaded(req, _)
                if self.loading_playlist.as_deref() != Some(req.as_str()) =>
            {
                Task::none()
            }
            Message::PlaylistLoaded(_, Ok(detail)) => {
                self.loading_playlist = None;
                if let Some(avatar) = station_avatar(&detail) {
                    let mut changed = false;
                    for r in self
                        .library_radios
                        .iter_mut()
                        .filter(|r| r.urn == detail.id_or_urn)
                    {
                        changed |= r.artwork_url.as_deref() != Some(avatar.as_str());
                        r.artwork_url = Some(avatar.clone());
                    }
                    if changed {
                        self.store_library_items();
                    }
                }
                self.current_playlist = Some(detail);
                let urls = artwork_urls(
                    &self.home,
                    self.current_playlist.as_ref(),
                    &self.search,
                    self.state.me.as_ref(),
                );
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::PlaylistLoaded(req, Err(e)) => {
                self.loading_playlist = None;
                crate::log!("playlist FAILED: {e}");
                self.page_error = Some((
                    Tab::Playlist,
                    format!("Couldn't load this playlist: {e}"),
                    Box::new(Message::OpenPlaylist(req)),
                ));
                Task::none()
            }
            Message::NavBack => match self.nav_history.pop() {
                Some(prev) => {
                    self.nav_future.push(self.current_route());
                    self.restore_route(prev)
                }
                None => Task::none(),
            },
            Message::NavForward => match self.nav_future.pop() {
                Some(next) => {
                    self.nav_history.push(self.current_route());
                    self.restore_route(next)
                }
                None => Task::none(),
            },
            Message::ArtistClicked(name) => {
                // one lookup per name at a time; say it's on its way (it can
                // take a few searches)
                if self.artist_lookup.as_ref().is_some_and(|(_, n)| *n == name) {
                    return Task::none();
                }
                self.artist_lookup_gen += 1;
                let gen = self.artist_lookup_gen;
                self.artist_lookup = Some((gen, name.clone()));
                self.show_toast(format!("Finding {}…", trunc(&name, 32)), ToastKind::Info);
                Task::perform(lookup_artist_profile(name), move |r| {
                    Message::ArtistProfileFound(gen, r.map_err(|e| e.to_string()))
                })
            }
            // a lookup the user has since moved on from (or repeated)
            Message::ArtistProfileFound(gen, _)
                if self.artist_lookup.as_ref().map(|(g, _)| *g) != Some(gen) =>
            {
                Task::none()
            }
            Message::ArtistProfileFound(_, Ok(Some(id))) => {
                self.artist_lookup = None;
                self.toast = None;
                self.update(Message::OpenProfile(id))
            }
            Message::ArtistProfileFound(_, Ok(None)) => {
                self.artist_lookup = None;
                self.show_toast("Artist profile not found", ToastKind::Info);
                Task::none()
            }
            Message::ArtistProfileFound(_, Err(e)) => {
                self.artist_lookup = None;
                crate::log!("artist profile lookup failed: {e}");
                self.show_toast("Couldn't open artist profile", ToastKind::Error);
                Task::none()
            }
            Message::Tab(t) => {
                if t == Tab::Home {
                    self.search_query.clear();
                }
                self.push_nav(Route::Tab(t.clone()));
                if t == Tab::Settings {
                    self.confirm_remove_downloads = false;
                    return self.measure_storage();
                }
                if t == Tab::Library
                    && self.state.authenticated
                    && self.library.is_empty()
                    && !self.settings.offline_mode
                {
                    self.library_loading = true;
                    return Task::perform(fetch_library(), |r| {
                        Message::LibraryLoaded(r.map_err(|e| e.to_string()))
                    });
                }
                Task::none()
            }
            Message::PlayTrack(id) => {
                // context queue: queue the whole list the row belongs to so
                // next/prev keep working
                let Some(context) = self.row_context(id) else {
                    // not on the open page any more: play it from the queue,
                    // or on its own
                    if let Some(idx) = self.queue.iter().position(|t| t.id == id) {
                        return self.play_index(idx);
                    }
                    return match self.find_track_anywhere(id) {
                        Some(track) => self.play_track(track),
                        None => Task::none(),
                    };
                };
                // Already queued from this very list, in its order (maybe with
                // tracks added after it): jump within the queue so those
                // survive. Only containing the same tracks isn't enough: a
                // playlist of liked songs would keep Liked Tracks' order.
                let same_list = self.queue.len() >= context.len()
                    && self.queue.iter().zip(&context).all(|(q, c)| q.id == c.id);
                if same_list {
                    if let Some(idx) = context.iter().position(|t| t.id == id) {
                        return self.play_index(idx);
                    }
                }
                let pos = context.iter().position(|t| t.id == id).unwrap_or(0);
                crate::log!(
                    "queue context set: {} tracks (start at {})",
                    context.len(),
                    pos + 1
                );
                let pos = self.set_queue(context, pos);
                self.play_index(pos)
            }
            Message::StreamReady(gen, Ok((id, src))) => {
                // A resolve for an earlier pick (they finish in any order)
                // must not replace the track chosen since.
                if gen != self.play_gen {
                    crate::log!("stream ready: track {id} is stale, dropping");
                    return Task::none();
                }
                crate::log!("stream ready: track {id}, {}", src.describe());
                if let Some(p) = &self.player {
                    let cmd = match src {
                        crate::api::StreamSource::Single(u) => PlayerCommand::PlaySingle {
                            track_id: Some(id),
                            url: u,
                            proxy: None,
                        },
                        crate::api::StreamSource::Chunks { init, chunks } => {
                            PlayerCommand::PlayChunks {
                                track_id: Some(id),
                                init,
                                chunks,
                                proxy: None,
                            }
                        }
                    };
                    p.send(cmd);
                    self.apply_pending_seek(id);
                    // Paused while it resolved: the play command starts the
                    // sink, so pause again (commands run in order).
                    if let (true, Some(p)) = (self.is_paused, &self.player) {
                        p.send(PlayerCommand::Pause);
                    }
                    self.awaiting_audio = Some(std::time::Instant::now());
                    if let Some(t) = self.queue.get(self.queue_pos).filter(|t| t.id == id) {
                        self.dur_ms = self.track_ms(t).unwrap_or(self.dur_ms);
                    }
                }
                // Pre-buffer next track in the queue right away for browser-like instant skip
                let next_idx = self.queue_pos + 1;
                if next_idx < self.queue.len() {
                    let next_track = self.queue[next_idx].clone();
                    if self.prefetched_stream.as_ref().map(|(id, _)| *id) != Some(next_track.id)
                        && self.prefetch_in_progress != Some(next_track.id)
                        && !self.settings.offline_mode
                        && !self.plays_on_youtube(&next_track)
                    {
                        self.prefetch_in_progress = Some(next_track.id);
                        let quality = self
                            .settings
                            .audio_quality
                            .clone()
                            .unwrap_or_else(|| "hls".into());
                        return Task::perform(
                            resolve_stream_from_track(next_track, quality),
                            |r| Message::StreamPrefetched(r.map_err(|e| e.to_string())),
                        );
                    }
                }
                Task::none()
            }
            Message::CachedAudioReady(gen, id, path) => {
                if gen != self.play_gen {
                    crate::log!("cached audio ready: track {id} is stale, dropping");
                    return Task::none();
                }
                crate::log!("cached audio ready: track {id}, path {}", path.display());
                if let Some(p) = &self.player {
                    p.send(PlayerCommand::PlayCached { track_id: id, path });
                    self.apply_pending_seek(id);
                    if let (true, Some(p)) = (self.is_paused, &self.player) {
                        p.send(PlayerCommand::Pause);
                    }
                    self.awaiting_audio = Some(std::time::Instant::now());
                    if let Some(t) = self.queue.get(self.queue_pos).filter(|t| t.id == id) {
                        self.dur_ms = self.track_ms(t).unwrap_or(self.dur_ms);
                    }
                }
                let next_idx = self.queue_pos + 1;
                if next_idx < self.queue.len() {
                    let next_track = self.queue[next_idx].clone();
                    if self.prefetched_stream.as_ref().map(|(id, _)| *id) != Some(next_track.id)
                        && self.prefetch_in_progress != Some(next_track.id)
                        && !self.settings.offline_mode
                        && !self.plays_on_youtube(&next_track)
                    {
                        self.prefetch_in_progress = Some(next_track.id);
                        let quality = self
                            .settings
                            .audio_quality
                            .clone()
                            .unwrap_or_else(|| "hls".into());
                        return Task::perform(
                            resolve_stream_from_track(next_track, quality),
                            |r| Message::StreamPrefetched(r.map_err(|e| e.to_string())),
                        );
                    }
                }
                Task::none()
            }
            Message::StreamPrefetched(Ok((id, src))) => {
                crate::log!("prefetched stream ready: track {id}, {}", src.describe());
                self.prefetched_stream = Some((id, src));
                self.prefetched_at = Some(std::time::Instant::now());
                // a late result must not clear the guard of a newer prefetch
                if self.prefetch_in_progress == Some(id) {
                    self.prefetch_in_progress = None;
                }
                Task::none()
            }
            Message::StreamPrefetched(Err(e)) => {
                crate::log!("prefetch stream failed: {e}");
                // Keep prefetch_in_progress on the failed id: clearing it made
                // every Tick of the last 15s (endlessly while paused there)
                // resolve the same track again. play_index resolves it itself
                // if it's reached; a new next track prefetches as usual.
                Task::none()
            }
            Message::StreamReady(gen, Err(e)) => {
                if gen != self.play_gen {
                    crate::log!("stream FAILED for a stale pick, ignoring: {e}");
                    return Task::none();
                }
                crate::log!("stream FAILED: {e}");
                if self.settings.bypass_unavailable && !self.settings.offline_mode {
                    if let Some(id) = self.playing_id {
                        return self.start_bypass(gen, id, None);
                    }
                }
                if self.youtube_usable() {
                    if let Some(track) = self.queue.get(self.queue_pos).cloned() {
                        return self.start_youtube(gen, track, YtFallback::Skip);
                    }
                }
                self.skip_unplayable("no stream")
            }
            Message::BypassReady(gen, _, _) if gen != self.play_gen => Task::none(),
            Message::BypassReady(_, id, Ok((src, proxy))) => {
                crate::log!("bypass: playing track {id} through {proxy}");
                self.bypass_proxy = Some((id, proxy.clone()));
                if let Some(p) = &self.player {
                    let cmd = match src {
                        crate::api::StreamSource::Single(url) => PlayerCommand::PlaySingle {
                            track_id: Some(id),
                            url,
                            proxy: Some(proxy),
                        },
                        crate::api::StreamSource::Chunks { init, chunks } => {
                            PlayerCommand::PlayChunks {
                                track_id: Some(id),
                                init,
                                chunks,
                                proxy: Some(proxy),
                            }
                        }
                    };
                    p.send(cmd);
                    self.apply_pending_seek(id);
                    if let (true, Some(p)) = (self.is_paused, &self.player) {
                        p.send(PlayerCommand::Pause);
                    }
                    self.awaiting_audio = Some(std::time::Instant::now());
                }
                Task::none()
            }
            Message::BypassReady(gen, id, Err(e)) => {
                crate::log!("bypass FAILED for track {id}: {e}");
                if self.youtube_usable() {
                    if let Some(track) = self
                        .queue
                        .get(self.queue_pos)
                        .filter(|t| t.id == id)
                        .cloned()
                    {
                        return self.start_youtube(gen, track, YtFallback::Skip);
                    }
                }
                self.skip_unplayable("unavailable here, and no working proxy")
            }
            Message::YtFound(gen, _, _, _) if gen != self.play_gen => Task::none(),
            Message::YtFound(_, id, Ok(song), _) => {
                crate::log!("youtube: track {id} -> {} ({})", song.video_id, song.label);
                self.show_toast(format!("Playing from {}", song.label), ToastKind::Success);
                self.yt_video = Some((id, song.video_id.clone()));
                self.yt_ad = false;
                self.yt_restarted = false;
                self.dur_ms = song.duration_ms;
                let volume = if self.is_muted { 0.0 } else { self.volume };
                crate::yt_music::play(&song.video_id, volume, self.playback_speed);
                self.apply_pending_seek(id);
                if self.is_paused {
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::Pause);
                    }
                }
                self.awaiting_audio = Some(std::time::Instant::now());
                Task::none()
            }
            Message::YtFound(gen, id, Err(e), fallback) => {
                crate::log!("youtube: no song for track {id}: {e}");
                match fallback {
                    YtFallback::Skip => {
                        self.skip_unplayable("unavailable, and not on YouTube Music")
                    }
                    YtFallback::Preview => {
                        self.show_toast(
                            "Only a 30 s preview (no full version found)",
                            ToastKind::Info,
                        );
                        let Some(track) = self
                            .queue
                            .get(self.queue_pos)
                            .filter(|t| t.id == id)
                            .cloned()
                        else {
                            return Task::none();
                        };
                        let quality = self
                            .settings
                            .audio_quality
                            .clone()
                            .unwrap_or_else(|| "hls".into());
                        Task::perform(resolve_stream_from_track(track, quality), move |r| {
                            Message::StreamReady(gen, r.map_err(|e| e.to_string()))
                        })
                    }
                }
            }
            Message::Yt(ev) => self.on_youtube_event(ev),
            Message::YoutubeSignIn => {
                self.stop_youtube_track();
                self.show_toast("Opening Google sign-in…", ToastKind::Info);
                Task::perform(crate::yt_music::sign_in(), Message::YoutubeSignedIn)
            }
            Message::YoutubeSignedIn(Ok(())) => {
                self.yt_signed_in = true;
                self.show_toast("Signed in to YouTube Music", ToastKind::Success);
                Task::none()
            }
            Message::YoutubeSignedIn(Err(e)) => {
                self.yt_signed_in = crate::yt_music::signed_in();
                self.show_toast(
                    format!("YouTube Music sign-in didn't finish: {e}"),
                    ToastKind::Error,
                );
                Task::none()
            }
            Message::YoutubeSignOut => {
                self.stop_youtube_track();
                Task::perform(
                    async {
                        let _ = tokio::task::spawn_blocking(crate::yt_music::sign_out).await;
                    },
                    |_| Message::YoutubeSignedOut,
                )
            }
            Message::YoutubeSignedOut => {
                self.yt_signed_in = false;
                self.show_toast("Signed out of YouTube Music", ToastKind::Info);
                Task::none()
            }
            Message::SettingsYoutubeToggled(v) => {
                self.settings.youtube_music = v;
                self.settings.save();
                self.show_toast(
                    if v {
                        "Unlock through YouTube Music: on"
                    } else {
                        "Unlock through YouTube Music: off"
                    },
                    ToastKind::Info,
                );
                Task::none()
            }
            Message::WaveLoaded(Ok((id, samples))) => {
                crate::log!("waveform loaded: track {id}, {} samples", samples.len());
                let p = crate::config::cached_waveform_path(id);
                if self.keeps_locally(id) && !p.exists() && !samples.is_empty() {
                    write_atomic(&p, &samples);
                }
                if self.playing_id == Some(id) {
                    self.wave_bars = std::sync::Arc::new(compute_wave_bars(&samples));
                }
                Task::none()
            }
            Message::WaveLoaded(Err(e)) => {
                crate::log!("waveform FAILED: {e}");
                Task::none()
            }
            Message::WaveVisualResolved(id, Ok(url)) => {
                self.visual_urls.insert(id, url.clone());
                match url {
                    Some(url) if self.playing_id == Some(id) && !self.story_playback => {
                        Task::perform(fetch_wave_visual(url, self.keeps_locally(id)), move |r| {
                            Message::WaveVisualLoaded(id, r)
                        })
                    }
                    _ => Task::none(),
                }
            }
            Message::WaveVisualResolved(id, Err(e)) => {
                crate::log!("visual lookup FAILED: track {id}: {e}");
                Task::none()
            }
            Message::WaveVisualLoaded(id, Ok(pixels)) => {
                if self.playing_id != Some(id) || self.story_playback {
                    return Task::none();
                }
                crate::log!("visual loaded: track {id}, {pixels:?}");
                self.wave_visual_src = Some((id, pixels));
                self.bake_wave_visual()
            }
            Message::WaveVisualLoaded(id, Err(e)) => {
                crate::log!("visual FAILED: track {id}: {e}");
                Task::none()
            }
            Message::WaveVisualBaked(track_id, size, handle) => {
                self.wave_visual_baking = false;
                let current = self
                    .wave_visual_src
                    .as_ref()
                    .is_some_and(|(id, _)| *id == track_id);
                match handle {
                    Some(handle) if current => {
                        self.wave_visual = Some(WaveVisual {
                            track_id,
                            size,
                            handle,
                        });
                    }
                    // this picture doesn't bake at this size: don't loop on it
                    None if current && size == self.wave_visual_px() => return Task::none(),
                    _ => {}
                }
                // the window or the track changed while it baked
                self.bake_wave_visual()
            }
            Message::CommentsLoaded(Ok(page)) => {
                // a page for a track that's no longer playing (play_index
                // reset the paging for the new one)
                if self.playing_id != Some(page.track_id) {
                    return Task::none();
                }
                self.comments_loading = false;
                self.comments_pages += 1;
                crate::log!(
                    "comments loaded: track {}, +{} (total {}, more={})",
                    page.track_id,
                    page.comments.len(),
                    self.wave_comments.len() + page.comments.len(),
                    page.next.is_some()
                );
                for c in page.comments {
                    let ts_ms = c.timestamp.unwrap_or(0).max(0) as u64;
                    self.wave_comments.push(WaveComment {
                        ts_ms,
                        author: c.author().to_string(),
                        body: c.body,
                    });
                }
                // Every page, so the waveform shows all its comments (up to
                // COMMENTS_MAX_PAGES); the set is saved once it's complete.
                self.comments_next = page
                    .next
                    .filter(|n| !n.is_empty() && self.comments_pages < COMMENTS_MAX_PAGES);
                if self.comments_next.is_some() {
                    return self.update(Message::CommentsMore);
                }
                if !page.cached {
                    self.save_wave_comments(page.track_id);
                }
                Task::none()
            }
            Message::CommentsLoaded(Err(e)) => {
                self.comments_loading = false;
                crate::log!("comments FAILED: {e}");
                Task::none()
            }
            Message::CommentsMore => {
                if self.comments_loading {
                    return Task::none();
                }
                match (self.comments_next.take(), self.playing_id) {
                    (Some(next), Some(id)) => {
                        self.comments_loading = true;
                        Task::perform(fetch_comments_next(next, id), |r| {
                            Message::CommentsLoaded(r.map_err(|e| e.to_string()))
                        })
                    }
                    _ => Task::none(),
                }
            }
            Message::WaveSeek(frac) => {
                // Fix #8: Don't seek if nothing is playing — prevents spurious seeks
                // when user clicks the empty waveform placeholder before playing anything.
                if self.playing_id.is_none() {
                    return Task::none();
                }
                // frac < 0 → seek to last hovered position instead
                let frac = if frac < 0.0 {
                    self.hover_frac.unwrap_or(-1.0)
                } else {
                    frac
                };
                if frac >= 0.0 && self.dur_ms > 0 {
                    let frac = frac.clamp(0.0, 1.0);
                    let target = (self.dur_ms as f32 * frac) as u64;
                    crate::log!("seek to {}ms ({:.0}%)", target, frac * 100.0);
                    self.pos_ms = target;
                    if self.pending_seek_track == self.playing_id && self.pending_seek_ms.is_some()
                    {
                        self.pending_seek_ms = Some(target);
                    }
                    self.update_discord_rpc();
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SeekMs(target));
                    }
                }
                Task::none()
            }
            Message::WaveContextOpen(frac) => {
                if self.playing_id.is_some() && self.dur_ms > 0 {
                    self.wave_context_frac = Some(frac.clamp(0.0, 1.0));
                    self.wave_context_track = self.playing_id;
                    self.wave_context_comment.clear();
                }
                Task::none()
            }
            Message::WaveContextClose => {
                self.wave_context_frac = None;
                self.wave_context_track = None;
                self.wave_context_comment.clear();
                Task::none()
            }
            Message::WaveContextCommentInput(input) => {
                self.wave_context_comment = input;
                Task::none()
            }
            Message::WaveContextPostComment => {
                // the track the panel was opened on (it closes when the
                // track changes, so that's still the playing one)
                let Some(track_id) = self
                    .wave_context_track
                    .filter(|id| Some(*id) == self.playing_id)
                else {
                    return Task::none();
                };
                let Some(frac) = self.wave_context_frac else {
                    return Task::none();
                };
                let body = self.wave_context_comment.trim().to_string();
                if body.is_empty() {
                    return Task::none();
                }
                let ts_ms = (self.dur_ms as f32 * frac).round() as u64;
                Task::perform(do_post_comment(track_id, body.clone(), ts_ms), move |r| {
                    Message::WaveContextPosted(
                        track_id,
                        ts_ms,
                        body.clone(),
                        r.map_err(|e| e.to_string()),
                    )
                })
            }
            Message::WaveContextReact(codepoint) => {
                let Some(track_id) = self
                    .wave_context_track
                    .filter(|id| Some(*id) == self.playing_id)
                else {
                    return Task::none();
                };
                let Some(frac) = self.wave_context_frac else {
                    return Task::none();
                };
                let second = ((self.dur_ms as f32 * frac) / 1000.0).floor() as u64;
                let origin = Point::new(
                    self.window_size.width * frac,
                    self.window_size.height - PB_H - 8.0,
                );
                self.particles.extend(Particle::burst(&codepoint, origin));
                let excess = self.particles.len().saturating_sub(MAX_PARTICLES);
                self.particles.drain(..excess);
                self.wave_context_frac = None;
                self.wave_context_comment.clear();
                let posted = Some((track_id, second, codepoint.clone()));
                if self.last_posted_reaction == posted {
                    return Task::none();
                }
                self.last_posted_reaction = posted;
                let list = self.reactions.entry(second).or_default();
                let reaction = WaveReaction {
                    second,
                    codepoint: codepoint.clone(),
                };
                if let Some(existing) = list.last_mut() {
                    *existing = reaction;
                } else {
                    list.push(reaction);
                }
                Task::perform(
                    do_add_reaction(track_id, second, codepoint),
                    Message::ReactionPosted,
                )
            }
            Message::WaveContextPosted(track_id, ts_ms, body, Ok(())) => {
                if self.wave_context_track == Some(track_id) {
                    self.wave_context_frac = None;
                    self.wave_context_track = None;
                    self.wave_context_comment.clear();
                }
                self.show_toast("Comment posted!", ToastKind::Success);
                // yours joins the markers at once (and the saved set); a
                // refetch could still come back without it
                if self.playing_id == Some(track_id) {
                    let author = self
                        .state
                        .me
                        .as_ref()
                        .map(|m| m.username.clone())
                        .unwrap_or_else(|| "You".into());
                    self.wave_comments.push(WaveComment {
                        ts_ms,
                        author,
                        body,
                    });
                    if self.comments_next.is_none() && !self.comments_loading {
                        self.save_wave_comments(track_id);
                    }
                }
                Task::none()
            }
            Message::WaveContextPosted(_, _, _, Err(e)) => {
                self.show_toast(format!("Failed to post comment: {e}"), ToastKind::Error);
                Task::none()
            }
            Message::WaveHover(f) => {
                self.hover_frac = f;
                Task::none()
            }
            Message::ReactionPosted(Ok(())) => {
                self.show_toast("Reaction added!", ToastKind::Success);
                Task::none()
            }
            Message::ReactionPosted(Err(e)) => {
                crate::log!("reaction FAILED: {e}");
                self.action_failed(&e);
                Task::none()
            }
            Message::ReactionsPoll => self.poll_reactions(),
            Message::ReactionsLoaded(track_id, seconds, result) => {
                if self.playing_id != Some(track_id) {
                    return Task::none();
                }
                self.reactions_inflight = false;
                // asked-for seconds count as fetched even on failure: a
                // track without reactions (or a refused request) isn't
                // asked again every second
                self.reactions_fetched.extend(seconds);
                match result {
                    Ok(list) => {
                        for r in list {
                            self.reactions.entry(r.second).or_default().push(r);
                        }
                        Task::none()
                    }
                    Err(e) => {
                        crate::log!("reactions FAILED: {e}");
                        Task::none()
                    }
                }
            }
            Message::WaveWheel(delta_y) => {
                let now = std::time::Instant::now();
                if let Some(last) = self.last_wheel_skip {
                    if now.duration_since(last) < std::time::Duration::from_millis(300) {
                        return Task::none();
                    }
                }
                self.last_wheel_skip = Some(now);
                if delta_y > 0.0 {
                    self.update(Message::NextTrack)
                } else if delta_y < 0.0 {
                    self.update(Message::PrevTrack)
                } else {
                    Task::none()
                }
            }
            Message::InitWindowId(opt_id) => {
                self.window_id = opt_id;
                if let Some(id) = opt_id {
                    let scale = iced::window::get_scale_factor(id).map(Message::InitWindowScale);
                    if std::mem::take(&mut self.start_minimized) {
                        return Task::batch(vec![scale, iced::window::minimize(id, true)]);
                    }
                    return scale;
                }
                Task::none()
            }
            Message::InitWindowScale(scale) => {
                if scale > 0.1 {
                    self.window_scale = scale;
                }
                // the banner is baked in physical pixels
                self.bake_wave_visual()
            }
            Message::AnimTick => {
                // Smooth wave color animation: orange (1.0) <-> grey (0.0)
                let wave_target = if self.is_paused || self.playing_id.is_none() {
                    0.0
                } else {
                    1.0
                };
                if (self.wave_color_t - wave_target).abs() > 0.005 {
                    self.wave_color_t += (wave_target - self.wave_color_t) * 0.18;
                } else {
                    self.wave_color_t = wave_target;
                }

                // Smooth volume color animation: orange (1.0) <-> grey (0.0)
                let vol_target = if self.is_muted { 0.0 } else { 1.0 };
                if (self.vol_color_t - vol_target).abs() > 0.005 {
                    self.vol_color_t += (vol_target - self.vol_color_t) * 0.18;
                } else {
                    self.vol_color_t = vol_target;
                }

                // Spotify Toast auto-dismiss check (3.2 seconds total, with fade-out in last 400ms)
                if let Some(t) = &self.toast {
                    if t.created_at.elapsed() > std::time::Duration::from_millis(3200) {
                        self.toast = None;
                    }
                }

                if self.player.is_some()
                    && !self.is_paused
                    && self.last_pos_poll.elapsed() >= std::time::Duration::from_millis(80)
                {
                    self.last_pos_poll = std::time::Instant::now();
                    return self.update(Message::Tick);
                }
                self.pump_art()
            }
            Message::WindowDragStart => {
                if let Some(id) = self.window_id {
                    return iced::window::drag(id);
                }
                Task::none()
            }
            Message::WindowMinimize => {
                if let Some(id) = self.window_id {
                    return iced::window::minimize(id, true);
                }
                Task::none()
            }
            Message::WindowToggleMaximize => {
                if let Some(id) = self.window_id {
                    return iced::window::toggle_maximize(id);
                }
                Task::none()
            }
            Message::WindowClose => {
                self.save_playback_state(true);
                self.save_track_speeds();
                self.save_volume();
                flush_disk_writes();
                match self.window_id {
                    Some(id) => iced::window::close(id),
                    // the window never reported its id: close regardless
                    None => iced::exit(),
                }
            }
            Message::Tick => {
                if let Some(t) = &self.toast {
                    if t.created_at.elapsed() > std::time::Duration::from_secs(4) {
                        self.toast = None;
                    }
                }
                // (a YouTube Music track's position comes with its events)
                let restoring_saved_position =
                    self.pending_seek_ms.is_some() && self.pending_seek_track == self.playing_id;
                if !crate::yt_music::is_active() && !restoring_saved_position {
                    if let Some(pos) = self
                        .player
                        .as_ref()
                        .map(|p| p.pos_ms.load(std::sync::atomic::Ordering::Relaxed))
                    {
                        self.pos_ms = pos;
                    }
                }
                self.save_playback_state(false);
                self.save_track_speeds();
                self.save_volume();
                self.float_reactions();
                if let Some(p) = &self.player {
                    // Audio moving means the track really plays. Still silent
                    // long after the play command means the player failed it
                    // (decode error, no data), which it doesn't report: skip
                    // it rather than show it playing over silence.
                    if let Some(since) = self.awaiting_audio {
                        if self.pos_ms > 0 {
                            self.awaiting_audio = None;
                            self.play_failures = 0;
                        } else if !self.is_paused
                            && crate::yt_music::is_active()
                            && !self.yt_restarted
                            && since.elapsed() >= std::time::Duration::from_millis(YT_STALL_MS)
                        {
                            crate::log!("ui: YouTube Music player silent, restarting it");
                            self.yt_restarted = true;
                            crate::yt_music::restart();
                            self.awaiting_audio = Some(std::time::Instant::now());
                        } else if !self.is_paused
                            && since.elapsed() >= std::time::Duration::from_millis(STALL_MS)
                        {
                            // a bypass proxy that stopped delivering: next one
                            let dead = self.bypass_proxy.clone().filter(|(id, _)| {
                                Some(*id) == self.playing_id && self.bypass_attempts < 3
                            });
                            if let Some((id, proxy)) = dead {
                                crate::log!("ui: bypass proxy {proxy} stalled, switching");
                                let gen = self.play_gen;
                                return self.start_bypass(gen, id, Some(proxy));
                            }
                            crate::log!(
                                "ui: no audio {STALL_MS}ms after the play command, skipping"
                            );
                            return self.skip_unplayable("no audio");
                        }
                    }
                    if !self.is_paused
                        && self.pos_ms >= 1000
                        && p.ended.swap(false, std::sync::atomic::Ordering::SeqCst)
                    {
                        // Nothing is loaded any more (see PlayerToggle).
                        self.at_end = true;
                        crate::log!(
                            "ui: track ended naturally (pos_ms={}, dur_ms={}), advancing to next",
                            self.pos_ms,
                            self.dur_ms
                        );
                        // auto-advance to next track according to repeat/shuffle mode
                        return self.update(Message::TrackEnded);
                    }
                    // Crossfade: the next track starts this long before the
                    // end, the two overlapping (not for short tracks, Go+
                    // previews or YouTube Music)
                    let fade = u64::from(self.settings.crossfade_secs) * 1000;
                    if fade > 0
                        && !self.is_paused
                        && !self.story_playback
                        && !crate::yt_music::is_active()
                        && self.dur_ms > 2 * fade + 10_000
                        && self.pos_ms > 0
                        && self.pos_ms + fade >= self.dur_ms
                        && self.crossfaded_from != self.playing_id
                        && self.repeat != RepeatMode::One
                        && (self.queue_pos + 1 < self.queue.len() || self.repeat == RepeatMode::All)
                    {
                        self.crossfaded_from = self.playing_id;
                        self.crossfade_next = true;
                        let task = self.update(Message::NextTrack);
                        // not taken by a play_index (the queue ended): the next
                        // play stops the old track as usual
                        self.crossfade_next = false;
                        return task;
                    }
                    // Prefetch check: if track is nearing completion (last 15s) and next track hasn't been prefetched
                    let next_idx = self.queue_pos + 1;
                    if next_idx < self.queue.len()
                        && self.dur_ms > 0
                        && self.dur_ms.saturating_sub(self.pos_ms) <= 15000
                    {
                        let next_track = self.queue[next_idx].clone();
                        if self.prefetched_stream.as_ref().map(|(id, _)| *id) != Some(next_track.id)
                            && self.prefetch_in_progress != Some(next_track.id)
                            && !self.settings.offline_mode
                            && !self.plays_on_youtube(&next_track)
                        {
                            self.prefetch_in_progress = Some(next_track.id);
                            let quality = self
                                .settings
                                .audio_quality
                                .clone()
                                .unwrap_or_else(|| "hls".into());
                            return Task::perform(
                                resolve_stream_from_track(next_track, quality),
                                |r| Message::StreamPrefetched(r.map_err(|e| e.to_string())),
                            );
                        }
                    }
                }
                self.pump_art()
            }
            Message::LikeCurrent => {
                if let Some(track_id) = self.playing_id {
                    self.update(Message::LikeTrack(track_id))
                } else {
                    Task::none()
                }
            }
            Message::LikeTrack(track_id) => {
                if !self.state.authenticated {
                    self.show_toast("Log in first to like tracks", ToastKind::Error);
                    return Task::none();
                }
                let was_liked = self.liked_ids.contains(&track_id);
                crate::log!(
                    "like toggle: track {track_id} -> {}",
                    if was_liked { "unlike" } else { "like" }
                );
                // Liked Tracks follows at once (it's otherwise only rebuilt on
                // a full reload); a failure puts things back in TrackLikeDone
                let removed = self
                    .library
                    .iter()
                    .find(|t| t.id == track_id && was_liked)
                    .cloned()
                    .map(Box::new);
                self.set_liked(track_id, !was_liked, None);
                if was_liked {
                    self.show_toast("Removed from liked tracks", ToastKind::Info);
                } else {
                    self.show_toast("Added to liked tracks", ToastKind::Success);
                }
                let liked_now = !was_liked;
                Task::perform(do_like(track_id, was_liked), move |r| {
                    Message::TrackLikeDone(
                        track_id,
                        liked_now,
                        removed.clone(),
                        r.map_err(|e| e.to_string()),
                    )
                })
            }
            Message::TrackLikeDone(track_id, liked_now, removed, Err(e)) => {
                crate::log!("like FAILED: {e}");
                // SoundCloud didn't take it: the heart and the list go back
                self.set_liked(track_id, !liked_now, removed.map(|t| *t));
                self.action_failed(&e);
                Task::none()
            }
            Message::TrackLikeDone(_, _, _, Ok(())) => {
                crate::log!("like done");
                Task::none()
            }
            Message::ToggleDone(toggle, result) => {
                match &result {
                    Ok(()) => crate::log!("{toggle:?} done"),
                    Err(e) => crate::log!("{toggle:?} FAILED: {e}"),
                }
                if let Err(e) = &result {
                    // put it back the way it was
                    let set = |ids: &mut std::collections::HashSet<i64>, id, on| {
                        if on {
                            ids.insert(id);
                        } else {
                            ids.remove(&id);
                        }
                    };
                    match toggle {
                        Toggle::Repost(id, was) => set(&mut self.reposted_ids, id, was),
                        Toggle::Follow(uid, was) => {
                            set(&mut self.my_following_ids, uid, was);
                            if let Some(p) = self.profile.as_mut().filter(|p| p.id == uid) {
                                p.following = was;
                            }
                        }
                        Toggle::PlaylistLike(id, was) => set(&mut self.liked_playlist_ids, id, was),
                    }
                    self.action_failed(e);
                }
                // Your Library's lists as SoundCloud has them now: the entry
                // added or taken out at once gets its real data, or comes
                // back after a failure
                match toggle {
                    Toggle::Repost(..) => Task::none(),
                    Toggle::Follow(..) => self.fetch_followings_task(),
                    Toggle::PlaylistLike(..) => Task::perform(fetch_liked_playlists(), |r| {
                        Message::LikedPlaylistsLoaded(r.map_err(|e| e.to_string()))
                    }),
                }
            }
            Message::PlayerPause => {
                if self.is_paused || self.playing_id.is_none() {
                    Task::none()
                } else {
                    self.update(Message::PlayerToggle)
                }
            }
            Message::PlayerToggle => {
                // After the queue ran out or a failed track the sink is empty and Resume would change
                // nothing: go on to the next track, or replay the last one
                // when there is none.
                if self.at_end && self.is_paused {
                    return if self.queue_pos + 1 < self.queue.len()
                        || self.repeat != RepeatMode::Off
                    {
                        self.update(Message::TrackEnded)
                    } else {
                        self.play_index(self.queue_pos)
                    };
                }
                self.is_paused = !self.is_paused;
                if !self.is_paused && self.awaiting_audio.is_some() {
                    // time spent paused doesn't count towards the stall check
                    self.awaiting_audio = Some(std::time::Instant::now());
                }
                self.update_discord_rpc();
                if let Some(p) = &self.player {
                    p.send(if self.is_paused {
                        PlayerCommand::Pause
                    } else {
                        PlayerCommand::Resume
                    });
                }
                Task::none()
            }
            Message::PlayerVolume(v) => {
                let v = v.clamp(0.0, 1.0);
                self.volume = v;
                if v > 0.005 {
                    self.prev_volume = v;
                    self.is_muted = false;
                } else {
                    self.is_muted = true;
                }
                if let Some(p) = &self.player {
                    p.send(PlayerCommand::SetVolume(self.volume));
                }
                // saved by VolumeCommit on release: a drag sends one of these
                // per mouse move
                Task::none()
            }
            Message::VolumeCommit => {
                self.settings.volume = Some(self.volume);
                self.settings.save();
                Task::none()
            }
            // a story plays alone, under a hidden player bar: its one-track
            // queue isn't the one to reorder
            Message::ToggleShuffle if self.story_playback => Task::none(),
            Message::ToggleShuffle => {
                self.shuffle = !self.shuffle;
                if self.shuffle {
                    self.shuffle_upcoming();
                } else {
                    self.unshuffle();
                }
                let s = if self.shuffle {
                    "Shuffle: ON"
                } else {
                    "Shuffle: OFF"
                };
                self.show_toast(s, ToastKind::Info);
                Task::none()
            }
            Message::ToggleRepeat => {
                self.repeat = match self.repeat {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                let s = match self.repeat {
                    RepeatMode::Off => "Repeat: OFF",
                    RepeatMode::All => "Repeat: ALL",
                    RepeatMode::One => "Repeat: ONE",
                };
                self.show_toast(s, ToastKind::Info);
                Task::none()
            }
            Message::ToggleMute => {
                if self.is_muted {
                    self.is_muted = false;
                    self.volume = if self.prev_volume > 0.02 {
                        self.prev_volume
                    } else {
                        0.5
                    };
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SetVolume(self.volume));
                    }
                    self.show_toast(
                        format!("Unmuted ({:.0}%)", self.volume * 100.0),
                        ToastKind::Info,
                    );
                } else {
                    self.is_muted = true;
                    self.prev_volume = self.volume.max(0.05);
                    self.volume = 0.0;
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SetVolume(0.0));
                    }
                    self.show_toast("Muted", ToastKind::Info);
                }
                self.settings.volume = Some(if self.is_muted {
                    self.prev_volume
                } else {
                    self.volume
                });
                self.settings.save();
                Task::none()
            }
            Message::SeekRelative(delta) if self.active_story_index.is_some() => {
                self.update(if delta < 0 {
                    Message::PrevStory
                } else {
                    Message::NextStory
                })
            }
            Message::SeekRelative(delta) => {
                if self.dur_ms > 0 {
                    let cur = self.pos_ms as i64;
                    let target = (cur + delta).clamp(0, self.dur_ms as i64) as u64;
                    self.pos_ms = target;
                    if self.pending_seek_track == self.playing_id && self.pending_seek_ms.is_some()
                    {
                        self.pending_seek_ms = Some(target);
                    }
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SeekMs(target));
                    }
                    self.update_discord_rpc();
                }
                Task::none()
            }
            Message::VolumeRelative(delta) => {
                // Turning it down while muted changes nothing, and must not
                // save 0 over the level ToggleMute stored.
                if self.is_muted && delta <= 0.0 {
                    return Task::none();
                }
                let mut v = self.volume + delta;
                if self.is_muted && delta > 0.0 {
                    v = (self.prev_volume + delta).clamp(0.01, 1.0);
                    self.is_muted = false;
                }
                self.volume = v.clamp(0.0, 1.0);
                if self.volume > 0.005 {
                    self.prev_volume = self.volume;
                    self.is_muted = false;
                } else {
                    self.is_muted = true;
                }
                if let Some(p) = &self.player {
                    p.send(PlayerCommand::SetVolume(self.volume));
                }
                // written on the next Tick, not once per notch
                self.settings.volume = Some(self.volume);
                self.volume_dirty = true;
                Task::none()
            }
            Message::CloseModals => {
                if self.delete_playlist_confirm.is_some() {
                    self.delete_playlist_confirm = None;
                } else if self.image_viewer.is_some() {
                    self.image_viewer = None;
                } else if self.active_story_index.is_some() {
                    return self.close_story();
                } else if self.wave_context_frac.is_some() {
                    self.wave_context_frac = None;
                    self.wave_context_comment.clear();
                } else if self.show_user_menu {
                    self.show_user_menu = false;
                } else if self.add_popover.is_some() {
                    self.add_popover = None;
                } else if self.speed_popup.is_some() {
                    self.speed_popup = None;
                } else if self.toast.is_some() {
                    self.toast = None;
                } else if self.action_menu.is_some() {
                    self.action_menu = None;
                    self.menu_anchor = None;
                } else {
                    self.inspector_track = None;
                }
                Task::none()
            }
            Message::ToggleUserMenu => {
                self.show_user_menu = !self.show_user_menu;
                Task::none()
            }
            Message::CloseUserMenu => {
                self.show_user_menu = false;
                Task::none()
            }
            Message::UserMenuAccount => {
                self.show_user_menu = false;
                if let Some(me) = &self.state.me {
                    let uid = me.id;
                    self.update(Message::OpenProfile(uid))
                } else {
                    self.push_nav(Route::Tab(Tab::Settings));
                    Task::none()
                }
            }
            Message::UserMenuSettings => {
                self.show_user_menu = false;
                self.confirm_remove_downloads = false;
                self.push_nav(Route::Tab(Tab::Settings));
                self.measure_storage()
            }
            Message::DismissToast => {
                self.toast = None;
                Task::none()
            }
            Message::DownloadTrack(track) => {
                let (display_artist, display_title) =
                    track.display_artist_and_title(self.settings.prefer_artist_from_name);
                let name = format!("{} — {}", display_artist, display_title);
                self.show_toast(format!("Downloading: {}", name), ToastKind::Info);
                let cid = self.cid();
                let prefer = self.settings.prefer_artist_from_name;
                Task::perform(
                    download_track_to_disk(track.clone(), cid, prefer),
                    move |r| Message::DownloadDone(track.clone(), r),
                )
            }
            Message::DownloadDone(track, Ok(file)) => {
                // the export also fills the offline cache
                self.downloaded_track_ids.insert(track.id);
                self.remember_offline_track(&track);
                self.save_offline_store();
                self.show_toast(
                    format!("Saved to Downloads/Wavify: {}", file),
                    ToastKind::Success,
                );
                if let Some(dir) = directories::UserDirs::new()
                    .and_then(|u| u.download_dir().map(|p| p.join("Wavify")))
                {
                    let _ = open::that(dir);
                }
                Task::none()
            }
            Message::DownloadDone(_, Err(e)) => {
                self.show_toast(format!("Download failed: {}", e), ToastKind::Error);
                Task::none()
            }
            Message::SetPlaybackSpeed(speed) => {
                self.playback_speed = speed.clamp(SPEED_MIN, SPEED_MAX);
                if let Some(p) = &self.player {
                    p.send(PlayerCommand::SetSpeed(self.playback_speed));
                }
                self.remember_speed();
                self.update_discord_rpc();
                let normal = (self.playback_speed - 1.0).abs() < 0.005;
                let label = if (self.playback_speed - 1.25).abs() < 0.005 {
                    "1.25x ⚡ Nightcore".to_string()
                } else if normal {
                    "1.0x (Normal)".to_string()
                } else {
                    format!("{:.2}x", self.playback_speed)
                };
                // the track keeps it (see remember_speed): say so
                let kept = if normal || self.playing_id.is_none() || self.story_playback {
                    ""
                } else {
                    " · saved for this track"
                };
                self.show_toast(format!("Playback speed: {label}{kept}"), ToastKind::Info);
                Task::none()
            }
            Message::CyclePlaybackSpeed => {
                // Presets in click order. From a wheel / slider value between
                // presets go to the next preset above it, so none is skipped.
                let speeds = [1.0f32, 1.15, 1.25, 1.5, 2.0, 0.75];
                let cur = self.playback_speed;
                let next = match speeds.iter().position(|&s| (s - cur).abs() < 0.005) {
                    Some(idx) => speeds[(idx + 1) % speeds.len()],
                    None => speeds
                        .iter()
                        .copied()
                        .filter(|&s| s > cur)
                        .min_by(|a, b| a.total_cmp(b))
                        .unwrap_or(1.0),
                };
                self.update(Message::SetPlaybackSpeed(next))
            }
            Message::TrackRepostToggle(track_id) => {
                if !self.state.authenticated {
                    self.show_toast("Log in first to repost tracks", ToastKind::Error);
                    return Task::none();
                }
                let was_reposted = self.reposted_ids.contains(&track_id);
                if was_reposted {
                    self.reposted_ids.remove(&track_id);
                    self.show_toast("Removed repost", ToastKind::Info);
                } else {
                    self.reposted_ids.insert(track_id);
                    self.show_toast("Reposted to your profile", ToastKind::Success);
                }
                Task::perform(do_repost(track_id, was_reposted), move |r| {
                    Message::ToggleDone(
                        Toggle::Repost(track_id, was_reposted),
                        r.map_err(|e| format!("repost: {e}")),
                    )
                })
            }
            // a story plays alone in its own queue: Next/Prev (keys, media
            // keys) step through the stories instead
            Message::NextTrack if self.active_story_index.is_some() => {
                self.update(Message::NextStory)
            }
            Message::PrevTrack if self.active_story_index.is_some() => {
                self.update(Message::PrevStory)
            }
            Message::NextTrack => {
                if self.queue.is_empty() {
                    return Task::none();
                }
                // in shuffle too: the queue is in the shuffled order
                let next = if self.queue_pos + 1 < self.queue.len() {
                    Some(self.queue_pos + 1)
                } else if self.repeat == RepeatMode::All {
                    if self.shuffle {
                        self.reshuffle_round();
                    }
                    Some(0)
                } else {
                    None
                };
                if let Some(next) = next {
                    crate::log!("next track (auto or manual)");
                    self.play_index(next)
                } else if self.settings.offline_mode || !self.settings.autoplay {
                    // no related tracks offline, or with Autoplay off: the
                    // queue just ends
                    if self.at_end {
                        self.is_paused = true;
                        self.update_discord_rpc();
                    }
                    Task::none()
                } else if let Some(id) = self.playing_id {
                    // queue exhausted: fetch related tracks and keep going.
                    // One fetch per seed: a second Next while it runs would
                    // jump past the first batch.
                    if self.related_seed == Some(id) {
                        return Task::none();
                    }
                    crate::log!("next: queue ended, fetching related tracks…");
                    self.related_seed = Some(id);
                    Task::perform(fetch_related(id), move |r| match r {
                        Ok(x) => Message::RelatedLoaded(id, Ok(x)),
                        Err(e) => Message::RelatedLoaded(id, Err(e.to_string())),
                    })
                } else {
                    Task::none()
                }
            }
            Message::TrackEnded => {
                if self.story_playback {
                    return self.close_story();
                }
                if self.repeat == RepeatMode::One {
                    return self.play_index(self.queue_pos);
                }
                self.update(Message::NextTrack)
            }
            Message::RelatedLoaded(seed, result) => {
                // only the latest request counts
                if self.related_seed != Some(seed) {
                    return Task::none();
                }
                self.related_seed = None;
                // ...and only while its seed is still on: after the user
                // picked another track these belong to a song that's gone.
                if self.playing_id != Some(seed) {
                    crate::log!("related for track {seed} arrived after another pick, dropping");
                    return Task::none();
                }
                let tracks = result.unwrap_or_else(|e| {
                    crate::log!("related FAILED: {e}");
                    Vec::new()
                });
                let mut fresh: Vec<Track> = tracks
                    .into_iter()
                    .filter(|t| t.id != 0 && self.queue.iter().all(|q| q.id != t.id))
                    .take(20)
                    .collect();
                if self.shuffle {
                    use rand::seq::SliceRandom;
                    fresh.shuffle(&mut rand::thread_rng());
                    self.spread_artists(&mut fresh);
                }
                crate::log!(
                    "related loaded: {} new tracks appended to queue",
                    fresh.len()
                );
                if !fresh.is_empty() {
                    let urls = extract_tracks_artwork(&fresh);
                    let art_task = Task::perform(fetch_artwork(urls), Message::ArtworkLoaded);
                    self.queue.extend(fresh);
                    // the first related track, or one queued while it loaded
                    let play_task = self.play_index(self.queue_pos + 1);
                    return Task::batch(vec![play_task, art_task]);
                }
                crate::log!("related: nothing new, stopping");
                if self.at_end {
                    // the last track has finished: show it stopped, not playing
                    self.is_paused = true;
                    self.update_discord_rpc();
                }
                Task::none()
            }
            Message::OpenProfile(user_id) => {
                if self.settings.offline_mode {
                    self.show_toast("Artist pages aren't available offline", ToastKind::Info);
                    return Task::none();
                }
                self.push_nav(Route::Profile(user_id));
                self.profile = None;
                self.loading_profile = Some(user_id);
                self.page_error = None;
                crate::log!("open profile: user {user_id}");
                Task::perform(fetch_profile(user_id), move |r| {
                    Message::ProfileLoaded(user_id, r.map_err(|e| e.to_string()))
                })
            }
            Message::ProfileLoaded(uid, _) if self.loading_profile != Some(uid) => Task::none(),
            Message::ProfileLoaded(_, Ok(detail)) => {
                self.loading_profile = None;
                crate::log!(
                    "profile loaded: @{} ({} followers, {} tracks, following={})",
                    detail.username,
                    detail.followers,
                    detail.track_count,
                    detail.following
                );
                let mut urls: Vec<String> = Vec::new();
                if let Some(a) = &detail.avatar_url {
                    urls.push(a.clone());
                }
                for t in detail.top_tracks.iter().chain(detail.reposts.iter()) {
                    if let Some(u) = &t.artwork_url {
                        if !urls.contains(u) {
                            urls.push(u.clone());
                        }
                    }
                    if let Some(u) = t.user.as_ref().and_then(|usr| usr.avatar_url.as_ref()) {
                        if !urls.contains(u) {
                            urls.push(u.clone());
                        }
                    }
                }
                for pl in &detail.playlists {
                    if let Some(u) = pl.artwork_or_avatar() {
                        if !urls.contains(&u.to_string()) {
                            urls.push(u.to_string());
                        }
                    }
                }
                for rel in &detail.related {
                    if let Some(u) = &rel.avatar_url {
                        if !urls.contains(u) {
                            urls.push(u.clone());
                        }
                    }
                }
                let banner = detail.banner_url.clone();
                let uid = detail.id;
                if self
                    .profile_banner
                    .as_ref()
                    .is_some_and(|(id, _)| *id != uid)
                {
                    self.profile_banner = None;
                }
                self.profile = Some(detail.clone());
                let art = Task::perform(fetch_artwork(urls), Message::ArtworkLoaded);
                match banner {
                    Some(url) => Task::batch(vec![
                        art,
                        Task::perform(fetch_banner(url), move |r| {
                            Message::ProfileBannerLoaded(uid, r)
                        }),
                    ]),
                    None => art,
                }
            }
            Message::ProfileBannerLoaded(uid, Ok(handle)) => {
                if self.profile.as_ref().is_some_and(|p| p.id == uid) {
                    self.profile_banner = Some((uid, handle));
                }
                Task::none()
            }
            Message::ProfileBannerLoaded(uid, Err(e)) => {
                crate::log!("profile banner of {uid}: {e}");
                Task::none()
            }
            Message::ProfileLoaded(uid, Err(e)) => {
                self.loading_profile = None;
                crate::log!("profile FAILED: {e}");
                self.page_error = Some((
                    Tab::Profile,
                    format!("Couldn't load this profile: {e}"),
                    Box::new(Message::OpenProfile(uid)),
                ));
                Task::none()
            }
            Message::ProfileSubTabSelected(tab) => {
                if let Some(p) = self.profile.as_mut() {
                    p.active_tab = tab;
                    let uid = p.id;
                    match tab {
                        ProfileSubTab::Overview => Task::none(),
                        ProfileSubTab::Tracks => {
                            if p.all_tracks.is_empty()
                                && self.profile_tabs_loading.insert((uid, tab))
                            {
                                Task::perform(fetch_profile_tracks(uid), move |r| {
                                    Message::ProfileTracksLoaded(
                                        uid,
                                        r.map(|(_, x)| x).map_err(|e| e.to_string()),
                                    )
                                })
                            } else {
                                Task::none()
                            }
                        }
                        ProfileSubTab::Playlists => Task::none(),
                        ProfileSubTab::Likes => {
                            if p.likes.is_empty() && self.profile_tabs_loading.insert((uid, tab)) {
                                Task::perform(fetch_profile_likes(uid), move |r| {
                                    Message::ProfileLikesLoaded(
                                        uid,
                                        r.map(|(_, x)| x).map_err(|e| e.to_string()),
                                    )
                                })
                            } else {
                                Task::none()
                            }
                        }
                        ProfileSubTab::Followers => {
                            if p.followers_list.is_empty()
                                && self.profile_tabs_loading.insert((uid, tab))
                            {
                                Task::perform(fetch_profile_followers(uid), move |r| {
                                    Message::ProfileFollowersLoaded(
                                        uid,
                                        r.map(|(_, x)| x).map_err(|e| e.to_string()),
                                    )
                                })
                            } else {
                                Task::none()
                            }
                        }
                        ProfileSubTab::Following => {
                            if p.followings_list.is_empty()
                                && self.profile_tabs_loading.insert((uid, tab))
                            {
                                Task::perform(fetch_profile_followings(uid), move |r| {
                                    Message::ProfileFollowingsLoaded(
                                        uid,
                                        r.map(|(_, x)| x).map_err(|e| e.to_string()),
                                    )
                                })
                            } else {
                                Task::none()
                            }
                        }
                    }
                } else {
                    Task::none()
                }
            }
            Message::ProfileFollowersLoaded(uid, Ok(users)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Followers));
                if let Some(p) = self.profile.as_mut() {
                    if p.id == uid {
                        p.followers_list = users.clone();
                    }
                }
                let urls: Vec<String> = users.into_iter().filter_map(|u| u.avatar_url).collect();
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::ProfileFollowersLoaded(uid, Err(e)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Followers));
                crate::log!("profile followers error: {e}");
                Task::none()
            }
            Message::ProfileFollowingsLoaded(uid, Ok(users)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Following));
                if let Some(p) = self.profile.as_mut() {
                    if p.id == uid {
                        p.followings_list = users.clone();
                    }
                }
                let urls: Vec<String> = users.into_iter().filter_map(|u| u.avatar_url).collect();
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::ProfileFollowingsLoaded(uid, Err(e)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Following));
                crate::log!("profile followings error: {e}");
                Task::none()
            }
            Message::ProfileLikesLoaded(uid, Ok(tracks)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Likes));
                if let Some(p) = self.profile.as_mut() {
                    if p.id == uid {
                        p.likes = tracks.clone();
                    }
                }
                let urls = extract_tracks_artwork(&tracks);
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::ProfileLikesLoaded(uid, Err(e)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Likes));
                crate::log!("profile likes error: {e}");
                Task::none()
            }
            Message::ProfileTracksLoaded(uid, Ok(tracks)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Tracks));
                if let Some(p) = self.profile.as_mut() {
                    if p.id == uid {
                        p.all_tracks = tracks.clone();
                    }
                }
                let urls = extract_tracks_artwork(&tracks);
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::ProfileTracksLoaded(uid, Err(e)) => {
                self.profile_tabs_loading
                    .remove(&(uid, ProfileSubTab::Tracks));
                crate::log!("profile tracks error: {e}");
                Task::none()
            }
            Message::FollowUserToggle(uid, currently_following) => {
                if !self.state.authenticated {
                    self.login_error = Some("Log in first to follow users".into());
                    return Task::none();
                }
                if currently_following {
                    self.my_following_ids.remove(&uid);
                } else {
                    self.my_following_ids.insert(uid);
                }
                if let Some(p) = self.profile.as_mut() {
                    if p.id == uid {
                        p.following = !currently_following;
                    }
                }
                let user = self.known_user(uid);
                self.library_follow(uid, !currently_following, user);
                Task::perform(do_follow(uid, currently_following), move |r| {
                    Message::ToggleDone(
                        Toggle::Follow(uid, currently_following),
                        r.map_err(|e| e.to_string()),
                    )
                })
            }
            Message::OpenTrackInspector(track) => {
                let tid = track.id;
                self.inspector_track = Some(track);
                self.inspector_tab = InspectorTab::Comments;
                // 3 async tasks will run; spinner clears when all 3 complete or fail
                self.inspector_tasks_pending = 3;
                self.inspector_favoriters.clear();
                self.inspector_reposters.clear();
                self.inspector_comments.clear();
                Task::batch(vec![
                    Task::perform(fetch_inspector_comments(tid), move |r| {
                        Message::InspectorCommentsLoaded(
                            tid,
                            r.map(|(_, x)| x).map_err(|e| e.to_string()),
                        )
                    }),
                    Task::perform(fetch_track_favoriters(tid), move |r| {
                        Message::TrackFavoritersLoaded(
                            tid,
                            r.map(|(_, x)| x).map_err(|e| e.to_string()),
                        )
                    }),
                    Task::perform(fetch_track_reposters(tid), move |r| {
                        Message::TrackRepostersLoaded(
                            tid,
                            r.map(|(_, x)| x).map_err(|e| e.to_string()),
                        )
                    }),
                ])
            }
            Message::CloseTrackInspector => {
                self.inspector_track = None;
                Task::none()
            }
            Message::InspectorTabSelected(tab) => {
                self.inspector_tab = tab;
                Task::none()
            }
            // replies for a track the inspector showed before: its spinner
            // and lists are the open track's
            Message::TrackFavoritersLoaded(tid, _)
            | Message::TrackRepostersLoaded(tid, _)
            | Message::InspectorCommentsLoaded(tid, _)
                if self.inspector_track.as_ref().map(|t| t.id) != Some(tid) =>
            {
                Task::none()
            }
            Message::TrackFavoritersLoaded(_, Ok(users)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                self.inspector_favoriters = users.clone();
                let urls: Vec<String> = users.into_iter().filter_map(|u| u.avatar_url).collect();
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::TrackFavoritersLoaded(_, Err(e)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                crate::log!("track favoriters error: {e}");
                Task::none()
            }
            Message::TrackRepostersLoaded(_, Ok(users)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                self.inspector_reposters = users.clone();
                let urls: Vec<String> = users.into_iter().filter_map(|u| u.avatar_url).collect();
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::TrackRepostersLoaded(_, Err(e)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                crate::log!("track reposters error: {e}");
                Task::none()
            }
            Message::InspectorCommentsLoaded(_, Ok(comments)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                self.inspector_comments = comments.clone();
                let urls: Vec<String> = comments
                    .into_iter()
                    .filter_map(|c| c.user.and_then(|u| u.avatar_url))
                    .collect();
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::InspectorCommentsLoaded(_, Err(e)) => {
                self.inspector_tasks_pending = self.inspector_tasks_pending.saturating_sub(1);
                crate::log!("inspector comments error: {e}");
                Task::none()
            }
            Message::InspectorSeek(track_id, ms) => {
                if self.playing_id == Some(track_id) && !self.at_end && !self.story_playback {
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SeekMs(ms));
                    }
                    self.pos_ms = ms;
                    return Task::none();
                }
                // a comment on another track (or one that finished): play
                // that track from the comment's second
                let track = self
                    .inspector_track
                    .iter()
                    .chain(self.track_page.iter().map(|p| &p.track))
                    .find(|t| t.id == track_id)
                    .cloned()
                    .or_else(|| self.find_track_anywhere(track_id));
                let Some(track) = track else {
                    return Task::none();
                };
                self.story_playback = false;
                self.set_queue(vec![track], 0);
                self.pending_seek_ms = Some(ms);
                self.pending_seek_track = Some(track_id);
                self.play_index(0)
            }
            Message::OpenTrackPage(boxed_track) => {
                let track = *boxed_track;
                let tid = track.id;
                if self.track_page.as_ref().map(|page| page.track.id) != Some(tid) {
                    self.expanded_description_track = None;
                }
                self.push_nav(Route::Track(Box::new(track.clone())));
                self.track_page = Some(TrackDetailPage {
                    track: track.clone(),
                    related_tracks: vec![],
                    comments: vec![],
                    likers: vec![],
                    reposters: vec![],
                    active_tab: TrackSubTab::Related,
                    pending: [
                        TrackSubTab::Related,
                        TrackSubTab::Comments,
                        TrackSubTab::Likers,
                        TrackSubTab::Reposters,
                    ]
                    .into_iter()
                    .collect(),
                    comment_input: String::new(),
                });

                // Pre-fetch track artwork (t500x500) and artist avatar immediately
                let mut init_urls = Vec::new();
                if let Some(art) = track.artwork_or_avatar() {
                    init_urls.push(art.replace("-large.jpg", "-t500x500.jpg"));
                    init_urls.push(art.to_string());
                }
                if let Some(av) = track.user.as_ref().and_then(|u| u.avatar_url.as_ref()) {
                    init_urls.push(av.clone());
                }
                if self.settings.offline_mode {
                    if let Some(page) = self.track_page.as_mut() {
                        page.pending.clear();
                    }
                    return Task::perform(fetch_artwork(init_urls), Message::ArtworkLoaded);
                }

                Task::batch(vec![
                    Task::perform(fetch_artwork(init_urls), Message::ArtworkLoaded),
                    Task::perform(fetch_track_detail(tid), |r| match r {
                        Ok(t) => Message::TrackPageDetailLoaded(Ok(t)),
                        Err(e) => Message::TrackPageDetailLoaded(Err(e.to_string())),
                    }),
                    Task::perform(fetch_track_related_pair(tid), |r| match r {
                        Ok(p) => Message::TrackPageRelatedLoaded(Ok(p)),
                        Err(e) => Message::TrackPageRelatedLoaded(Err(e.to_string())),
                    }),
                    Task::perform(fetch_inspector_comments(tid), |r| match r {
                        Ok(c) => Message::TrackPageCommentsLoaded(Ok(c)),
                        Err(e) => Message::TrackPageCommentsLoaded(Err(e.to_string())),
                    }),
                    Task::perform(fetch_track_favoriters(tid), |r| match r {
                        Ok(f) => Message::TrackPageLikersLoaded(Ok(f)),
                        Err(e) => Message::TrackPageLikersLoaded(Err(e.to_string())),
                    }),
                    Task::perform(fetch_track_reposters(tid), |r| match r {
                        Ok(rp) => Message::TrackPageRepostersLoaded(Ok(rp)),
                        Err(e) => Message::TrackPageRepostersLoaded(Err(e.to_string())),
                    }),
                ])
            }
            Message::TrackPageDetailLoaded(Ok(track)) => {
                if let Some(page) = self.track_page.as_mut() {
                    if page.track.id == track.id {
                        page.track = track.clone();
                    }
                }
                Task::none()
            }
            Message::TrackPageDetailLoaded(Err(e)) => {
                crate::log!("track detail load error: {e}");
                Task::none()
            }
            Message::TrackPageRelatedLoaded(Ok((tid, tracks))) => {
                let mut urls = Vec::new();
                if let Some(page) = self.track_page.as_mut() {
                    if page.track.id == tid {
                        page.pending.remove(&TrackSubTab::Related);
                        page.related_tracks = tracks.clone();
                        urls = extract_tracks_artwork(&tracks);
                    }
                }
                if urls.is_empty() {
                    Task::none()
                } else {
                    Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
                }
            }
            Message::TrackPageRelatedLoaded(Err(e)) => {
                crate::log!("track related load error: {e}");
                if let Some(page) = self.track_page.as_mut() {
                    page.pending.remove(&TrackSubTab::Related);
                }
                Task::none()
            }
            Message::TrackPageCommentsLoaded(Ok((tid, comments))) => {
                let mut urls = Vec::new();
                if let Some(page) = self.track_page.as_mut() {
                    if page.track.id == tid {
                        page.pending.remove(&TrackSubTab::Comments);
                        page.comments = comments.clone();
                        urls = comments
                            .into_iter()
                            .filter_map(|c| c.user.and_then(|u| u.avatar_url))
                            .collect();
                    }
                }
                if urls.is_empty() {
                    Task::none()
                } else {
                    Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
                }
            }
            Message::TrackPageCommentsLoaded(Err(e)) => {
                crate::log!("track comments load error: {e}");
                if let Some(page) = self.track_page.as_mut() {
                    page.pending.remove(&TrackSubTab::Comments);
                }
                Task::none()
            }
            Message::TrackPageLikersLoaded(Ok((tid, users))) => {
                let mut urls = Vec::new();
                if let Some(page) = self.track_page.as_mut() {
                    if page.track.id == tid {
                        page.pending.remove(&TrackSubTab::Likers);
                        page.likers = users.clone();
                        urls = users.into_iter().filter_map(|u| u.avatar_url).collect();
                    }
                }
                if urls.is_empty() {
                    Task::none()
                } else {
                    Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
                }
            }
            Message::TrackPageLikersLoaded(Err(e)) => {
                crate::log!("track likers load error: {e}");
                if let Some(page) = self.track_page.as_mut() {
                    page.pending.remove(&TrackSubTab::Likers);
                }
                Task::none()
            }
            Message::TrackPageRepostersLoaded(Ok((tid, users))) => {
                let mut urls = Vec::new();
                if let Some(page) = self.track_page.as_mut() {
                    if page.track.id == tid {
                        page.pending.remove(&TrackSubTab::Reposters);
                        page.reposters = users.clone();
                        urls = users.into_iter().filter_map(|u| u.avatar_url).collect();
                    }
                }
                if urls.is_empty() {
                    Task::none()
                } else {
                    Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
                }
            }
            Message::TrackPageRepostersLoaded(Err(e)) => {
                crate::log!("track reposters load error: {e}");
                if let Some(page) = self.track_page.as_mut() {
                    page.pending.remove(&TrackSubTab::Reposters);
                }
                Task::none()
            }
            Message::TrackPageSubTabSelected(tab) => {
                if let Some(page) = self.track_page.as_mut() {
                    page.active_tab = tab;
                }
                Task::none()
            }
            Message::TrackPageCommentInput(input) => {
                if let Some(page) = self.track_page.as_mut() {
                    page.comment_input = input;
                }
                Task::none()
            }
            Message::TrackPagePostComment => {
                if let Some(page) = self.track_page.as_mut() {
                    let text = page.comment_input.trim().to_string();
                    if !text.is_empty() {
                        let tid = page.track.id;
                        let ts = if self.playing_id == Some(tid) {
                            self.pos_ms
                        } else {
                            0
                        };
                        page.comment_input.clear();
                        Task::perform(do_post_comment(tid, text, ts), |r| match r {
                            Ok(()) => Message::TrackPageCommentPosted(Ok(())),
                            Err(e) => Message::TrackPageCommentPosted(Err(e.to_string())),
                        })
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::TrackPageReact(codepoint) => {
                let Some(page) = self.track_page.as_ref() else {
                    return Task::none();
                };
                let track_id = page.track.id;
                let playing = self.playing_id == Some(track_id);
                let second = if playing { self.pos_ms / 1000 } else { 0 };
                let origin = Point::new(
                    self.window_size.width / 2.0,
                    self.window_size.height - PB_H - 8.0,
                );
                self.particles.extend(Particle::burst(&codepoint, origin));
                let excess = self.particles.len().saturating_sub(MAX_PARTICLES);
                self.particles.drain(..excess);
                let posted = Some((track_id, second, codepoint.clone()));
                if self.last_posted_reaction == posted {
                    return Task::none();
                }
                self.last_posted_reaction = posted;
                // the waveform's reactions are the playing track's: another
                // track's page only sends its own
                if playing {
                    let list = self.reactions.entry(second).or_default();
                    let reaction = WaveReaction {
                        second,
                        codepoint: codepoint.clone(),
                    };
                    if let Some(existing) = list.last_mut() {
                        *existing = reaction;
                    } else {
                        list.push(reaction);
                    }
                }
                Task::perform(
                    do_add_reaction(track_id, second, codepoint),
                    Message::ReactionPosted,
                )
            }
            Message::TrackPageCommentPosted(Ok(())) => {
                self.show_toast("Comment posted!", ToastKind::Success);
                if let Some(page) = self.track_page.as_ref() {
                    let tid = page.track.id;
                    Task::perform(fetch_inspector_comments(tid), |r| match r {
                        Ok(c) => Message::TrackPageCommentsLoaded(Ok(c)),
                        Err(e) => Message::TrackPageCommentsLoaded(Err(e.to_string())),
                    })
                } else {
                    Task::none()
                }
            }
            Message::TrackPageCommentPosted(Err(e)) => {
                self.show_toast(format!("Failed to post comment: {e}"), ToastKind::Error);
                Task::none()
            }
            Message::TrackPagePlayToggle => {
                if let Some(page) = self.track_page.as_ref() {
                    let tid = page.track.id;
                    if self.playing_id == Some(tid) {
                        // same toggle as the player bar (handles a finished track)
                        self.update(Message::PlayerToggle)
                    } else {
                        let mut context = vec![page.track.clone()];
                        for rel in &page.related_tracks {
                            if rel.id != tid {
                                context.push(rel.clone());
                            }
                        }
                        let pos = self.set_queue(context, 0);
                        self.play_index(pos)
                    }
                } else {
                    Task::none()
                }
            }
            Message::TrackPagePlayAllRelated => {
                if let Some(page) = self.track_page.as_ref() {
                    if !page.related_tracks.is_empty() {
                        let tracks = page.related_tracks.clone();
                        let pos = self.set_queue(tracks, 0);
                        self.play_index(pos)
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::FollowToggle => {
                if !self.state.authenticated {
                    self.show_toast("Log in first to follow artists", ToastKind::Error);
                    return Task::none();
                }
                let Some(profile) = self.profile.clone() else {
                    return Task::none();
                };
                let was = profile.following;
                let mut newp = profile.clone();
                newp.following = !was;
                self.profile = Some(newp);
                if was {
                    self.my_following_ids.remove(&profile.id);
                } else {
                    self.my_following_ids.insert(profile.id);
                }
                crate::log!(
                    "follow toggle: user {} -> {}",
                    profile.id,
                    if was { "unfollow" } else { "follow" }
                );
                let uid = profile.id;
                self.library_follow(
                    uid,
                    !was,
                    Some(UserMini {
                        id: uid,
                        username: profile.username.clone(),
                        avatar_url: profile.avatar_url.clone(),
                        track_count: Some(profile.track_count),
                        followers_count: Some(profile.followers),
                        ..Default::default()
                    }),
                );
                Task::perform(do_follow(uid, was), move |r| {
                    Message::ToggleDone(Toggle::Follow(uid, was), r.map_err(|e| e.to_string()))
                })
            }
            Message::StartTrackRadio(_, _) | Message::StartArtistRadio(_, _)
                if self.settings.offline_mode =>
            {
                self.show_toast("Radio isn't available offline", ToastKind::Info);
                Task::none()
            }
            Message::StartTrackRadio(track_id, track_title) => {
                self.show_toast(
                    format!("Starting radio for '{}'…", trunc(&track_title, 26)),
                    ToastKind::Info,
                );
                self.action_menu = None;
                self.radio_gen += 1;
                let gen = self.radio_gen;
                self.radio_request = Some(RadioRequest {
                    gen,
                    seed: Some(track_id),
                    title: format!("{} Radio", trunc(&track_title, 26)),
                    urn: format!("soundcloud:system-playlists:track-stations:{track_id}"),
                });
                Task::perform(do_radio(track_id), move |r| {
                    Message::RadioLoaded(gen, r.map_err(|e| e.to_string()))
                })
            }
            Message::StartArtistRadio(artist_id, artist_name) => {
                self.show_toast(
                    format!("Starting artist radio for '{}'…", trunc(&artist_name, 26)),
                    ToastKind::Info,
                );
                self.action_menu = None;
                self.radio_gen += 1;
                let gen = self.radio_gen;
                self.radio_request = Some(RadioRequest {
                    gen,
                    seed: None,
                    title: format!("{} Radio", trunc(&artist_name, 26)),
                    urn: format!("soundcloud:system-playlists:artist-stations:{artist_id}"),
                });
                Task::perform(do_artist_radio(artist_id), move |r| {
                    Message::RadioLoaded(gen, r.map_err(|e| e.to_string()))
                })
            }
            // a radio asked for before the latest one
            Message::RadioLoaded(gen, _)
                if self.radio_request.as_ref().map(|r| r.gen) != Some(gen) =>
            {
                Task::none()
            }
            Message::RadioLoaded(_, Ok(tracks)) => {
                let Some(req) = self.radio_request.take() else {
                    return Task::none();
                };
                if tracks.is_empty() {
                    crate::log!("radio: empty station");
                    self.show_toast("No station tracks found", ToastKind::Error);
                    return Task::none();
                }
                let n = tracks.len();
                crate::log!("radio: {} tracks queued", n);
                // started from the playing track: it keeps playing, the
                // station queued after it
                let keep_current = req.seed.is_some() && self.playing_id == req.seed;
                let urls = extract_tracks_artwork(&tracks);
                // a station shows its artist's avatar (see station_avatar)
                let artwork_url = station_avatar(&PlaylistDetail {
                    id_or_urn: req.urn.clone(),
                    tracks: tracks.clone(),
                    ..Default::default()
                })
                .or_else(|| {
                    tracks
                        .first()
                        .and_then(|t| t.artwork_or_avatar().map(str::to_string))
                });
                // the station is SoundCloud's own system playlist: Your
                // Library keeps it under Radio and reopens it by URN
                self.remember_radio(LibraryItem {
                    urn: req.urn.clone(),
                    title: req.title.clone(),
                    artwork_url: artwork_url.clone(),
                    started_ms: crate::config::now_ms(),
                });
                let radio_detail = PlaylistDetail {
                    id: playlist_id_for(&req.urn),
                    id_or_urn: req.urn.clone(),
                    title: req.title.clone(),
                    description: Some(
                        "SoundCloud Radio Station based on your selection".to_string(),
                    ),
                    artwork_url,
                    author: "SoundCloud Radio".to_string(),
                    author_avatar: None,
                    author_id: None,
                    permalink_url: None,
                    track_count: tracks.len(),
                    tracks: tracks.clone(),
                    is_album: false,
                };
                // the step away is the page on screen, so the radio becomes
                // the open playlist only after it's recorded; a playlist
                // still loading (or its error) mustn't replace it
                self.push_nav(Route::Playlist(req.urn.clone()));
                self.current_playlist = Some(radio_detail);
                self.loading_playlist = None;
                self.page_error = None;
                let start = self.set_queue(tracks, 0);
                let art_task = Task::perform(fetch_artwork(urls), Message::ArtworkLoaded);
                if keep_current {
                    self.queue_pos = start;
                    self.show_toast(
                        format!("{} ready ({} tracks queued)", req.title, n),
                        ToastKind::Success,
                    );
                    art_task
                } else {
                    self.show_toast(
                        format!("{} ready ({} tracks)", req.title, n),
                        ToastKind::Success,
                    );
                    let play_task = self.play_index(start);
                    Task::batch(vec![play_task, art_task])
                }
            }
            Message::RadioLoaded(_, Err(e)) => {
                self.radio_request = None;
                crate::log!("radio FAILED: {e}");
                self.show_toast(format!("Radio error: {e}"), ToastKind::Error);
                Task::none()
            }
            Message::LikedPlaylistsLoaded(Ok(list)) => {
                crate::log!("liked playlists loaded: {}", list.len());
                self.liked_playlists = list;
                self.offline_store.liked_playlists = self.liked_playlists.clone();
                self.save_offline_store();
                let mut urls = Vec::new();
                for pl in &self.liked_playlists {
                    if let Some(u) = pl.artwork_or_avatar() {
                        if !urls.iter().any(|x| x == u) {
                            urls.push(u.to_string());
                        }
                    }
                }
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::LikedPlaylistsLoaded(Err(e)) => {
                crate::log!("liked playlists FAILED: {e}");
                Task::none()
            }
            Message::UserFlagsLoaded(Ok((reposts, liked_playlists, followings, saved_system))) => {
                crate::log!(
                    "user flags: {} reposted tracks, {} liked playlists, {} followings, {} saved mixes/stations",
                    reposts.len(),
                    liked_playlists.len(),
                    followings.len(),
                    saved_system.len()
                );
                self.reposted_ids = reposts;
                self.liked_playlist_ids = liked_playlists;
                self.my_following_ids = followings;
                self.liked_system_urns = saved_system;
                Task::none()
            }
            Message::SystemPlaylistLikeToggle(urn) => {
                if !self.state.authenticated {
                    self.show_toast("Log in first to save to Your Library", ToastKind::Error);
                    return Task::none();
                }
                let was = self.liked_system_urns.contains(&urn);
                if was {
                    self.liked_system_urns.remove(&urn);
                    self.show_toast("Removed from Your Library", ToastKind::Info);
                } else {
                    self.liked_system_urns.insert(urn.clone());
                    self.show_toast("Saved to Your Library", ToastKind::Success);
                    // Your Library shows it at once, under Radio or Mixes
                    if let Some(page) = self
                        .current_playlist
                        .as_ref()
                        .filter(|p| p.id_or_urn == urn)
                    {
                        let item = LibraryItem {
                            urn: urn.clone(),
                            title: page.title.clone(),
                            artwork_url: station_avatar(page).or_else(|| page.artwork_url.clone()),
                            started_ms: 0,
                        };
                        let list = if is_station_urn(&urn) {
                            &mut self.library_radios
                        } else {
                            &mut self.library_mixes
                        };
                        if list.iter().all(|i| i.urn != urn) {
                            list.insert(0, item);
                            self.store_library_items();
                        }
                    }
                }
                let done_urn = urn.clone();
                Task::perform(do_system_playlist_like(urn, was), move |r| {
                    Message::SystemPlaylistLikeDone(
                        done_urn.clone(),
                        was,
                        r.map_err(|e| e.to_string()),
                    )
                })
            }
            Message::SystemPlaylistLikeDone(urn, was, Ok(())) => {
                crate::log!("system playlist {urn}: saved={}", !was);
                Task::none()
            }
            Message::SystemPlaylistLikeDone(urn, was, Err(e)) => {
                crate::log!("system playlist {urn}: save FAILED: {e}");
                // SoundCloud didn't take it: back the way it was
                if was {
                    self.liked_system_urns.insert(urn);
                } else {
                    self.liked_system_urns.remove(&urn);
                }
                self.action_failed(&e);
                Task::none()
            }
            Message::UserFlagsLoaded(Err(e)) => {
                crate::log!("user flags FAILED: {e}");
                Task::none()
            }
            Message::PlaylistLikeToggle(id) => {
                if !self.state.authenticated {
                    return Task::none();
                }
                let was = self.liked_playlist_ids.contains(&id);
                if was {
                    self.liked_playlist_ids.remove(&id);
                } else {
                    self.liked_playlist_ids.insert(id);
                }
                crate::log!(
                    "playlist like toggle: {id} -> {}",
                    if was { "unlike" } else { "like" }
                );
                // Your Library shows it (or not) at once; the list is fetched
                // again once SoundCloud has it
                if was {
                    self.liked_playlists.retain(|p| p.id != id);
                } else if let Some(page) = self.current_playlist.as_ref().filter(|p| p.id == id) {
                    if self.liked_playlists.iter().all(|p| p.id != id) {
                        self.liked_playlists.insert(
                            0,
                            Playlist {
                                id,
                                urn: Some(page.id_or_urn.clone()),
                                title: page.title.clone(),
                                artwork_url: page.artwork_url.clone(),
                                user: Some(UserMini {
                                    id: page.author_id.unwrap_or(0),
                                    username: page.author.clone(),
                                    avatar_url: page.author_avatar.clone(),
                                    ..Default::default()
                                }),
                                track_count: Some(page.track_count as u64),
                                is_album: Some(page.is_album),
                                permalink_url: page.permalink_url.clone(),
                                ..Default::default()
                            },
                        );
                    }
                }
                Task::perform(do_playlist_like(id, was), move |r| {
                    Message::ToggleDone(Toggle::PlaylistLike(id, was), r.map_err(|e| e.to_string()))
                })
            }
            Message::SearchMore => {
                if let Some(next) = self.search_next.take() {
                    crate::log!("search: loading more…");
                    let gen = self.search_gen;
                    return Task::perform(fetch_search_next(next.clone()), move |r| {
                        Message::SearchMoreLoaded(gen, next.clone(), r.map_err(|e| e.to_string()))
                    });
                }
                Task::none()
            }
            // a page of an older search: the results on screen are another's
            Message::SearchMoreLoaded(gen, ..) if gen != self.search_gen => Task::none(),
            Message::SearchMoreLoaded(_, _, Ok((tracks, next))) => {
                crate::log!("search more: +{} tracks", tracks.len());
                let urls = extract_tracks_artwork(&tracks);
                let known: std::collections::HashSet<i64> =
                    self.search.tracks.iter().map(|t| t.id).collect();
                self.search
                    .tracks
                    .extend(tracks.into_iter().filter(|t| !known.contains(&t.id)));
                // the page after this one ("Load more" again)
                self.search_next = next;
                Task::perform(fetch_artwork(urls), Message::ArtworkLoaded)
            }
            Message::SearchMoreLoaded(_, page, Err(e)) => {
                crate::log!("search more FAILED: {e}");
                // keep the page, so "Load more" can try it again
                self.search_next = Some(page);
                self.show_toast("Couldn't load more results", ToastKind::Error);
                Task::none()
            }
            Message::PrevTrack => {
                // Fix #5: Rewind to 0 if we're more than 3 seconds in — always, regardless of
                // repeat mode. Without this guard, repeat=All would wrap to the last track even
                // when the user just wants to restart the current song.
                if self.pos_ms > 3000 {
                    crate::log!("rewinding to start of current track");
                    self.pos_ms = 0;
                    // a resume still waiting for its audio starts from 0 too
                    if self.pending_seek_track == self.playing_id && self.pending_seek_ms.is_some()
                    {
                        self.pending_seek_ms = Some(0);
                    }
                    if let Some(p) = &self.player {
                        p.send(PlayerCommand::SeekMs(0));
                    }
                    return Task::none();
                }
                if self.queue_pos > 0 {
                    crate::log!("prev track");
                    self.play_index(self.queue_pos - 1)
                } else if self.repeat == RepeatMode::All && !self.queue.is_empty() {
                    self.play_index(self.queue.len() - 1)
                } else {
                    Task::none()
                }
            }
            Message::LoginStart => {
                self.login_pending = true;
                let _ = std::fs::remove_file(crate::config::auth_error_path());
                // Signing in again: set the current token aside, so a
                // cancelled or failed login leaves the user signed in.
                let tok = crate::config::token_path();
                if tok.exists() {
                    let _ = std::fs::rename(&tok, tok.with_extension("json.bak"));
                }
                Task::perform(spawn_login_child(), |r| match r {
                    Ok(()) => Message::TokenPolled(true),
                    Err(e) => Message::LoginFailed(e),
                })
            }
            Message::CookieInput(s) => {
                self.cookie_input = s;
                Task::none()
            }
            Message::PasteShortcut => {
                // A cookie paste only where the sign-in form is. Anywhere
                // else it re-ran the login with whatever was copied (a track
                // link) and threw the user onto Settings with an error.
                if !self.state.authenticated && self.tab == Tab::Settings {
                    return self.update(Message::PasteClipboard);
                }
                Task::none()
            }
            Message::PasteClipboard => clipboard::read().map(|content| match content {
                Some(s) if !s.trim().is_empty() => Message::CookiePasted(s),
                _ => Message::LoginFailed(
                    "Clipboard empty — copy Cookie header in browser first".into(),
                ),
            }),
            Message::CookiePasted(s) => {
                self.cookie_input = s.clone();
                self.tab = Tab::Settings;
                self.login_error = Some("Authorizing…".into());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || authorize_paste_keeping_jar(&s))
                            .await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| e.to_string())?;
                        init_api().await.map_err(|e| e.to_string())
                    },
                    |r| match r {
                        Ok(st) => Message::ApiReady(Ok(st)),
                        Err(e) => Message::ApiReady(Err(e)),
                    },
                )
            }
            Message::CookieOpenBrowser => {
                let _ = crate::cookie_auth::open_social_login();
                self.login_error = Some(
                    "Sign in in browser -> F12 -> Network -> click any soundcloud request -> copy Cookie -> Paste from clipboard."
                        .into(),
                );
                Task::none()
            }
            Message::CookieAuthorize => {
                let raw = self.cookie_input.trim().to_string();
                if raw.is_empty() {
                    return clipboard::read().map(|content| match content {
                        Some(s) if !s.trim().is_empty() => Message::CookiePasted(s),
                        _ => Message::LoginFailed(
                            "Nothing to authorize — use Paste from clipboard first".into(),
                        ),
                    });
                }
                self.login_error = None;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || authorize_paste_keeping_jar(&raw))
                            .await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| e.to_string())?;
                        init_api().await.map_err(|e| e.to_string())
                    },
                    |r| match r {
                        Ok(s) => Message::ApiReady(Ok(s)),
                        Err(e) => Message::ApiReady(Err(e)),
                    },
                )
            }
            Message::TokenPolled(true) => {
                self.login_pending = false;
                let _ =
                    std::fs::remove_file(crate::config::token_path().with_extension("json.bak"));
                Task::perform(init_api(), |r| match r {
                    Ok(s) => Message::ApiReady(Ok(s)),
                    Err(e) => Message::ApiReady(Err(e.to_string())),
                })
            }
            Message::TokenPolled(false) => {
                self.login_pending = false;
                restore_token_backup();
                if let Ok(e) = std::fs::read_to_string(crate::config::auth_error_path()) {
                    self.login_error = Some(format!("login: {e}"));
                } else {
                    self.login_error = Some("Login timed out ? paste the code manually".into());
                }
                Task::none()
            }
            Message::LoginFailed(e) => {
                self.login_pending = false;
                restore_token_backup();
                self.login_error = Some(format!("login: {e}"));
                Task::none()
            }
            Message::Logout => {
                self.show_user_menu = false;
                let _ = std::fs::remove_file(config_dir().join("token.json"));
                // The jar holds the browser session (oauth_token cookie) and
                // goes out with every request: it would outlive the logout
                // and ride along with the next account's token.
                let _ = std::fs::remove_file(crate::config::cookies_path());
                if let Some(p) = &self.player {
                    p.send(PlayerCommand::Stop);
                }
                self.state.authenticated = false;
                self.state.me = None;
                self.library.clear();
                // Nothing is loaded any more: show the bar stopped instead of
                // "playing" over silence with a Play that does nothing.
                self.play_gen = self.play_gen.wrapping_add(1); // drop in-flight resolves
                self.set_queue(Vec::new(), 0);
                self.queue_pos = 0;
                self.playing_id = None;
                self.playing_title.clear();
                self.playing_artist.clear();
                self.is_paused = true;
                self.update_discord_rpc();
                self.at_end = false;
                self.awaiting_audio = None;
                self.play_failures = 0;
                self.pos_ms = 0;
                self.dur_ms = 0;
                self.wave_bars = std::sync::Arc::new(Vec::new());
                self.wave_comments.clear();
                self.comments_next = None;
                self.prefetched_stream = None;
                self.prefetch_in_progress = None;
                self.related_seed = None;
                // The account's likes, follows and playlists (the sidebar
                // lists them without an auth check). The gen bump drops a
                // PlaylistsLoaded still in flight.
                self.liked_ids.clear();
                self.reposted_ids.clear();
                self.my_following_ids.clear();
                self.liked_playlist_ids.clear();
                self.liked_system_urns.clear();
                self.liked_playlists.clear();
                self.my_playlists.clear();
                self.my_followings.clear();
                self.library_mixes.clear();
                self.library_radios.clear();
                self.playlists_gen += 1;
                let _ = std::fs::remove_file(crate::config::library_cache_path());
                self.offline_store.me = None;
                self.offline_store.my_playlists.clear();
                self.offline_store.liked_playlists.clear();
                self.offline_store.my_followings.clear();
                self.offline_store.mixes.clear();
                self.offline_store.radios.clear();
                self.save_offline_store();
                self.add_popover = None;
                // Stories come from the account's follow feed (or were saved
                // from it last session): fall back to the ones Home offers,
                // as on a logged-out start.
                self.stories = extract_stories_from_home(&self.home);
                self.stories_from_stream = false;
                self.save_stories();
                self.update_discord_rpc();
                self.close_story()
            }
            Message::SettingsSearch(q) => {
                self.settings_search = q;
                Task::none()
            }
            Message::StorageMeasured(cache, downloads) => {
                self.storage = Some((cache, downloads));
                Task::none()
            }
            Message::ClearCache => {
                let keep = self.cache_keep();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || clear_cache(&keep))
                            .await
                            .unwrap_or(0)
                    },
                    Message::CacheCleared,
                )
            }
            Message::CacheCleared(freed) => {
                crate::log!("cache cleared: {} bytes", freed);
                self.show_toast(
                    format!("Cache cleared ({})", fmt_bytes(freed)),
                    ToastKind::Success,
                );
                // streamed tracks are no longer on disk
                self.downloaded_track_ids = App::scan_downloaded_track_ids();
                self.measure_storage()
            }
            Message::ClearTrackSpeeds => {
                if self.track_speeds.is_empty() {
                    return Task::none();
                }
                self.track_speeds.clear();
                self.track_speeds_dirty = true;
                self.save_track_speeds();
                // the playing track goes back to normal with the rest
                if !self.story_playback {
                    if let Some(id) = self.playing_id {
                        self.apply_track_speed(id);
                    }
                }
                self.show_toast("Saved track speeds cleared", ToastKind::Info);
                Task::none()
            }
            Message::SettingsProxyChanged(s) => {
                self.proxy_draft = s;
                Task::none()
            }
            Message::SettingsHideScrollbarsToggled(v) => {
                self.settings.hide_scrollbars = v;
                self.settings.save();
                Task::none()
            }
            Message::SettingsBypassToggled(v) => {
                self.settings.bypass_unavailable = v;
                self.settings.save();
                self.show_toast(
                    if v {
                        "Unlock through a proxy: on"
                    } else {
                        "Unlock through a proxy: off"
                    },
                    ToastKind::Info,
                );
                Task::none()
            }
            Message::SettingsOfflineModeToggled(v) => {
                self.settings.offline_mode = v;
                self.settings.save();
                self.show_toast(
                    if v {
                        "Offline mode enabled"
                    } else {
                        "Offline mode disabled"
                    },
                    ToastKind::Info,
                );
                if v {
                    self.go_offline();
                    Task::none()
                } else {
                    self.go_online()
                }
            }
            Message::SettingsDiscordRpcToggled(v) => {
                self.settings.discord_rpc = v;
                self.settings.save();
                self.update_discord_rpc();
                self.show_toast(
                    if v {
                        "Listening activity: on"
                    } else {
                        "Listening activity: off"
                    },
                    ToastKind::Info,
                );
                Task::none()
            }
            Message::SettingsPreferArtistFromName(v) => {
                self.settings.prefer_artist_from_name = v;
                self.settings.save();
                if let Some(id) = self.playing_id {
                    if let Some(tr) = self.find_track_anywhere(id) {
                        let (artist, title) = tr.display_artist_and_title(v);
                        self.playing_artist = artist;
                        self.playing_title = title;
                        self.update_discord_rpc();
                    }
                }
                self.show_toast(
                    if v {
                        "Artist mode: Prefer Artists from track name"
                    } else {
                        "Artist mode: Prefer Artists from track"
                    },
                    ToastKind::Info,
                );
                Task::none()
            }
            Message::ToggleStoriesExpanded => {
                self.stories_expanded = !self.stories_expanded;
                Task::none()
            }
            Message::OpenStory(idx) => self.open_story(idx),
            Message::CloseStory => self.close_story(),
            Message::NextStory => {
                if let Some(idx) = self.active_story_index {
                    if idx + 1 < self.stories.len() {
                        self.open_story(idx + 1)
                    } else {
                        self.close_story()
                    }
                } else {
                    Task::none()
                }
            }
            Message::PrevStory => {
                if let Some(idx) = self.active_story_index {
                    if idx > 0 {
                        self.open_story(idx - 1)
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::DownloadCollection(c) => {
                if self.offline_downloading.is_some() {
                    self.show_toast("Another download is already in progress", ToastKind::Info);
                    return Task::none();
                }
                if self.settings.offline_mode {
                    self.show_toast("Turn off offline mode to download", ToastKind::Info);
                    return Task::none();
                }
                let tracks = self.collection_tracks(c).to_vec();
                // No tracks yet (a preview still loading) is not "complete".
                if tracks.is_empty() {
                    self.show_toast("No tracks to download yet", ToastKind::Info);
                    return Task::none();
                }
                if let Collection::Playlist(id) = c {
                    // metadata first, so offline mode lists it even if the
                    // run is cut short
                    self.remember_offline_playlist(id);
                    self.save_offline_store();
                }

                let mut seen = std::collections::HashSet::new();
                self.offline_plan = tracks.into_iter().filter(|t| seen.insert(t.id)).collect();
                self.offline_attempted.clear();
                self.offline_failed = 0;
                self.offline_download_progress = (0, 0);
                self.offline_downloading = Some(c);
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.offline_batch_cancel = Some(cancel.clone());
                let Some(first) = self.next_offline_track() else {
                    self.offline_downloading = None;
                    self.offline_batch_cancel = None;
                    for t in std::mem::take(&mut self.offline_plan) {
                        self.remember_offline_track(&t);
                    }
                    self.save_offline_store();
                    if let Collection::Playlist(id) = c {
                        self.downloaded_playlist_ids.insert(id.to_string());
                        self.save_downloaded_playlists();
                    }
                    self.show_toast("Already downloaded", ToastKind::Success);
                    return Task::none();
                };

                let total = self.offline_download_progress.1;
                self.show_toast(format!("Downloading {} tracks...", total), ToastKind::Info);

                let first_id = first.id;
                let cid = self.state.client_id.clone();
                let quality = self.download_quality();
                Task::perform(
                    cache_track_offline(first, cid, quality, cancel),
                    move |res| Message::TrackDownloaded(first_id, res),
                )
            }
            Message::CancelOfflineDownload => {
                if let Some(cancel) = &self.offline_batch_cancel {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                    self.show_toast("Stopping download…", ToastKind::Info);
                }
                Task::none()
            }
            Message::TrackDownloaded(track_id, res) => {
                if matches!(&res, Err(e) if e == "Download canceled") {
                    self.offline_downloading = None;
                    self.offline_batch_cancel = None;
                    self.offline_plan.clear();
                    self.offline_attempted.clear();
                    self.save_offline_store();
                    self.show_toast("Download stopped", ToastKind::Info);
                    return Task::none();
                }
                self.offline_attempted.insert(track_id);
                self.offline_download_progress.0 += 1;
                match res {
                    Ok(()) => {
                        self.downloaded_track_ids.insert(track_id);
                        if let Some(t) =
                            self.offline_plan.iter().find(|t| t.id == track_id).cloned()
                        {
                            self.remember_offline_track(&t);
                        }
                        // saved in batches: a long run survives a crash
                        // without rewriting the store after every track
                        if self.offline_download_progress.0 % 10 == 0 {
                            self.save_offline_store();
                        }
                    }
                    Err(e) => {
                        // Counted and skipped, never retried: an unavailable
                        // (geo-blocked, removed) track fails the same way
                        // every time.
                        crate::log!("Track {} failed download: {}", track_id, e);
                        self.offline_failed += 1;
                    }
                }

                let Some(c) = self.offline_downloading else {
                    return Task::none();
                };
                if let Some(next_tr) = self.next_offline_track() {
                    let next_id = next_tr.id;
                    let cid = self.state.client_id.clone();
                    let quality = self.download_quality();
                    let cancel = self.offline_batch_cancel.clone().unwrap_or_else(|| {
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))
                    });
                    return Task::perform(
                        cache_track_offline(next_tr, cid, quality, cancel),
                        move |res| Message::TrackDownloaded(next_id, res),
                    );
                }
                self.offline_downloading = None;
                self.offline_batch_cancel = None;
                self.offline_plan.clear();
                self.offline_attempted.clear();
                if let Collection::Playlist(id) = c {
                    // the full track list, if the page loaded it meanwhile
                    self.remember_offline_playlist(id);
                }
                self.save_offline_store();
                if self.offline_failed > 0 {
                    // Not marked downloaded: the Download pill stays, and a
                    // later click fetches only the tracks still missing.
                    let failed = self.offline_failed;
                    let total = self.offline_download_progress.1;
                    self.show_toast(
                        format!("{failed} of {total} tracks couldn't be downloaded"),
                        ToastKind::Error,
                    );
                    return Task::none();
                }
                if let Collection::Playlist(id) = c {
                    self.downloaded_playlist_ids.insert(id.to_string());
                    self.save_downloaded_playlists();
                }
                self.show_toast("Downloaded for offline playback!", ToastKind::Success);
                Task::none()
            }
            Message::DownloadTrackOffline(track) => {
                if self.downloaded_track_ids.contains(&track.id) {
                    self.remember_offline_track(&track);
                    self.save_offline_store();
                    self.show_toast("Already downloaded", ToastKind::Success);
                    return Task::none();
                }
                if self.settings.offline_mode {
                    self.show_toast("Turn off offline mode to download", ToastKind::Info);
                    return Task::none();
                }
                if !self.offline_single.insert(track.id) {
                    return Task::none();
                }
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.offline_single_cancel.insert(track.id, cancel.clone());
                let cid = self.state.client_id.clone();
                let quality = self.download_quality();
                Task::perform(
                    cache_track_offline(track.clone(), cid, quality, cancel),
                    move |res| Message::TrackCachedOffline(track.clone(), res),
                )
            }
            Message::CancelOfflineTrack(track_id) => {
                if let Some(cancel) = self.offline_single_cancel.get(&track_id) {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                    self.show_toast("Stopping download…", ToastKind::Info);
                }
                Task::none()
            }
            Message::TrackCachedOffline(track, res) => {
                self.offline_single.remove(&track.id);
                self.offline_single_cancel.remove(&track.id);
                match res {
                    Ok(()) => {
                        self.downloaded_track_ids.insert(track.id);
                        self.remember_offline_track(&track);
                        self.save_offline_store();
                        let (_, title) =
                            track.display_artist_and_title(self.settings.prefer_artist_from_name);
                        self.show_toast(
                            format!("Downloaded '{}'", trunc(&title, 28)),
                            ToastKind::Success,
                        );
                    }
                    Err(e) => {
                        if e == "Download canceled" {
                            self.show_toast("Download stopped", ToastKind::Info);
                            return Task::none();
                        }
                        crate::log!("Track {} failed download: {}", track.id, e);
                        self.show_toast(format!("Download failed: {e}"), ToastKind::Error);
                    }
                }
                Task::none()
            }
            Message::SettingsSave => {
                let field = |s: &str| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                };
                let proxy = self.proxy_draft.trim().to_string();
                if !proxy.is_empty() && !crate::config::valid_proxy(&proxy) {
                    self.show_toast(
                        "A proxy looks like protocol://host:port or protocol://user:pass@host:port",
                        ToastKind::Error,
                    );
                    return Task::none();
                }
                self.proxy_draft = proxy;
                self.settings.proxy = field(&self.proxy_draft);
                self.settings.save();
                self.show_toast(
                    if self.settings.proxy.is_some() {
                        "Proxy saved"
                    } else {
                        "Proxy removed"
                    },
                    ToastKind::Success,
                );
                self.state.client_id = self
                    .settings
                    .client_id_override
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| CLIENT_ID.to_string());
                Task::perform(init_api(), |r| match r {
                    Ok(s) => Message::ApiReady(Ok(s)),
                    Err(e) => Message::ApiReady(Err(e.to_string())),
                })
            }
            Message::SetPref(pref) => self.set_pref(pref),
            Message::CheckUpdates => {
                self.update_checking = true;
                self.update_check_manual = true;
                Task::perform(crate::updater::check_latest(), |result| {
                    Message::UpdatesChecked(result.map_err(|e| e.to_string()), true)
                })
            }
            Message::UpdatesChecked(result, manual) => {
                self.update_checking = false;
                self.update_check_manual = false;
                match result {
                    Ok(Some(release))
                        if manual
                            || self.settings.ignored_update_version.as_deref()
                                != Some(release.version.as_str()) =>
                    {
                        self.remind_after_new_version = true;
                        self.update_release = Some(release);
                    }
                    Ok(None) if manual => self.show_toast("Wavify is up to date", ToastKind::Success),
                    Err(error) if manual => self.show_toast(
                        format!("Couldn't check for updates: {error}"),
                        ToastKind::Error,
                    ),
                    Err(error) => crate::log!("update check failed: {error}"),
                    _ => {}
                }
                Task::none()
            }
            Message::AcceptUpdate => {
                if let Some(release) = self.update_release.clone() {
                    self.update_installing = true;
                    Task::perform(crate::updater::download_installer(release), |result| {
                        Message::UpdateInstallerDownloaded(result.map_err(|e| e.to_string()))
                    })
                } else {
                    Task::none()
                }
            }
            Message::DeclineUpdate => {
                if self.remind_after_new_version {
                    if let Some(release) = &self.update_release {
                        self.settings.ignored_update_version = Some(release.version.clone());
                        self.settings.save();
                    }
                }
                self.update_release = None;
                Task::none()
            }
            Message::RemindAfterNewVersion(on) => {
                self.remind_after_new_version = on;
                Task::none()
            }
            Message::UpdateInstallerDownloaded(result) => {
                self.update_installing = false;
                match result {
                    Ok(path) => {
                        #[cfg(windows)]
                        let mut installer = std::process::Command::new(path);
                        installer.arg("/SILENT");
                        if let Ok(exe) = std::env::current_exe() {
                            if let Some(dir) = exe.parent() {
                                installer.arg(format!("/DIR={}", dir.display()));
                            }
                        }
                        match installer.spawn() {
                            Ok(_) => return Task::done(Message::WindowClose),
                            Err(error) => self.show_toast(
                                format!("Couldn't start the installer: {error}"),
                                ToastKind::Error,
                            ),
                        }
                        #[cfg(not(windows))]
                        {
                            let _ = std::fs::remove_file(path);
                            if let Some(release) = &self.update_release {
                                let _ = open::that(&release.page_url);
                            }
                        }
                    }
                    Err(error) => self.show_toast(
                        format!("Couldn't download the update: {error}"),
                        ToastKind::Error,
                    ),
                }
                Task::none()
            }
            Message::SavePrefs => {
                self.settings.save();
                Task::none()
            }
            Message::ZoomStep(step) => self.zoom_step(step),
            Message::CloseButton => {
                if self.settings.close_minimizes {
                    self.update(Message::WindowMinimize)
                } else {
                    self.update(Message::WindowClose)
                }
            }
            Message::OpenStorageFolder => {
                let _ = open::that(crate::config::cache_dir());
                Task::none()
            }
            Message::RemoveAllDownloads => {
                if !self.confirm_remove_downloads {
                    self.confirm_remove_downloads = true;
                    return Task::none();
                }
                self.confirm_remove_downloads = false;
                // a download still running stops first
                if let Some(cancel) = &self.offline_batch_cancel {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                let ids: Vec<i64> = self.offline_store.tracks.iter().map(|t| t.id).collect();
                self.offline_store.tracks.clear();
                self.offline_store.playlists.clear();
                self.downloaded_playlist_ids.clear();
                self.save_downloaded_playlists();
                self.save_offline_store();
                // the playing track's file is open: it stays, as cache
                let playing = self.playing_id;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut freed = 0u64;
                            for id in ids.into_iter().filter(|id| Some(*id) != playing) {
                                for path in [
                                    crate::config::cached_audio_path(id),
                                    crate::config::cached_waveform_path(id),
                                ] {
                                    let len = std::fs::metadata(&path).map(|m| m.len());
                                    if let (Ok(len), Ok(())) = (len, std::fs::remove_file(&path)) {
                                        freed += len;
                                    }
                                }
                            }
                            freed
                        })
                        .await
                        .unwrap_or(0)
                    },
                    Message::DownloadsRemoved,
                )
            }
            Message::DownloadsRemoved(freed) => {
                self.downloaded_track_ids = App::scan_downloaded_track_ids();
                self.show_toast(
                    format!("Downloads removed ({})", fmt_bytes(freed)),
                    ToastKind::Success,
                );
                self.measure_storage()
            }
            Message::ToggleLibraryCollapsed => {
                self.library_collapsed = !self.library_collapsed;
                Task::none()
            }
            Message::ListWindow(key, first, last) => {
                self.list_windows.insert(key, (first, last));
                Task::none()
            }
            Message::LibraryFilterPicked(filter) => {
                self.library_filter = (self.library_filter != Some(filter)).then_some(filter);
                Task::none()
            }
            Message::SidebarCreatePlaylist => {
                if !self.state.authenticated {
                    self.login_error = Some("Log in first to manage playlists".into());
                    return Task::none();
                }
                self.sidebar_create_mode = true;
                self.sidebar_create_title.clear();
                Task::none()
            }
            Message::SidebarCreatePlaylistTitle(s) => {
                self.sidebar_create_title = s;
                Task::none()
            }
            Message::SidebarCreatePlaylistSubmit => {
                let title = self.sidebar_create_title.trim().to_string();
                if title.is_empty() {
                    self.sidebar_create_mode = false;
                    return Task::none();
                }
                self.sidebar_create_mode = false;
                self.sidebar_create_title.clear();
                Task::perform(do_create_playlist(title, None), |r| {
                    Message::PlaylistActionDone(r.map_err(|e| e.to_string()))
                })
            }
            Message::SidebarCreatePlaylistCancel => {
                self.sidebar_create_mode = false;
                self.sidebar_create_title.clear();
                Task::none()
            }
            Message::PlaylistsLoaded(gen, Ok(pls)) => {
                // A response fetched before the last commit (or while one is
                // still writing) holds pre-write server state and would undo
                // the optimistic lists; SavesApplied refetches afterwards.
                if gen < self.playlists_gen || self.saves_in_flight > 0 {
                    crate::log!(
                        "my playlists: dropped stale response (gen {gen} < {})",
                        self.playlists_gen
                    );
                    return Task::none();
                }
                crate::log!("my playlists loaded: {}", pls.len());
                self.my_playlists = pls;
                self.offline_store.my_playlists = self.my_playlists.clone();
                self.save_offline_store();
                self.sync_add_popover();
                Task::none()
            }
            Message::PlaylistsLoaded(_, Err(e)) => {
                crate::log!("load my playlists failed: {e}");
                // Only surfaced where the list is on screen: the silent
                // startup prefetch must not flag the session as broken.
                if self.add_popover.is_some() {
                    self.show_toast(
                        format!("Couldn't load your playlists: {e}"),
                        ToastKind::Error,
                    );
                }
                Task::none()
            }
            Message::FollowingsLoaded(Ok(users)) => {
                crate::log!("my followings loaded: {}", users.len());
                let avatar_urls: Vec<String> =
                    users.iter().filter_map(|u| u.avatar_url.clone()).collect();
                self.my_followings = users;
                sort_followings(&mut self.my_followings, self.settings.artist_sort);
                self.offline_store.my_followings = self.my_followings.clone();
                self.save_offline_store();
                if !avatar_urls.is_empty() {
                    Task::perform(fetch_artwork(avatar_urls), Message::ArtworkLoaded)
                } else {
                    Task::none()
                }
            }
            Message::FollowingsLoaded(Err(e)) => {
                crate::log!("load followings failed: {e}");
                Task::none()
            }
            Message::RequestDeletePlaylist(id, title) => {
                self.action_menu = None;
                self.menu_anchor = None;
                self.delete_playlist_confirm = Some((id, title));
                Task::none()
            }
            Message::CancelDeletePlaylist => {
                self.delete_playlist_confirm = None;
                Task::none()
            }
            Message::DeletePlaylist(id) => {
                self.delete_playlist_confirm = None;
                self.my_playlists.retain(|p| p.id != id);
                self.liked_playlists.retain(|p| p.id != id);
                self.offline_store.my_playlists.retain(|p| p.id != id);
                self.offline_store.liked_playlists.retain(|p| p.id != id);
                self.save_offline_store();

                if self.current_playlist.as_ref().map(|p| p.id) == Some(id) {
                    self.current_playlist = None;
                    self.tab = Tab::Library;
                }
                Task::perform(do_delete_playlist(id), move |r| {
                    Message::PlaylistDeleted(id, r.map_err(|e| e.to_string()))
                })
            }
            Message::PlaylistDeleted(id, res) => match res {
                Ok(()) => {
                    crate::log!("successfully deleted playlist {id}");
                    // gone for good: so is its downloaded copy
                    self.offline_store.playlists.retain(|p| p.id != id);
                    self.downloaded_playlist_ids.remove(&id.to_string());
                    self.save_offline_store();
                    self.save_downloaded_playlists();
                    self.show_toast("Playlist deleted", ToastKind::Success);
                    self.fetch_playlists_task()
                }
                Err(e) => {
                    crate::log!("failed to delete playlist {id}: {e}");
                    self.show_toast(format!("Failed to delete playlist: {e}"), ToastKind::Error);
                    self.fetch_playlists_task()
                }
            },
            Message::SaveCurrentClicked => match self.playing_track() {
                Some(track) => self.update(Message::SaveTrackClicked(track, MenuAnchor::PlayerBar)),
                None => Task::none(),
            },
            Message::SaveTrackClicked(track, anchor) => {
                let id = track.id;
                if !self.state.authenticated {
                    self.show_toast("Log in first to save tracks", ToastKind::Error);
                    return Task::none();
                }
                if self.is_saved(id) {
                    return self.update(Message::OpenAddPopover(track, anchor));
                }
                // Spotify: the first click saves straight to Liked Songs and
                // the toast offers "Change", which opens the playlist picker.
                crate::log!("save: track {id} -> liked tracks");
                self.set_liked(id, true, Some(track.clone()));
                self.show_toast_action(
                    "Added to Liked Tracks.",
                    ToastKind::Success,
                    "Change",
                    Message::OpenAddPopover(track, anchor),
                );
                // via the commit path: SavesApplied takes the like back if it fails
                Task::perform(
                    do_apply_saves(id, Some(true), Vec::new(), Vec::new(), None),
                    Message::SavesApplied,
                )
            }
            Message::OpenAddPopover(track, anchor) => {
                self.add_popover_anchor = anchor;
                if !self.state.authenticated {
                    self.show_toast("Log in first to manage playlists", ToastKind::Error);
                    return Task::none();
                }
                let liked = self.liked_ids.contains(&track.id);
                let picked: std::collections::HashSet<i64> = self
                    .own_playlists()
                    .filter(|p| playlist_has(p, track.id))
                    .map(|p| p.id)
                    .collect();
                // the "Change" toast gives way to the picker, as in Spotify
                self.toast = None;
                self.show_user_menu = false;
                self.speed_popup = None;
                self.add_popover = Some(AddPopover {
                    track,
                    query: String::new(),
                    new_name: None,
                    liked,
                    liked_was: liked,
                    picked_was: picked.clone(),
                    picked,
                });
                // refresh membership in the background; untouched rows follow it
                self.fetch_playlists_task()
            }
            Message::CloseAddPopover => {
                self.add_popover = None;
                Task::none()
            }
            Message::OpenAddPopoverCurrent => match self.playing_track() {
                Some(track) => self.update(Message::OpenAddPopover(track, MenuAnchor::PlayerBar)),
                None => Task::none(),
            },
            Message::OpenSpeedPopup => {
                self.add_popover = None;
                self.toast = None;
                self.show_user_menu = false;
                self.speed_wheel_acc = 0.0;
                self.speed_popup = Some(format!("{:.2}", self.playback_speed));
                // the field takes the keyboard at once: type a value, Enter
                Task::batch([
                    text_input::focus(speed_input_id()),
                    text_input::select_all(speed_input_id()),
                ])
            }
            Message::CloseSpeedPopup => {
                self.speed_popup = None;
                Task::none()
            }
            Message::SpeedSlider(v) => {
                self.set_speed_quiet(v, true);
                Task::none()
            }
            Message::SpeedWheel(lines) => {
                self.speed_wheel_acc += lines;
                let steps = self.speed_wheel_acc.trunc();
                if steps == 0.0 {
                    return Task::none();
                }
                self.speed_wheel_acc -= steps;
                // Step to the next 0.05x mark in the wheel's direction, so a
                // value set by the slider (1.13x) goes to 1.15x / 1.10x first.
                let pos = self.playback_speed / SPEED_STEP;
                let base = if steps > 0.0 {
                    (pos + 1e-3).floor()
                } else {
                    (pos - 1e-3).ceil()
                };
                self.set_speed_quiet((base + steps) * SPEED_STEP, true);
                Task::none()
            }
            Message::SpeedInput(raw) => {
                // digits and one decimal mark ("," accepted for "1,5")
                let mut seen_mark = false;
                let clean: String = raw
                    .chars()
                    .filter(|c| {
                        if c.is_ascii_digit() {
                            true
                        } else if (*c == '.' || *c == ',') && !seen_mark {
                            seen_mark = true;
                            true
                        } else {
                            false
                        }
                    })
                    .take(5)
                    .collect();
                let live = parse_speed(&clean).filter(|v| (SPEED_MIN..=SPEED_MAX).contains(v));
                if let Some(field) = self.speed_popup.as_mut() {
                    *field = clean;
                }
                if let Some(v) = live {
                    self.set_speed_quiet(v, false);
                }
                Task::none()
            }
            Message::SpeedInputSubmit => {
                match self.speed_popup.as_deref().and_then(parse_speed) {
                    // out-of-range values are clamped (5 -> 2.00x)
                    Some(v) => {
                        self.set_speed_quiet(v, false);
                        self.speed_popup = None;
                    }
                    None => {
                        let v = self.playback_speed;
                        if let Some(field) = self.speed_popup.as_mut() {
                            *field = format!("{v:.2}");
                        }
                    }
                }
                Task::none()
            }
            Message::SpeedReset => {
                self.set_speed_quiet(1.0, true);
                Task::none()
            }
            Message::AddPopoverQuery(q) => {
                if let Some(pop) = self.add_popover.as_mut() {
                    pop.query = q;
                }
                Task::none()
            }
            Message::AddPopoverToggleLiked => {
                if let Some(pop) = self.add_popover.as_mut() {
                    pop.liked = !pop.liked;
                }
                Task::none()
            }
            Message::AddPopoverToggle(pid) => {
                if let Some(pop) = self.add_popover.as_mut() {
                    if !pop.picked.remove(&pid) {
                        pop.picked.insert(pid);
                    }
                }
                Task::none()
            }
            Message::AddPopoverDone => self.apply_add_popover(None),
            Message::AddPopoverNewPlaylist => {
                // a name first (the track's title to start with, selected so
                // typing replaces it); Enter or Create makes the playlist
                let prefer = self.settings.prefer_artist_from_name;
                let Some(pop) = self.add_popover.as_mut() else {
                    return Task::none();
                };
                let (_, title) = pop.track.display_artist_and_title(prefer);
                pop.new_name = Some(if title.trim().is_empty() {
                    "New playlist".to_string()
                } else {
                    title
                });
                Task::batch([
                    text_input::focus(new_playlist_input_id()),
                    text_input::select_all(new_playlist_input_id()),
                ])
            }
            Message::AddPopoverNewName(name) => {
                if let Some(pop) = self.add_popover.as_mut() {
                    pop.new_name = Some(name);
                }
                Task::none()
            }
            Message::AddPopoverCreate => {
                let name = self
                    .add_popover
                    .as_ref()
                    .and_then(|p| p.new_name.as_deref())
                    .map(str::trim)
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    return Task::none();
                }
                self.apply_add_popover(Some(name))
            }
            Message::SavesApplied(rep) => {
                crate::log!("saves applied: {rep:?}");
                if !rep.like_ok {
                    // put the Liked Tracks state back so the button tells the truth
                    if let Some(liked) = rep.like {
                        self.set_liked(rep.track_id, !liked, None);
                    }
                    // an open picker for this track follows, unless the user
                    // already changed its Liked row
                    let now = self.liked_ids.contains(&rep.track_id);
                    if let Some(pop) = self.add_popover.as_mut() {
                        if pop.track.id == rep.track_id && pop.liked == pop.liked_was {
                            pop.liked = now;
                            pop.liked_was = now;
                        }
                    }
                }
                if !rep.errors.is_empty() {
                    self.write_failed("Couldn't save", &rep.errors.join("; "));
                }
                if rep.playlists_touched {
                    self.saves_in_flight = self.saves_in_flight.saturating_sub(1);
                    // anything fetched before this point predates the writes
                    self.playlists_gen += 1;
                    if self.saves_in_flight == 0 {
                        return self.fetch_playlists_task();
                    }
                }
                Task::none()
            }
            Message::EscapePopups => {
                if self.wave_context_frac.is_some() {
                    self.wave_context_frac = None;
                    self.wave_context_comment.clear();
                } else if self.add_popover.is_some() {
                    self.add_popover = None;
                } else if self.speed_popup.is_some() {
                    self.speed_popup = None;
                }
                Task::none()
            }
            Message::PlaylistActionDone(Ok(msg)) => {
                crate::log!("playlist action ok: {msg}");
                self.show_toast(msg, ToastKind::Success);
                self.fetch_playlists_task()
            }
            Message::PlaylistActionDone(Err(e)) => {
                crate::log!("playlist action failed: {e}");
                self.write_failed("Playlist failed", &e);
                Task::none()
            }
            Message::OpenExternalLink(url) => {
                let _ = open::that(&url);
                Task::none()
            }
            Message::OpenActionMenu(menu, anchor) => {
                // the same "..." again closes its menu
                let reopen = self.action_menu.is_some() && self.menu_anchor == Some(anchor);
                self.add_popover = None;
                if reopen {
                    self.action_menu = None;
                    self.menu_anchor = None;
                } else {
                    self.action_menu = Some(menu);
                    self.menu_anchor = Some(anchor);
                }
                Task::none()
            }
            Message::CloseActionMenu => {
                self.action_menu = None;
                self.menu_anchor = None;
                Task::none()
            }
            Message::MenuPick(msg) => {
                let from_story = matches!(
                    self.menu_anchor,
                    Some(MenuAnchor::StoryMore | MenuAnchor::Story)
                ) && self.active_story_index.is_some();
                self.action_menu = None;
                self.menu_anchor = None;
                // A story plays in a one-track queue that closing it throws
                // away: a radio, a queued track or a page opened from its
                // menu must come after it closes, or they'd be undone (or
                // open behind it). Copy, like, repost, download stay put.
                let keeps_story = matches!(
                    *msg,
                    Message::CopyTrackLink(_)
                        | Message::LikeTrack(_)
                        | Message::TrackRepostToggle(_)
                        | Message::DownloadTrack(_)
                        | Message::DownloadTrackOffline(_)
                );
                if from_story && !keeps_story {
                    return self.update(Message::CloseStoryThen(msg));
                }
                self.update(*msg)
            }
            Message::PlayCollection(c, shuffle) => {
                self.action_menu = None;
                if !shuffle && self.collection_playing(c) {
                    // same toggle as the player bar (handles a finished track)
                    return self.update(Message::PlayerToggle);
                }
                let tracks = self.collection_tracks(c).to_vec();
                if tracks.is_empty() {
                    if let Collection::Profile(id) = c {
                        // nothing listed yet: the artist's radio instead
                        let name = self
                            .profile
                            .as_ref()
                            .map(|p| p.username.clone())
                            .unwrap_or_default();
                        return self.update(Message::StartArtistRadio(id, name));
                    }
                    self.show_toast("Nothing to play yet", ToastKind::Info);
                    return Task::none();
                }
                // Shuffle Play turns shuffle on; with it on, the first track
                // is a random one too (and the rest follow shuffled)
                self.shuffle = shuffle || self.shuffle;
                let start = if self.shuffle {
                    use rand::Rng;
                    rand::thread_rng().gen_range(0..tracks.len())
                } else {
                    0
                };
                let pos = self.set_queue(tracks, start);
                self.play_index(pos)
            }
            Message::QueueCollection(c) => {
                self.action_menu = None;
                let tracks = self.collection_tracks(c).to_vec();
                if tracks.is_empty() {
                    return Task::none();
                }
                let n = tracks.len();
                self.show_toast(format!("Added {n} tracks to queue"), ToastKind::Success);
                if self.queue.is_empty() {
                    let pos = self.set_queue(tracks, 0);
                    return self.play_index(pos);
                }
                self.queue.extend(tracks);
                Task::none()
            }
            Message::PlayerBarHover(link) => {
                self.pb_hover = link;
                Task::none()
            }
            Message::RowArtistHover(row) => {
                self.row_artist_hover = row;
                Task::none()
            }
            Message::TrackRowHover(row) => {
                self.hovered_track_row = row;
                Task::none()
            }
            Message::ToggleTrackDescription(track_id) => {
                self.expanded_description_track =
                    if self.expanded_description_track == Some(track_id) {
                        None
                    } else {
                        Some(track_id)
                    };
                Task::none()
            }
            Message::CopyTrackLink(track) => {
                let url = track
                    .permalink_url
                    .clone()
                    .unwrap_or_else(|| format!("https://soundcloud.com/tracks/{}", track.id));
                self.show_toast("Track link copied to clipboard!", ToastKind::Success);
                clipboard::write(url)
            }
            Message::CopyPlaylistLink(id, permalink) => {
                let url =
                    permalink.unwrap_or_else(|| format!("https://soundcloud.com/playlists/{}", id));
                self.show_toast("Playlist link copied to clipboard!", ToastKind::Success);
                clipboard::write(url)
            }
            Message::CopyProfileLink(id, username, permalink) => {
                let url = permalink
                    .map(|p| format!("https://soundcloud.com/{}", p))
                    .unwrap_or_else(|| {
                        if !username.is_empty() {
                            format!(
                                "https://soundcloud.com/{}",
                                username.trim_start_matches('@')
                            )
                        } else {
                            format!("https://soundcloud.com/users/{}", id)
                        }
                    });
                self.show_toast("Artist link copied to clipboard!", ToastKind::Success);
                clipboard::write(url)
            }
            Message::AddToQueue(track) => {
                let title = track.title.clone();
                if self.queue.is_empty() {
                    let pos = self.set_queue(vec![track], 0);
                    let play = self.play_index(pos);
                    self.show_toast(
                        format!("Playing '{}'", trunc(&title, 24)),
                        ToastKind::Success,
                    );
                    return play;
                } else {
                    self.queue.push(track);
                    self.show_toast(
                        format!("Added '{}' to queue", trunc(&title, 24)),
                        ToastKind::Success,
                    );
                }
                Task::none()
            }
        }
    }
}

/// The player's keyboard shortcuts:
///   Space / K        play or pause
///   Left / Right     seek 5 s back / forward (with Shift: 15 s)
///   Ctrl+Left/Right  previous / next track
///   Up / Down        volume
///   N / P            next / previous track
///   M  mute, L  like, S  shuffle, R  repeat
///   Alt+Left/Right   back / forward between pages
/// Letters go by physical key, so the layout doesn't matter. Media keys are
/// the global hook's job once it runs (see media_keys), else handled here.
/// --debug: what the app is asked to do, one short line per message.
/// Ticks, hovers, the player's status and typing are left out (too many, or
/// what someone typed); results are cut, so a page of tracks is one line.
fn debug_message(msg: &Message) {
    // Frequent or bulky messages aren't logged, and aren't formatted either:
    // Debug of an artwork batch or a liked-tracks list is megabytes (their
    // handlers log a summary line instead).
    if matches!(
        msg,
        Message::Tick
            | Message::AnimTick
            | Message::WindowResized(_)
            | Message::ListWindow(..)
            | Message::SettingsSearch(_)
            | Message::PlayerBarHover(_)
            | Message::RowArtistHover(_)
            | Message::TrackRowHover(_)
            | Message::WaveHover(_)
            | Message::ArtworkLoaded(_)
            | Message::ArtworkReady(_)
            | Message::ReactionsPoll
            | Message::Noop
            | Message::PlayerVolume(_)
            | Message::SpeedSlider(_)
            | Message::Yt(_)
            | Message::SearchChanged(_)
            | Message::CookieInput(_)
            | Message::CookiePasted(_)
            | Message::TrackPageCommentInput(_)
            | Message::SidebarCreatePlaylistTitle(_)
            | Message::AddPopoverQuery(_)
            | Message::AddPopoverNewName(_)
            | Message::SpeedInput(_)
            | Message::WaveLoaded(_)
            | Message::InitWindowId(_)
            | Message::InitWindowScale(_)
            | Message::HomeLoaded(_)
            | Message::LibraryLoaded(_)
            | Message::SearchLoaded(..)
            | Message::SearchMoreLoaded(..)
            | Message::PlaylistLoaded(..)
            | Message::ProfileLoaded(..)
            | Message::CommentsLoaded(_)
            | Message::ReactionsLoaded(..)
    ) {
        return;
    }
    let text = format!("{msg:?}");
    let cut: String = text.chars().take(300).collect();
    crate::log!(
        "ui: {cut}{}",
        if cut.len() < text.len() { " …" } else { "" }
    );
}

fn shortcut(
    key: &Key,
    physical: &keyboard::key::Physical,
    m: keyboard::Modifiers,
) -> Option<Message> {
    use keyboard::key::{Code, Physical};
    let ctrl = m.control() || m.logo();
    if let Key::Named(named) = key {
        let hooked = crate::media_keys::HOOKED.load(std::sync::atomic::Ordering::Relaxed);
        return match named {
            Named::Space => Some(Message::PlayerToggle),
            Named::ArrowLeft if m.alt() => Some(Message::NavBack),
            Named::ArrowRight if m.alt() => Some(Message::NavForward),
            Named::ArrowLeft if ctrl => Some(Message::PrevTrack),
            Named::ArrowRight if ctrl => Some(Message::NextTrack),
            Named::ArrowLeft => Some(Message::SeekRelative(if m.shift() {
                -15_000
            } else {
                -5_000
            })),
            Named::ArrowRight => Some(Message::SeekRelative(if m.shift() {
                15_000
            } else {
                5_000
            })),
            Named::ArrowUp => Some(Message::VolumeRelative(0.05)),
            Named::ArrowDown => Some(Message::VolumeRelative(-0.05)),
            Named::Escape => Some(Message::CloseModals),
            Named::BrowserBack => Some(Message::NavBack),
            Named::BrowserForward => Some(Message::NavForward),
            Named::MediaPlayPause if !hooked => Some(Message::PlayerToggle),
            Named::MediaStop if !hooked => Some(Message::PlayerPause),
            Named::MediaTrackNext if !hooked => Some(Message::NextTrack),
            Named::MediaTrackPrevious if !hooked => Some(Message::PrevTrack),
            _ => None,
        };
    }
    let Physical::Code(code) = physical else {
        return None;
    };
    match code {
        Code::KeyV if ctrl => Some(Message::PasteShortcut),
        Code::Minus | Code::NumpadSubtract if ctrl => Some(Message::ZoomStep(-1)),
        Code::Equal | Code::NumpadAdd if ctrl => Some(Message::ZoomStep(1)),
        Code::Digit0 | Code::Numpad0 if ctrl => Some(Message::ZoomStep(0)),
        _ if ctrl || m.alt() => None,
        Code::KeyK => Some(Message::PlayerToggle),
        Code::KeyM => Some(Message::ToggleMute),
        Code::KeyL => Some(Message::LikeCurrent),
        Code::KeyS => Some(Message::ToggleShuffle),
        Code::KeyR => Some(Message::ToggleRepeat),
        Code::KeyN => Some(Message::NextTrack),
        Code::KeyP => Some(Message::PrevTrack),
        _ => None,
    }
}

// ---------- view helpers ----------

fn t_color(c: Color) -> text::Style {
    text::Style { color: Some(c) }
}
fn dim() -> text::Style {
    t_color(TEXT_DIM)
}
fn muted() -> text::Style {
    t_color(TEXT_MUTED)
}
fn orange_t() -> text::Style {
    t_color(ORANGE)
}
fn bright() -> text::Style {
    t_color(TEXT)
}

fn round(r: f32) -> Border {
    Border {
        radius: border::Radius::from(r),
        width: 0.0,
        color: Color::TRANSPARENT,
    }
}

fn glass_border(r: f32) -> Border {
    Border {
        radius: border::Radius::from(r),
        width: 0.0,
        color: Color::TRANSPARENT,
    }
}

fn pad4(top: f32, right: f32, bottom: f32, left: f32) -> Padding {
    Padding {
        top,
        right,
        bottom,
        left,
    }
}

fn panel(bg: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(bg)),
        border: glass_border(radius),
        shadow: Shadow::default(),
        ..container::Style::default()
    }
}

fn cover_color(id: i64) -> Color {
    let h = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let r = 0.25 + ((h >> 0) & 0xFF) as f32 / 255.0 * 0.55;
    let g = 0.18 + ((h >> 8) & 0xFF) as f32 / 255.0 * 0.45;
    let b = 0.22 + ((h >> 16) & 0xFF) as f32 / 255.0 * 0.55;
    Color::from_rgb(r.min(0.95), g.min(0.9), b.min(0.95))
}

fn initials(s: &str) -> String {
    let clean = s.trim_start_matches('@').trim();
    let mut chars = clean.chars().filter(|c| c.is_alphanumeric());
    let a = chars.next().unwrap_or('S');
    let b = chars.next().unwrap_or('C');
    format!("{a}{b}").to_uppercase()
}

fn parse_trailing_id(s: &str) -> i64 {
    s.rsplit(':')
        .next()
        .and_then(|x| x.parse::<i64>().ok())
        .unwrap_or(0)
}

/// A track's genre and tags, as soundcloud.com lists them: the genre first,
/// then `tag_list` (space-separated, several words in quotes), without
/// machine tags ("soundcloud:source=...") or repeats.
fn track_tags(t: &Track) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |tag: &str| {
        let tag = tag.trim().trim_start_matches('#').trim();
        if tag.is_empty() || tag.contains(':') || tag.contains('=') {
            return;
        }
        if !out.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            out.push(tag.to_string());
        }
    };
    if let Some(g) = t.genre.as_deref() {
        push(g);
    }
    let list = t.tag_list.as_deref().unwrap_or("");
    let mut rest = list;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if let Some(q) = rest.strip_prefix('"') {
            let end = q.find('"').unwrap_or(q.len());
            push(&q[..end]);
            rest = q.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            push(&rest[..end]);
            rest = &rest[end..];
        }
    }
    out.truncate(8);
    out
}

/// "4 days ago" for an epoch-ms time; None when it's unknown (0).
fn time_ago(ms: u64) -> Option<String> {
    if ms == 0 {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let s = now.saturating_sub(ms) / 1000;
    let (n, unit) = match s {
        0..=59 => return Some("just now".to_string()),
        60..=3_599 => (s / 60, "minute"),
        3_600..=86_399 => (s / 3_600, "hour"),
        86_400..=2_591_999 => (s / 86_400, "day"),
        2_592_000..=31_535_999 => (s / 2_592_000, "month"),
        _ => (s / 31_536_000, "year"),
    };
    Some(format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" }))
}

/// A text link: underlined and in `hover_color` while `hovered`; the hover
/// is tracked by the app (`enter` / `exit`), since a text knows no hover.
///
/// The click must be the span's own link: iced's rich text captures a press
/// on any span, so a `mouse_area` around it never sees a click on the words
/// (only on its padding). The `mouse_area` keeps the hover and the pointer,
/// and still catches a press that lands between the glyphs.
fn hover_link<'a>(
    label: String,
    size: u16,
    color: Color,
    hover_color: Color,
    hovered: bool,
    press: Message,
    enter: Message,
    exit: Message,
) -> Element<'a, Message> {
    iced::widget::mouse_area(link_text(
        label,
        size,
        color,
        hover_color,
        hovered,
        Some(press.clone()),
    ))
    .on_press(press)
    .on_enter(enter)
    .on_exit(exit)
    .interaction(mouse::Interaction::Pointer)
    .into()
}

/// One-line rich text, underlined while hovered, clickable through its span
/// (see hover_link for why the span itself must carry the message).
fn link_text<'a>(
    label: String,
    size: u16,
    color: Color,
    hover_color: Color,
    hovered: bool,
    press: Option<Message>,
) -> iced::widget::text::Rich<'a, Message> {
    let span = iced::widget::span::<Message, iced::Font>(label)
        .underline(hovered)
        .color(if hovered { hover_color } else { color });
    let span = match press {
        Some(msg) => span.link(msg),
        None => span,
    };
    iced::widget::rich_text([span])
        .size(size)
        .wrapping(text::Wrapping::None)
}

/// Every label is built with Advanced shaping. iced's default (Basic) does no
/// font fallback at all, so anything Segoe UI lacks — the ☆ in a username,
/// emoji, CJK titles — rendered as a tofu box. Advanced falls back through
/// Segoe UI Symbol / Emoji and the system CJK fonts.
fn text<'a>(fragment: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    iced::widget::text(fragment).shaping(text::Shaping::Advanced)
}

/// Segoe UI's tall ascent (1.08em) places glyphs about 0.16em below the
/// centre of their line box, so a text block centred beside an icon or a
/// cover reads ~2px low (measured on screen). Padding the bottom by twice
/// that offset lifts the block back onto the row's true centre line.
fn optical_center<'a>(
    content: impl Into<Element<'a, Message>>,
    font_px: f32,
) -> iced::widget::Container<'a, Message> {
    container(content).padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: (font_px * 0.30).round(),
        left: 0.0,
    })
}

/// Segoe UI advance widths (em) for ASCII U+0020..=U+007E, measured from segoeui.ttf.
const SEGOE_ASCII: [f32; 95] = [
    0.274, 0.284, 0.392, 0.591, 0.539, 0.818, 0.800, 0.230, 0.302, 0.302, 0.417, 0.684, 0.217,
    0.400, 0.217, 0.390, 0.539, 0.539, 0.539, 0.539, 0.539, 0.539, 0.539, 0.539, 0.539, 0.539,
    0.217, 0.217, 0.684, 0.684, 0.684, 0.448, 0.955, 0.645, 0.573, 0.619, 0.701, 0.506, 0.488,
    0.686, 0.710, 0.266, 0.357, 0.580, 0.471, 0.898, 0.748, 0.754, 0.560, 0.754, 0.598, 0.531,
    0.524, 0.687, 0.621, 0.934, 0.590, 0.553, 0.570, 0.302, 0.379, 0.302, 0.684, 0.415, 0.268,
    0.509, 0.588, 0.462, 0.589, 0.523, 0.313, 0.589, 0.566, 0.242, 0.242, 0.497, 0.242, 0.861,
    0.566, 0.586, 0.588, 0.589, 0.348, 0.424, 0.339, 0.566, 0.479, 0.723, 0.459, 0.484, 0.452,
    0.302, 0.239, 0.302, 0.684,
];
/// Segoe UI advance widths (em) for Cyrillic U+0410..=U+044F, measured from segoeui.ttf.
const SEGOE_CYR: [f32; 64] = [
    0.645, 0.572, 0.573, 0.472, 0.693, 0.506, 0.867, 0.540, 0.749, 0.749, 0.580, 0.673, 0.898,
    0.710, 0.754, 0.713, 0.560, 0.619, 0.524, 0.567, 0.727, 0.590, 0.742, 0.661, 0.949, 0.980,
    0.706, 0.783, 0.576, 0.616, 1.019, 0.591, 0.509, 0.579, 0.530, 0.383, 0.547, 0.523, 0.746,
    0.446, 0.581, 0.581, 0.497, 0.527, 0.702, 0.577, 0.586, 0.577, 0.588, 0.462, 0.410, 0.484,
    0.686, 0.459, 0.600, 0.565, 0.800, 0.824, 0.591, 0.709, 0.504, 0.462, 0.813, 0.503,
];

/// Advance width of one glyph in the UI font as a fraction of the font size.
/// ASCII and Cyrillic come from the exact tables above; everything else falls
/// back by class (symbols like ☆ and CJK render from a fallback font at about
/// a full em). Used to fit single-line labels before cutting them.
fn glyph_em(c: char) -> f32 {
    let o = c as u32;
    if (0x20..=0x7E).contains(&o) {
        return SEGOE_ASCII[(o - 0x20) as usize];
    }
    if (0x410..=0x44F).contains(&o) {
        return SEGOE_CYR[(o - 0x410) as usize];
    }
    match c {
        'ё' => 0.523,
        'Ё' => 0.506,
        '•' => 0.406,
        '·' => 0.217,
        '–' => 0.5,
        '—' => 1.0,
        '…' => 0.733,
        '«' | '»' => 0.506,
        '№' => 1.122,
        '\u{1100}'..='\u{115F}'
        | '\u{2000}'..='\u{2BFF}'
        | '\u{2E80}'..='\u{A4CF}'
        | '\u{AC00}'..='\u{D7A3}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{FE30}'..='\u{FE4F}'
        | '\u{FF00}'..='\u{FF60}'
        | '\u{1F000}'..='\u{1FAFF}' => 1.0,
        c if c.is_uppercase() => 0.66,
        _ => 0.56,
    }
}

/// Estimated rendered width of `s` at `font_px`.
fn text_px(s: &str, font_px: f32) -> f32 {
    s.chars().map(glyph_em).sum::<f32>() * font_px
}

/// Cut `s` so it fits `max_px` at `font_px`, ending in "…" when cut.
fn trunc_px(s: &str, max_px: f32, font_px: f32) -> String {
    let max_px = max_px * 0.97; // margin for kerning/hinting vs. the estimate
    if text_px(s, font_px) <= max_px {
        return s.to_string();
    }
    let mut out = String::new();
    let mut acc = font_px * 0.73; // room for the ellipsis (0.73em in Segoe UI)
    for c in s.chars() {
        let cw = glyph_em(c) * font_px;
        if acc + cw > max_px {
            break;
        }
        acc += cw;
        out.push(c);
    }
    out.push('…');
    out
}

/// Entity-header title size: the largest step of Spotify's scale (never above
/// what hero_title_size allows for this length) at which the whole title fits
/// `max_px`. Only if even the smallest step overflows does the caller cut it.
fn fit_title_px(s: &str, max_px: f32) -> u16 {
    let cap = hero_title_size(s);
    [68u16, 54, 40, 30, 24]
        .into_iter()
        .filter(|&px| px <= cap)
        .find(|&px| text_px(s, f32::from(px)) <= max_px)
        .unwrap_or(24)
}

fn trunc(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        t.push('…');
    }
    t
}

fn clean_username(s: &str) -> &str {
    s.trim().trim_start_matches('@')
}

/// What clicking the artist shown for `t` opens: the uploader's profile when
/// `shown` is the uploader, else the profile found by that name ("Artist -
/// Title" read from the name of a label's or a fan's upload).
fn artist_link_msg(t: &Track, shown: &str) -> Message {
    let Some(user) = t.user.as_ref().filter(|u| u.id != 0) else {
        return Message::ArtistClicked(shown.to_string());
    };
    let key = clean_username(shown).to_lowercase();
    let name = clean_username(&user.username).to_lowercase();
    let permalink = user.permalink.as_deref().unwrap_or("").to_lowercase();
    let same = |a: &str, b: &str| {
        !a.is_empty() && !b.is_empty() && (a == b || (b.len() >= 3 && a.contains(b)))
    };
    if key.is_empty() || same(&key, &name) || same(&name, &key) || same(&key, &permalink) {
        Message::OpenProfile(user.id)
    } else {
        Message::ArtistClicked(shown.to_string())
    }
}

fn pill_btn(label: impl Into<String>, fill: Color, msg: Message) -> Element<'static, Message> {
    button(text(label.into()).size(16).style(|_| bright()))
        .on_press(msg)
        .padding(Padding {
            top: 8.0,
            bottom: 8.0,
            left: 16.0,
            right: 16.0,
        })
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered => {
                    let mut c = fill;
                    c.r = (c.r * 1.08).min(1.0);
                    c.g = (c.g * 1.08).min(1.0);
                    c.b = (c.b * 1.08).min(1.0);
                    Background::Color(c)
                }
                _ => Background::Color(fill),
            };
            button::Style {
                background: Some(bg),
                text_color: TEXT,
                border: round(20.0),
                ..button::Style::default()
            }
        })
        .into()
}

/// Spotify's "announcement" blue (--text-announcement): the badge of a track
/// that plays at its own saved speed.
const SPEED_BADGE_BG: Color = Color::from_rgb(0.325, 0.616, 0.961); // #539DF5
const SPEED_BADGE_H: f32 = 16.0;
/// Space between a title and its speed badge.
const SPEED_BADGE_GAP: f32 = 6.0;

/// "x1.5", "x1.25", "x2": a saved speed with the trailing zeros cut.
fn speed_badge_label(speed: f32) -> String {
    let s = format!("{speed:.2}");
    format!("x{}", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Room a speed badge takes after a title, gap included (bold 11px label
/// plus 5px each side), so the title can be cut short of it.
fn speed_badge_room(label: &str) -> f32 {
    (text_px(label, 11.0) * 1.1 + 10.0).ceil() + SPEED_BADGE_GAP
}

/// Blue badge, black text: this track starts at the speed last set for it.
fn speed_badge(label: String) -> Element<'static, Message> {
    container(optical_center(
        text(label)
            .size(11)
            .font(UI_BOLD)
            .wrapping(text::Wrapping::None)
            .style(|_| t_color(Color::BLACK)),
        11.0,
    ))
    .padding(pad4(0.0, 5.0, 0.0, 5.0))
    .center_y(Length::Fixed(SPEED_BADGE_H))
    .style(|_| container::Style {
        background: Some(Background::Color(SPEED_BADGE_BG)),
        border: round(3.0),
        ..container::Style::default()
    })
    .into()
}

// ---------- Spotify settings controls (open.spotify.com/preferences) ----------

/// Height of every settings control but the switch (--encore-control-size-smaller).
const SET_CTRL_H: f32 = 32.0;
/// SoundCloud orange stands in for Spotify's green; hovered, a touch brighter.
const ORANGE_HOVER: Color = Color::from_rgb(1.0, 0.42, 0.12);

/// Spotify's switch: a 42x24 pill with a 20px white knob, #535353 off
/// (#B3B3B3 hovered), the accent colour on.
fn settings_switch(on: bool, msg: Message) -> Element<'static, Message> {
    let knob = container(iced::widget::Space::new(
        Length::Fixed(20.0),
        Length::Fixed(20.0),
    ))
    .style(|_| container::Style {
        background: Some(Background::Color(Color::WHITE)),
        border: round(10.0),
        ..container::Style::default()
    });
    button(
        container(knob)
            .width(Length::Fixed(42.0))
            .height(Length::Fixed(24.0))
            .padding(2)
            .align_x(if on {
                iced::alignment::Horizontal::Right
            } else {
                iced::alignment::Horizontal::Left
            })
            .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(msg)
    .padding(0)
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(match (on, hovered) {
                (true, false) => ORANGE,
                (true, true) => ORANGE_HOVER,
                (false, false) => Color::from_rgb(0.325, 0.325, 0.325),
                (false, true) => TEXT_DIM,
            })),
            border: round(12.0),
            ..button::Style::default()
        }
    })
    .into()
}

/// Spotify's secondary button: an outline pill with a bold 14px label, the
/// outline white under the pointer. `None`: disabled.
fn settings_outline_btn(label: &str, msg: Option<Message>) -> Element<'static, Message> {
    let enabled = msg.is_some();
    let mut btn = button(
        container(optical_center(
            text(label.to_string())
                .size(14)
                .font(UI_BOLD)
                .wrapping(text::Wrapping::None),
            14.0,
        ))
        .padding(pad4(0.0, 16.0, 0.0, 16.0))
        .center_y(Length::Fixed(SET_CTRL_H)),
    )
    .padding(0)
    .style(move |_, status| {
        let hovered =
            enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: None,
            text_color: if enabled { TEXT } else { TEXT_MUTED },
            border: Border {
                radius: border::Radius::from(SET_CTRL_H / 2.0),
                width: 1.0,
                color: if hovered {
                    TEXT
                } else {
                    Color {
                        a: if enabled { 1.0 } else { 0.5 },
                        ..TEXT_MUTED
                    }
                },
            },
            ..button::Style::default()
        }
    });
    if let Some(msg) = msg {
        btn = btn.on_press(msg);
    }
    btn.into()
}

/// Spotify's primary button: filled with the accent, black bold label.
fn settings_primary_btn(label: &str, msg: Message) -> Element<'static, Message> {
    button(
        container(optical_center(
            text(label.to_string())
                .size(14)
                .font(UI_BOLD)
                .wrapping(text::Wrapping::None),
            14.0,
        ))
        .padding(pad4(0.0, 16.0, 0.0, 16.0))
        .center_y(Length::Fixed(SET_CTRL_H)),
    )
    .on_press(msg)
    .padding(0)
    .style(|_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => ORANGE_HOVER,
            _ => ORANGE,
        })),
        text_color: Color::BLACK,
        border: round(SET_CTRL_H / 2.0),
        ..button::Style::default()
    })
    .into()
}

/// A 32px text field on Spotify's elevated surface.
fn settings_input<'a>(
    placeholder: &str,
    value: &str,
    on_input: fn(String) -> Message,
    on_submit: Option<Message>,
) -> Element<'a, Message> {
    let mut field = text_input(placeholder, value)
        .on_input(on_input)
        .on_paste(on_input)
        .size(14)
        .line_height(text::LineHeight::Absolute(iced::Pixels(18.0)))
        .padding(Padding {
            top: 7.0,
            right: 12.0,
            bottom: 7.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(|_, status| text_input::Style {
            background: Background::Color(match status {
                text_input::Status::Hovered | text_input::Status::Focused => BG_HOVER,
                _ => BG_CARD,
            }),
            border: Border {
                radius: border::Radius::from(4.0),
                width: if matches!(status, text_input::Status::Focused) {
                    1.0
                } else {
                    0.0
                },
                color: TEXT_MUTED,
            },
            icon: TEXT_DIM,
            placeholder: TEXT_MUTED,
            value: TEXT,
            selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
        });
    if let Some(msg) = on_submit {
        field = field.on_submit(msg);
    }
    field.into()
}

/// A setting's name, and under it what it does: Spotify's 14px label (white
/// over a 12px #B3B3B3 description, or #B3B3B3 alone).
fn settings_label<'a>(title: &str, desc: Option<&str>) -> Element<'a, Message> {
    match desc {
        None => text(title.to_string()).size(14).style(|_| dim()).into(),
        Some(desc) => column![
            text(title.to_string()).size(14).style(|_| bright()),
            text(desc.to_string()).size(12).style(|_| dim()),
        ]
        .spacing(4)
        .into(),
    }
}

/// One settings row, and the text "Search in Settings" matches it by:
/// the label in two thirds of the width, the control right-aligned in the
/// last third (Spotify's `grid-template-columns: 2fr 1fr`, 24px apart).
fn settings_row<'a>(
    find: String,
    label: impl Into<Element<'a, Message>>,
    control: impl Into<Element<'a, Message>>,
) -> (String, Element<'a, Message>) {
    let view = row![
        container(label)
            .width(Length::FillPortion(2))
            .align_y(iced::alignment::Vertical::Center),
        container(control)
            .width(Length::FillPortion(1))
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center),
        // every row at least a control tall, so rows keep one rhythm
        iced::widget::Space::new(Length::Fixed(0.0), Length::Fixed(SET_CTRL_H)),
    ]
    .spacing(24)
    .align_y(iced::Alignment::Center);
    (find, view.into())
}

/// A section: its bold 16px heading over the rows the search leaves (all
/// of them when the heading itself matches); None when none are left.
fn settings_section<'a>(
    title: &str,
    rows: Vec<(String, Element<'a, Message>)>,
    query: &str,
) -> Option<Element<'a, Message>> {
    let all = query.is_empty() || title.to_lowercase().contains(query);
    let shown: Vec<Element<'a, Message>> = rows
        .into_iter()
        .filter(|(find, _)| all || find.to_lowercase().contains(query))
        .map(|(_, view)| view)
        .collect();
    if shown.is_empty() {
        return None;
    }
    Some(
        column![
            text(title.to_string())
                .size(16)
                .font(UI_BOLD)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
            iced::widget::Column::with_children(shown).spacing(8),
        ]
        .spacing(8)
        .into(),
    )
}

impl App {
    fn user_card(&self, user: &UserMini, name_max: usize) -> Element<'static, Message> {
        // Fixed geometry: 42px avatar + 6px padding top and bottom = 54px card.
        // Name column is 16*1.3 + 2 + 12*1.3 = 38.4px, inside the avatar height.
        // The Follow / Following pill has one fixed width for both labels, so
        // toggling it never moves the name, and the name column fills and clips.
        // Narrowest host is the 320px inspector (296 inner): 296 - 2*10 pad
        // - 42 avatar - 84 pill - 2*10 spacing = 130px for the name, so
        // floor(130 / (16 * 0.56)) = 14 chars at 16px (+ clip as the safety net).
        const AVATAR: f32 = 42.0;
        const PAD_Y: f32 = 6.0;
        const PAD_X: f32 = 10.0;
        const CARD_H: f32 = AVATAR + PAD_Y * 2.0;
        const FOLLOW_W: f32 = 84.0;
        const FOLLOW_H: f32 = 28.0;

        let is_following = self.my_following_ids.contains(&user.id);
        let uid = user.id;
        let uname = clean_username(&user.username).to_string();

        let follow_btn = button(
            container(
                text(if is_following { "Following" } else { "Follow" })
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| {
                        if is_following {
                            t_color(TEXT_DIM)
                        } else {
                            t_color(Color::BLACK)
                        }
                    }),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        )
        .on_press(Message::FollowUserToggle(uid, is_following))
        .padding(0)
        .width(Length::Fixed(FOLLOW_W))
        .height(Length::Fixed(FOLLOW_H))
        .style(move |_, status| button::Style {
            background: Some(match status {
                button::Status::Hovered => {
                    if is_following {
                        Background::Color(BG_HOVER)
                    } else {
                        Background::Color(Color::WHITE)
                    }
                }
                _ => {
                    if is_following {
                        Background::Color(BG_ELEV)
                    } else {
                        Background::Color(Color::from_rgb(0.9, 0.9, 0.9))
                    }
                }
            }),
            text_color: if is_following { TEXT } else { Color::BLACK },
            border: round(FOLLOW_H / 2.0),
            ..button::Style::default()
        });

        button(
            row![
                self.artwork_tile(
                    user.avatar_url.as_deref(),
                    user.id,
                    clean_username(&user.username),
                    AVATAR,
                    true
                ),
                column![
                    text(trunc(&uname, name_max))
                        .size(16)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                    text("User")
                        .size(12)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                ]
                .spacing(2)
                .width(Length::Fill)
                .clip(true),
                follow_btn,
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::OpenProfile(user.id))
        .padding(pad4(PAD_Y, PAD_X, PAD_Y, PAD_X))
        .width(Length::Fill)
        .height(Length::Fixed(CARD_H))
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            border: round(8.0),
            ..button::Style::default()
        })
        .into()
    }
}

fn cover_tile(id: i64, title: &str, size: f32, round_full: bool) -> Element<'static, Message> {
    let bg = cover_color(id);
    // same corners as a decoded tile, so the placeholder -> art swap doesn't jump
    let r = if round_full {
        size / 2.0
    } else {
        art_radius(art_px(size))
    };
    let label = initials(title);
    let border = Border {
        radius: border::Radius::from(r),
        width: 0.0,
        color: Color::TRANSPARENT,
    };

    container(
        text(label)
            .size((size * 0.28).clamp(12.0, 28.0))
            .style(|_| bright()),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border,
        shadow: Shadow::default(),
        ..container::Style::default()
    })
    .into()
}

impl App {
    fn artwork_tile(
        &self,
        url: Option<&str>,
        id: i64,
        title: &str,
        size: f32,
        round_full: bool,
    ) -> Element<'static, Message> {
        if let Some(url_str) = url {
            let px = art_px(size);
            let key = art_key_hash(url_str, round_full, px);
            self.art_touched.borrow_mut().insert(key);
            if let Some(handle) = self.artwork.get(&key) {
                // Decoded at exactly this size with the rounding baked in, so
                // it maps 1:1 onto screen pixels and needs no wrapper.
                return image(handle.clone())
                    .width(Length::Fixed(size))
                    .height(Length::Fixed(size))
                    .content_fit(iced::ContentFit::Cover)
                    .into();
            }
            // Not decoded yet: queue exactly this (url, shape, size); the next
            // tick picks it up.
            if !self.art_inflight.contains(&key) && !self.art_failed.contains_key(&key) {
                self.art_wanted
                    .borrow_mut()
                    .entry(key)
                    .or_insert_with(|| (url_str.to_string(), round_full, px));
            }
        }
        cover_tile(id, title, size, round_full)
    }

    /// Width of the main content island right now: window minus the sidebar
    /// and gutters (268) and whichever side panels are open.
    /// The rows of a long list to build: the window its virtual list last
    /// asked for (see virtual_rows), within the list's current length.
    fn list_window(&self, key: ListKey, count: usize) -> (usize, usize) {
        /// Enough to fill a tall window before the list reports its view.
        const FIRST_ROWS: usize = 40;
        let (first, last) = self
            .list_windows
            .get(&key)
            .copied()
            .unwrap_or((0, FIRST_ROWS));
        let last = last.min(count);
        if first >= last {
            // the list got shorter (another playlist, a new search)
            return (0, count.min(FIRST_ROWS));
        }
        (first, last)
    }

    /// A long list with only its rows on screen built: `row(i, item)` for
    /// those, spacers for the rest (every row `row_h` tall, `spacing` apart).
    fn virtual_rows<'a, T>(
        &'a self,
        key: ListKey,
        items: &'a [T],
        row_h: f32,
        spacing: f32,
        row: impl Fn(usize, &'a T) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let (first, last) = self.list_window(key, items.len());
        let rows = items[first..last]
            .iter()
            .enumerate()
            .map(|(k, item)| row(first + k, item))
            .collect();
        crate::virtual_list::virtual_list(
            items.len(),
            row_h + spacing,
            spacing,
            (first, last),
            rows,
            move |a, b| Message::ListWindow(key, a, b),
        )
        .into()
    }

    /// Track rows (track_row: 56px, 2px apart) of a long list.
    fn virtual_track_rows<'a>(&'a self, key: ListKey, tracks: &'a [Track]) -> Element<'a, Message> {
        self.virtual_rows(key, tracks, 56.0, 2.0, |i, t| self.track_row(i, t))
    }

    fn content_w(&self) -> f32 {
        let panels: f32 = [
            (self.show_queue, 280.0_f32),
            (self.inspector_track.is_some(), 320.0_f32),
        ]
        .iter()
        .filter(|(open, _)| *open)
        .map(|(_, w)| *w)
        .sum();
        (self.window_size.width - 268.0 - panels).max(320.0)
    }

    /// A playlist card's owner line: their name, a link to their profile,
    /// underlined under the pointer. It sits inside the card's button and
    /// wins the click there (a link span takes its press first).
    fn owner_link(&self, owner: &UserMini, card: &str, max_px: f32) -> Element<'static, Message> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        card.hash(&mut h);
        let which = PbLink::Owner(h.finish());
        hover_link(
            trunc_px(clean_username(&owner.username), max_px, 12.0),
            12,
            TEXT_MUTED,
            TEXT,
            self.pb_hover == Some(which),
            Message::OpenProfile(owner.id),
            Message::PlayerBarHover(Some(which)),
            Message::PlayerBarHover(None),
        )
    }

    /// Artwork that opens the full-resolution photo when clicked.
    fn art_button(
        &self,
        url: Option<&str>,
        id: i64,
        title: &str,
        size: f32,
        round_full: bool,
    ) -> Element<'static, Message> {
        let tile = self.artwork_tile(url, id, title, size, round_full);
        let Some(u) = url else { return tile };
        button(tile)
            .on_press(Message::OpenImageViewer(u.to_string()))
            .padding(0)
            .style(|_, _| button::Style {
                background: None,
                ..button::Style::default()
            })
            .into()
    }

    /// Drain artwork requests recorded during view() and spawn one batched
    /// fetch+decode+mask task for them. Batches are capped so one huge page
    /// (e.g. a 1500-track library) streams in progressively instead of
    /// arriving as a single multi-hundred-MB message.
    fn pump_art(&mut self) -> Task<Message> {
        const ART_BATCH: usize = 64;
        const ART_RETRY: std::time::Duration = std::time::Duration::from_secs(60);

        // At most two batches in flight: each tick would otherwise start
        // another 64 downloads and decodes, and a long page had hundreds
        // running at once, the visible covers queued behind the rest.
        const ART_INFLIGHT_MAX: usize = ART_BATCH * 2;
        if self.art_wanted.borrow().is_empty() || self.art_inflight.len() >= ART_INFLIGHT_MAX {
            return Task::none();
        }
        let now = std::time::Instant::now();
        self.art_failed
            .retain(|_, t| now.duration_since(*t) < ART_RETRY);

        let mut reqs: Vec<ArtKey> = Vec::new();
        let mut keys: Vec<u64> = Vec::new();
        {
            let inflight = &self.art_inflight;
            let failed = &self.art_failed;
            self.art_wanted.borrow_mut().retain(|k, req| {
                if inflight.contains(k) || failed.contains_key(k) {
                    return false;
                }
                if reqs.len() < ART_BATCH {
                    reqs.push(req.clone());
                    keys.push(*k);
                    false
                } else {
                    true // stays queued for the next pump
                }
            });
        }
        if reqs.is_empty() {
            return Task::none();
        }
        self.art_inflight.extend(keys);
        Task::perform(fetch_artwork_masked(reqs), Message::ArtworkReady)
    }

    fn view(&self) -> Element<'_, Message> {
        // Entity pages (playlist / artist / track) run their tinted header
        // edge to edge like Spotify; every other tab keeps its inset.
        let island_pad = if matches!(self.tab, Tab::Playlist | Tab::Profile | Tab::Track) {
            Padding::ZERO
        } else {
            pad4(4.0, 8.0, 8.0, 8.0)
        };
        let content_card = container(self.v_scrollable(self.view_content()))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(island_pad)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_MUTED)),
                border: Border {
                    radius: border::Radius::from(8.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..container::Style::default()
            });

        let mut body_row = row![
            self.view_sidebar(),
            container(content_card)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(pad4(4.0, 8.0, 8.0, 0.0))
                .style(|_| container::Style {
                    border: Border {
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        radius: border::Radius::from(0.0),
                    },
                    ..container::Style::default()
                }),
        ]
        .spacing(0)
        .height(Length::Fill);

        if self.show_queue {
            body_row = body_row.push(self.view_queue_panel());
        }

        if self.inspector_track.is_some() {
            body_row = body_row.push(self.view_inspector_panel());
        }

        let main_col = column![self.view_drag_bar(), body_row, self.view_player_bar(),];

        let main_view = container(main_col.height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| panel(BG, 0.0));

        let mut layers: Vec<Element<'_, Message>> = vec![main_view.into()];

        // Live emoji reactions: full-window transparent overlay. Bubbles fly
        // from the waveform (bottom bar) upward towards mid-screen. This is
        // placed AFTER main_view so it renders above the bar, but BEFORE the
        // toast/modals so those still win on top.
        if !self.floating.is_empty() || !self.particles.is_empty() {
            layers.push(self.view_reaction_overlay());
        } else {
            layers.push(iced::widget::Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into());
        }

        if self.wave_context_frac.is_some() {
            layers.push(self.view_wave_context());
        } else {
            layers.push(iced::widget::Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into());
        }

        // The toast keeps its slot even when there is none, so the layers
        // above it never change index: iced diffs stack layers by position,
        // and a shift would rebuild an open popup (field focus, slider drag
        // and list scroll lost when a toast appears or expires).
        layers.push(match &self.toast {
            Some(toast) => self.view_spotify_toast(toast),
            None => iced::widget::Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into(),
        });

        if let Some(pop) = self
            .add_popover
            .as_ref()
            .filter(|_| self.add_popover_anchor == MenuAnchor::PlayerBar)
        {
            // Any click elsewhere closes it without saving, like Spotify. It
            // masks what is below (Idle cursor, no hover) and eats the wheel,
            // so nothing underneath scrolls or skips tracks meanwhile.
            let backdrop = iced::widget::mouse_area(
                container(horizontal_space())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::CloseAddPopover)
            .on_right_press(Message::CloseAddPopover)
            .on_middle_press(Message::CloseAddPopover)
            .on_scroll(|_| Message::Noop)
            .interaction(mouse::Interaction::Idle);
            // Spotify's "top-start" placement, right over the check: the
            // card's left edge on the glyph's, its bottom 8px above the button.
            let layer = column![
                vertical_space(),
                row![
                    horizontal_space().width(Length::Fixed(PB_SAVE_ICON_X)),
                    self.view_add_popover(pop),
                ],
                vertical_space().height(Length::Fixed(PB_POP_BOTTOM)),
            ]
            .width(Length::Fill)
            .height(Length::Fill);
            layers.push(backdrop.into());
            layers.push(layer.into());
        }

        if let Some(field) = &self.speed_popup {
            // any click elsewhere closes it; the bar below is masked and the
            // wheel eaten, except over the pill (see `pill_zone` below)
            let backdrop = iced::widget::mouse_area(
                container(horizontal_space())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::CloseSpeedPopup)
            .on_right_press(Message::CloseSpeedPopup)
            .on_middle_press(Message::CloseSpeedPopup)
            .on_scroll(|_| Message::Noop)
            .interaction(mouse::Interaction::Idle);
            // Right over the speed pill, the first control of the right-hand
            // controls (PB_RIGHT_W wide, flush with the bar's right padding).
            let x = (self.window_size.width - PB_PAD_X - PB_RIGHT_W).max(0.0);
            let layer = column![
                vertical_space(),
                row![
                    horizontal_space().width(Length::Fixed(x)),
                    self.view_speed_popup(field),
                ],
                vertical_space().height(Length::Fixed(PB_POP_BOTTOM)),
            ]
            .width(Length::Fill)
            .height(Length::Fill);
            // Over the pill itself (vertically centred in the bar) the wheel
            // still steps the speed; a click there closes, like elsewhere.
            let pill_zone = column![
                vertical_space(),
                row![
                    horizontal_space().width(Length::Fixed(x)),
                    iced::widget::mouse_area(iced::widget::Space::new(
                        Length::Fixed(PB_SPEED_W),
                        Length::Fixed(PB_BTN),
                    ))
                    .on_scroll(|d| Message::SpeedWheel(wheel_notches(d)))
                    .on_press(Message::CloseSpeedPopup)
                    .on_right_press(Message::CloseSpeedPopup)
                    .interaction(mouse::Interaction::Pointer),
                ],
                vertical_space().height(Length::Fixed(PB_H / 2.0 - PB_BTN / 2.0)),
            ]
            .width(Length::Fill)
            .height(Length::Fill);
            layers.push(backdrop.into());
            layers.push(pill_zone.into());
            layers.push(layer.into());
        }

        if self.show_user_menu {
            let backdrop = button(
                container(horizontal_space())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::CloseUserMenu)
            .padding(0)
            .style(|_, _| button::Style {
                background: None,
                ..button::Style::default()
            });

            let menu_card = self.view_user_menu_dropdown();

            // Right edge flush with the profile chip: PAD_R 10 + 3 window
            // buttons (3 x 32) + 4 gaps (4 x 4) + 8 spacer = 130px from the edge.
            let dropdown_layer = column![
                vertical_space().height(Length::Fixed(48.0)),
                row![
                    horizontal_space(),
                    menu_card,
                    horizontal_space().width(Length::Fixed(130.0)),
                ]
                .align_y(iced::Alignment::Start),
            ]
            .width(Length::Fill)
            .height(Length::Fill);

            layers.push(backdrop.into());
            layers.push(dropdown_layer.into());
        }

        if let Some(story_idx) = self.active_story_index {
            layers.push(self.view_story_fullscreen(story_idx));
        }

        if let Some(viewer) = self.image_viewer.as_ref() {
            layers.push(self.view_image_viewer(viewer));
        }

        if let Some((id, title)) = &self.delete_playlist_confirm {
            layers.push(self.view_delete_playlist_confirm(*id, title));
        }
        if let Some(release) = &self.update_release {
            layers.push(self.view_update_available(release));
        }

        stack(layers).into()
    }

    fn view_update_available(&self, release: &crate::updater::Release) -> Element<'_, Message> {
        let notes = if release.notes.trim().is_empty() {
            "See the GitHub release page for details.".to_string()
        } else {
            release
                .notes
                .lines()
                .map(|line| {
                    let line = line.trim();
                    if let Some(heading) = line.strip_prefix("### ") {
                        heading.to_uppercase()
                    } else if let Some(heading) = line.strip_prefix("## ") {
                        heading.to_string()
                    } else if let Some(heading) = line.strip_prefix("# ") {
                        heading.to_string()
                    } else if let Some(item) = line.strip_prefix("- ") {
                        format!("• {item}")
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let remind = button(
            row![
                text(if self.remind_after_new_version { "☑" } else { "☐" })
                    .size(18)
                    .style(|_| bright()),
                text("Remind me after new version").size(13).style(|_| dim()),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::RemindAfterNewVersion(!self.remind_after_new_version))
        .padding(0)
        .style(|_, _| button::Style {
            background: None,
            ..button::Style::default()
        });
        let accept_label = if self.update_installing {
            "Downloading…"
        } else {
            "Accept"
        };
        let card = container(
            column![
                text(format!("Update available — Wavify {}", release.version))
                    .size(20)
                    .font(UI_BOLD)
                    .style(|_| bright()),
                text("What's new")
                    .size(13)
                    .font(UI_BOLD)
                    .style(|_| dim()),
                scrollable(
                    container(text(notes).size(13).style(|_| dim()))
                        .width(Length::Fill)
                        .padding(Padding::from([4, 2])),
                )
                .height(Length::Fixed(340.0)),
                remind,
                row![
                    horizontal_space(),
                    button(text("Decline").size(14))
                        .on_press_maybe((!self.update_installing).then_some(Message::DeclineUpdate))
                        .padding(Padding::from([9, 18]))
                        .style(|_, status| button::Style {
                            background: Some(Background::Color(if status == button::Status::Hovered {
                                BG_HOVER
                            } else {
                                BG_CARD
                            })),
                            text_color: TEXT,
                            border: Border { radius: border::Radius::from(18.0), width: 1.0, color: TEXT_MUTED },
                            ..button::Style::default()
                        }),
                    button(text(accept_label).size(14).font(UI_BOLD))
                        .on_press_maybe((!self.update_installing).then_some(Message::AcceptUpdate))
                        .padding(Padding::from([9, 20]))
                        .style(|_, status| button::Style {
                            background: Some(Background::Color(if status == button::Status::Hovered {
                                ORANGE_DIM
                            } else {
                                ORANGE
                            })),
                            text_color: Color::BLACK,
                            border: round(18.0),
                            ..button::Style::default()
                        }),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(14)
            .width(Length::Fill),
        )
        .width(Length::Fixed(620.0))
        .padding(24)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_CARD)),
            border: round(14.0),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.7),
                offset: Vector::new(0.0, 10.0),
                blur_radius: 30.0,
            },
            ..container::Style::default()
        });
        let backdrop = iced::widget::mouse_area(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.68))),
                    ..container::Style::default()
                }),
        )
        .on_press(Message::DeclineUpdate)
        .on_scroll(|_| Message::Noop);
        let layers: Vec<Element<'_, Message>> = vec![
            backdrop.into(),
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
        ];
        stack(layers).into()
    }

    /// Confirmation dialog before deleting an owned playlist.
    fn view_delete_playlist_confirm(&self, id: i64, title: &str) -> Element<'_, Message> {
        let card = container(
            column![
                text("Delete from Library?").size(18).font(UI_BOLD).style(|_| bright()),
                text(format!(
                    "This will delete '{}' from your SoundCloud account and library. This action cannot be undone.",
                    title
                ))
                .size(14)
                .style(|_| dim()),
                row![
                    horizontal_space(),
                    button(
                        container(text("Cancel").size(14).wrapping(text::Wrapping::None))
                            .center_y(Length::Fixed(32.0))
                            .padding(Padding::from([0, 16])),
                    )
                    .on_press(Message::CancelDeletePlaylist)
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                            _ => None,
                        },
                        text_color: TEXT,
                        border: Border { radius: border::Radius::from(16.0), width: 1.0, color: TEXT_MUTED },
                        ..button::Style::default()
                    }),
                    button(
                        container(text("Delete").size(14).wrapping(text::Wrapping::None))
                            .center_y(Length::Fixed(32.0))
                            .padding(Padding::from([0, 16])),
                    )
                    .on_press(Message::DeletePlaylist(id))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered | button::Status::Pressed => {
                                Some(Background::Color(Color::from_rgb(0.75, 0.15, 0.15)))
                            }
                            _ => Some(Background::Color(DANGER_RED)),
                        },
                        text_color: Color::WHITE,
                        border: round(16.0),
                        ..button::Style::default()
                    }),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(16),
        )
        .width(Length::Fixed(400.0))
        .padding(24)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_CARD)),
            border: round(12.0),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

        // A click on the dim backdrop cancels; one on the card itself (its
        // text, its padding) must not, so the card swallows its own clicks.
        iced::widget::mouse_area(
            container(
                iced::widget::mouse_area(card)
                    .on_press(Message::Noop)
                    .interaction(mouse::Interaction::Idle),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
                ..container::Style::default()
            }),
        )
        .on_press(Message::CancelDeletePlaylist)
        .into()
    }

    /// Full-window transparent overlay for live reactions: others' float
    /// up from the waveform's playhead, yours burst from its button.
    fn view_reaction_overlay(&self) -> Element<'_, Message> {
        canvas(ReactionOverlay {
            floating: self.floating.clone(),
            particles: self.particles.clone(),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Contextual editor for a precise waveform moment. It replaces the
    /// always-visible reaction strip with one compact action surface.
    fn view_wave_context(&self) -> Element<'_, Message> {
        let frac = self.wave_context_frac.unwrap_or(0.0);
        let at_ms = (self.dur_ms as f32 * frac).round() as u64;
        let moment = format!("at {}", fmt_time(at_ms));
        let emoji_buttons = QUICK_REACTIONS
            .iter()
            .fold(row![].spacing(8), |r, (codepoint, _)| {
                let glyph: Element<'_, Message> = match reaction_image(codepoint) {
                    Some(handle) => image(handle)
                        .width(Length::Fixed(22.0))
                        .height(Length::Fixed(22.0))
                        .into(),
                    None => text(
                        WaveReaction {
                            second: 0,
                            codepoint: (*codepoint).to_string(),
                        }
                        .emoji(),
                    )
                    .size(20)
                    .font(iced::Font::with_name("Segoe UI Emoji"))
                    .into(),
                };
                r.push(
                    button(
                        container(glyph)
                            .center_x(Length::Fixed(40.0))
                            .center_y(Length::Fixed(36.0)),
                    )
                    .on_press(Message::WaveContextReact((*codepoint).to_string()))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(if status == button::Status::Hovered {
                            BG_HOVER
                        } else {
                            BG_TINT
                        })),
                        border: round(8.0),
                        ..button::Style::default()
                    }),
                )
            });
        let card = container(
            column![
                row![
                    text("Add to this moment")
                        .size(16)
                        .font(UI_BOLD)
                        .style(|_| bright()),
                    horizontal_space(),
                    text(moment).size(12).style(|_| muted()),
                    button(text(icons::XMARK).font(FA_SOLID).size(13))
                        .on_press(Message::WaveContextClose)
                        .padding(8)
                        .style(|_, _| button::Style {
                            background: None,
                            ..button::Style::default()
                        }),
                ]
                .align_y(iced::Alignment::Center),
                text("Comment or react without leaving the player")
                    .size(12)
                    .style(|_| muted()),
                text_input("Write a comment…", &self.wave_context_comment)
                    .on_input(Message::WaveContextCommentInput)
                    .on_submit(Message::WaveContextPostComment)
                    .size(14)
                    .padding(Padding::from([9, 12])),
                row![
                    button(text("Post comment").size(13).font(UI_BOLD))
                        .on_press(Message::WaveContextPostComment)
                        .padding(Padding::from([8, 14]))
                        .style(|_, status| button::Style {
                            background: Some(Background::Color(
                                if status == button::Status::Hovered {
                                    ORANGE_DIM
                                } else {
                                    ORANGE
                                }
                            )),
                            text_color: Color::BLACK,
                            border: round(6.0),
                            ..button::Style::default()
                        }),
                    horizontal_space(),
                    text("React").size(12).style(|_| muted()),
                    emoji_buttons,
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12)
            .padding(18)
            .width(Length::Fixed(430.0)),
        )
        .style(|_| container::Style {
            background: Some(Background::Color(BG_CARD)),
            border: round(12.0),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.65),
                offset: Vector::new(0.0, 10.0),
                blur_radius: 28.0,
            },
            ..container::Style::default()
        });
        let backdrop = iced::widget::mouse_area(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.42))),
                    ..container::Style::default()
                }),
        )
        .on_press(Message::WaveContextClose)
        .on_right_press(Message::WaveContextClose)
        .on_scroll(|_| Message::Noop);
        // the card keeps clicks on its title, padding and the gaps between
        // its buttons; only the backdrop around it closes the panel
        let card = iced::widget::mouse_area(card)
            .on_press(Message::Noop)
            .on_right_press(Message::Noop)
            .interaction(mouse::Interaction::Idle);
        stack![
            backdrop,
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        ]
        .into()
    }
    /// bold title, "Find a playlist" search, "New playlist", then Liked
    /// Tracks and the user's playlists as tick rows, and Cancel / Done.
    fn view_add_popover<'a>(&'a self, pop: &'a AddPopover) -> Element<'a, Message> {
        const W: f32 = 292.0;
        const HEADER_H: f32 = 40.0;
        const SEARCH_H: f32 = 32.0; // 18 line + 2 x 7 padding
        const SEARCH_GAP: f32 = 4.0;
        const NEW_H: f32 = 48.0;
        const RULE_H: f32 = 1.0;
        const ROW_H: f32 = 56.0;
        const ROW_PAD: f32 = 8.0;
        const LIST_PAD: f32 = 12.0;
        const COVER: f32 = 32.0;
        const MARK: f32 = 16.0;
        const GAP: f32 = 12.0;
        const FOOTER_H: f32 = 56.0;
        const BTN_H: f32 = 32.0;
        // row title: list width minus row padding, cover, tick and two gaps
        const TITLE_W: f32 = W - 2.0 * LIST_PAD - 2.0 * ROW_PAD - COVER - MARK - 2.0 * GAP;
        // title bar (46) + gap (8): the card never runs under it
        const TOP_RESERVE: f32 = 54.0;

        let row_style = |_: &iced::Theme, status: button::Status| button::Style {
            background: match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(Background::Color(BG_TINT))
                }
                _ => None,
            },
            border: round(4.0),
            ..button::Style::default()
        };

        let header = container(optical_center(
            text("Add to playlist")
                .size(12)
                .font(UI_BOLD)
                .wrapping(text::Wrapping::None)
                .style(|_| dim()),
            12.0,
        ))
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_H))
        .padding(pad4(0.0, 20.0, 0.0, 20.0))
        .align_y(iced::alignment::Vertical::Center);

        let search = container(
            text_input("Find a playlist", &pop.query)
                .on_input(Message::AddPopoverQuery)
                .icon(text_input::Icon {
                    font: FA_SOLID,
                    code_point: '\u{f002}',
                    size: Some(iced::Pixels(12.0)),
                    spacing: 8.0,
                    side: text_input::Side::Left,
                })
                .size(14)
                .line_height(text::LineHeight::Absolute(iced::Pixels(18.0)))
                .padding(Padding {
                    top: 7.0,
                    right: 8.0,
                    bottom: 7.0,
                    left: 10.0,
                })
                .style(|_, _| text_input::Style {
                    background: Background::Color(BG_TINT),
                    border: round(4.0),
                    icon: TEXT_DIM,
                    placeholder: TEXT_DIM,
                    value: TEXT,
                    selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
                }),
        )
        .height(Length::Fixed(SEARCH_H))
        .padding(pad4(0.0, LIST_PAD, 0.0, LIST_PAD));

        let new_row = match &pop.new_name {
            // naming it: the field, and Create
            Some(name) => container(
                row![
                    text_input("Playlist name", name)
                        .id(new_playlist_input_id())
                        .on_input(Message::AddPopoverNewName)
                        .on_submit(Message::AddPopoverCreate)
                        .size(14)
                        .line_height(text::LineHeight::Absolute(iced::Pixels(18.0)))
                        .padding(Padding {
                            top: 7.0,
                            right: 8.0,
                            bottom: 7.0,
                            left: 10.0
                        })
                        .style(|_, _| text_input::Style {
                            background: Background::Color(BG_TINT),
                            border: round(4.0),
                            icon: TEXT_DIM,
                            placeholder: TEXT_DIM,
                            value: TEXT,
                            selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
                        }),
                    button(
                        container(text("Create").size(14).wrapping(text::Wrapping::None))
                            .center_y(Length::Fixed(BTN_H))
                            .padding(Padding::from([0, 14])),
                    )
                    .on_press_maybe((!name.trim().is_empty()).then_some(Message::AddPopoverCreate))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(match status {
                            button::Status::Hovered => Color::WHITE,
                            button::Status::Disabled => Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                            _ => Color::from_rgb(0.9, 0.9, 0.9),
                        })),
                        text_color: Color::BLACK,
                        border: round(BTN_H / 2.0),
                        ..button::Style::default()
                    }),
                ]
                .spacing(8)
                .height(Length::Fixed(NEW_H))
                .align_y(iced::Alignment::Center),
            )
            .padding(pad4(0.0, LIST_PAD, 0.0, LIST_PAD)),
            None => container(
                button(
                    row![
                        canvas(SaveGlyph {
                            kind: GlyphKind::Plus,
                            color: TEXT,
                        })
                        .width(Length::Fixed(MARK))
                        .height(Length::Fixed(MARK)),
                        optical_center(
                            text("New playlist")
                                .size(16)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright()),
                            16.0,
                        ),
                    ]
                    .spacing(GAP)
                    .height(Length::Fill)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::AddPopoverNewPlaylist)
                .padding(pad4(0.0, ROW_PAD, 0.0, ROW_PAD))
                .width(Length::Fill)
                .height(Length::Fixed(NEW_H))
                .style(row_style),
            )
            .padding(pad4(0.0, LIST_PAD, 0.0, LIST_PAD)),
        };

        let rule = container(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fixed(RULE_H))
                .style(|_| panel(BG_TINT, 0.0)),
        )
        .padding(pad4(0.0, LIST_PAD, 0.0, LIST_PAD));

        // One tick row: cover, title, and Spotify's tick (filled check when
        // picked, a thin ring when not). Fixed height, one clipped line.
        let tick_row = |cover: Element<'a, Message>, title: &str, picked: bool, msg: Message| {
            let mark: Element<'a, Message> = if picked {
                canvas(SaveGlyph {
                    kind: GlyphKind::Saved,
                    color: ORANGE,
                })
                .width(Length::Fixed(MARK))
                .height(Length::Fixed(MARK))
                .into()
            } else {
                container(horizontal_space())
                    .width(Length::Fixed(MARK))
                    .height(Length::Fixed(MARK))
                    .style(|_| container::Style {
                        border: Border {
                            radius: border::Radius::from(MARK / 2.0),
                            width: 1.0,
                            color: TEXT_MUTED,
                        },
                        ..container::Style::default()
                    })
                    .into()
            };
            button(
                row![
                    cover,
                    container(optical_center(
                        text(trunc_px(title, TITLE_W, 16.0))
                            .size(16)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                        16.0,
                    ))
                    .width(Length::Fill)
                    .clip(true),
                    mark,
                ]
                .spacing(GAP)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center),
            )
            .on_press(msg)
            .padding(pad4(0.0, ROW_PAD, 0.0, ROW_PAD))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_H))
            .style(row_style)
        };

        let q = pop.query.trim().to_lowercase();
        let matches = |name: &str| q.is_empty() || name.to_lowercase().contains(&q);
        let mut rows = column![];
        let mut n_rows = 0usize;
        if matches("Liked Tracks") {
            rows = rows.push(tick_row(
                liked_tracks_tile(COVER),
                "Liked Tracks",
                pop.liked,
                Message::AddPopoverToggleLiked,
            ));
            n_rows += 1;
        }
        for p in self.own_playlists().filter(|p| matches(&p.title)) {
            rows = rows.push(tick_row(
                self.artwork_tile(p.artwork_or_avatar(), p.id, &p.title, COVER, false),
                &p.title,
                pop.picked.contains(&p.id),
                Message::AddPopoverToggle(p.id),
            ));
            n_rows += 1;
        }
        if n_rows == 0 {
            rows = rows.push(
                container(
                    text("No playlists found")
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                )
                .width(Length::Fill)
                .height(Length::Fixed(ROW_H))
                .center_x(Length::Fill)
                .center_y(Length::Fixed(ROW_H)),
            );
            n_rows = 1;
        }
        // Five and a half rows before it scrolls (the cut row hints at more,
        // as in Spotify); short windows get fewer so the card stays on screen.
        let fixed = HEADER_H + SEARCH_H + SEARCH_GAP + NEW_H + RULE_H + FOOTER_H;
        let room = (self.window_size.height - PB_POP_BOTTOM - TOP_RESERVE - fixed).max(ROW_H);
        let list_h = (n_rows as f32 * ROW_H).min(5.5 * ROW_H).min(room);
        let list = self
            .v_scrollable(container(rows).padding(pad4(0.0, LIST_PAD, 0.0, LIST_PAD)))
            .width(Length::Fill)
            .height(Length::Fixed(list_h));

        // Cancel always; Done only once something changed (Spotify).
        let dirty = pop.liked != pop.liked_was || pop.picked != pop.picked_was;
        let cancel = button(
            container(optical_center(
                text("Cancel")
                    .size(14)
                    .font(UI_BOLD)
                    .wrapping(text::Wrapping::None),
                14.0,
            ))
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::CloseAddPopover)
        .padding(pad4(0.0, 4.0, 0.0, 4.0))
        .height(Length::Fixed(BTN_H))
        .style(|_, status| button::Style {
            background: None,
            text_color: match status {
                button::Status::Hovered | button::Status::Pressed => TEXT,
                _ => TEXT_DIM,
            },
            ..button::Style::default()
        });
        let mut foot = row![horizontal_space(), cancel]
            .spacing(16)
            .align_y(iced::Alignment::Center);
        if dirty {
            foot = foot.push(
                button(
                    container(optical_center(
                        text("Done")
                            .size(14)
                            .font(UI_BOLD)
                            .wrapping(text::Wrapping::None),
                        14.0,
                    ))
                    .height(Length::Fill)
                    .align_y(iced::alignment::Vertical::Center),
                )
                .on_press(Message::AddPopoverDone)
                .padding(pad4(0.0, 16.0, 0.0, 16.0))
                .height(Length::Fixed(BTN_H))
                .style(|_, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Color::from_rgb(1.0, 0.45, 0.15)
                        }
                        _ => ORANGE,
                    })),
                    text_color: Color::BLACK,
                    border: round(BTN_H / 2.0),
                    ..button::Style::default()
                }),
            );
        }
        let footer = container(foot)
            .width(Length::Fill)
            .height(Length::Fixed(FOOTER_H))
            .padding(pad4(12.0, 24.0, 12.0, 24.0));

        let card = container(column![
            header,
            search,
            vertical_space().height(Length::Fixed(SEARCH_GAP)),
            new_row,
            rule,
            list,
            footer,
        ])
        .width(Length::Fixed(W))
        .style(|_| container::Style {
            background: Some(Background::Color(BG_CARD)),
            border: round(8.0),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 16.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

        // clicks on the card's own padding must not reach the close backdrop
        iced::widget::mouse_area(card)
            .on_press(Message::Noop)
            .on_right_press(Message::Noop)
            .on_middle_press(Message::Noop)
            .interaction(mouse::Interaction::Idle)
            .into()
    }

    /// Right-click popup over the speed pill: a 0.1x-2.0x slider (the wheel
    /// works on it too) and an exact value field; Enter applies and closes.
    /// Same card as the playlist picker.
    fn view_speed_popup<'a>(&'a self, field: &'a str) -> Element<'a, Message> {
        const W: f32 = 264.0;
        const PAD_X: f32 = 20.0;
        const HEADER_H: f32 = 40.0;
        const FIELD_W: f32 = 76.0;
        const BTN_H: f32 = 32.0;

        let header = container(
            row![
                optical_center(
                    text("Playback speed")
                        .size(12)
                        .font(UI_BOLD)
                        .wrapping(text::Wrapping::None)
                        .style(|_| dim()),
                    12.0,
                ),
                horizontal_space(),
                optical_center(
                    text(format!("{:.2}x", self.playback_speed))
                        .size(14)
                        .font(UI_BOLD)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                    14.0,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_H))
        .padding(pad4(0.0, PAD_X, 0.0, PAD_X))
        .align_y(iced::alignment::Vertical::Center);

        // Rail and knob match the volume control: 4px rail, orange fill, white knob.
        let speed_slider = iced::widget::mouse_area(
            slider(
                SPEED_MIN..=SPEED_MAX,
                self.playback_speed,
                Message::SpeedSlider,
            )
            .step(0.01_f32)
            .width(Length::Fill)
            .style(|_, status| slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        Background::Color(ORANGE),
                        Background::Color(Color::from_rgb(0.30, 0.30, 0.30)),
                    ),
                    width: 4.0,
                    border: round(2.0),
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle {
                        radius: match status {
                            slider::Status::Active => 6.0,
                            _ => 7.0,
                        },
                    },
                    background: Background::Color(Color::WHITE),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }),
        )
        .on_scroll(|d| Message::SpeedWheel(wheel_notches(d)));

        let scale = row![
            text("0.1x")
                .size(11)
                .wrapping(text::Wrapping::None)
                .style(|_| muted()),
            horizontal_space(),
            text("2.0x")
                .size(11)
                .wrapping(text::Wrapping::None)
                .style(|_| muted()),
        ];

        let value_field = text_input("1.00", field)
            .id(speed_input_id())
            .on_input(Message::SpeedInput)
            .on_submit(Message::SpeedInputSubmit)
            .size(14)
            .line_height(text::LineHeight::Absolute(iced::Pixels(18.0)))
            .padding(Padding {
                top: 7.0,
                right: 8.0,
                bottom: 7.0,
                left: 10.0,
            })
            .width(Length::Fixed(FIELD_W))
            .style(|_, _| text_input::Style {
                background: Background::Color(BG_TINT),
                border: round(4.0),
                icon: TEXT_DIM,
                placeholder: TEXT_DIM,
                value: TEXT,
                selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
            });

        let reset = button(
            container(optical_center(
                text("Reset")
                    .size(14)
                    .font(UI_BOLD)
                    .wrapping(text::Wrapping::None),
                14.0,
            ))
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::SpeedReset)
        .padding(pad4(0.0, 4.0, 0.0, 4.0))
        .height(Length::Fixed(BTN_H))
        .style(|_, status| button::Style {
            background: None,
            text_color: match status {
                button::Status::Hovered | button::Status::Pressed => TEXT,
                _ => TEXT_DIM,
            },
            ..button::Style::default()
        });

        let controls = row![
            value_field,
            optical_center(
                text("x")
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| dim()),
                14.0,
            ),
            horizontal_space(),
            reset,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let card = container(column![
            header,
            container(column![speed_slider, scale].spacing(6))
                .padding(pad4(0.0, PAD_X, 0.0, PAD_X)),
            container(controls).padding(pad4(14.0, PAD_X, 16.0, PAD_X)),
        ])
        .width(Length::Fixed(W))
        .style(|_| container::Style {
            background: Some(Background::Color(BG_CARD)),
            border: round(8.0),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 16.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

        // clicks on the card's own padding must not reach the close backdrop
        iced::widget::mouse_area(card)
            .on_press(Message::Noop)
            .on_right_press(Message::Noop)
            .on_middle_press(Message::Noop)
            .interaction(mouse::Interaction::Idle)
            .into()
    }

    /// Floating Spotify-styled notification (snackbar) matching 1.html / 2.html.
    fn view_spotify_toast<'a>(&'a self, toast: &'a Toast) -> Element<'a, Message> {
        let elapsed = toast.created_at.elapsed().as_secs_f32();
        // 0.0..0.22s smooth cubic ease-out fade-in, 0.22..2.80s solid hold, 2.80..3.20s smooth fade-out
        let alpha = if elapsed < 0.22 {
            let t = (elapsed / 0.22).clamp(0.0, 1.0);
            1.0 - (1.0 - t).powi(2)
        } else if elapsed > 2.80 {
            let t = ((3.20 - elapsed) / 0.40).clamp(0.0, 1.0);
            t * t
        } else {
            1.0
        };

        let is_like = toast.message.contains("Liked")
            || toast.message.contains("liked")
            || toast.message.contains("like");
        let (icon_bg, icon_str, icon_color) = match toast.kind {
            ToastKind::Success => {
                if is_like {
                    (
                        Color::from_rgb(0.118, 0.843, 0.376),
                        icons::HEART,
                        Color::WHITE,
                    )
                } else {
                    (
                        Color::from_rgb(0.118, 0.843, 0.376),
                        icons::CHECK,
                        Color::WHITE,
                    ) // Spotify Green #1ED760
                }
            }
            ToastKind::Error => (
                Color::from_rgb(0.914, 0.078, 0.161),
                icons::XMARK,
                Color::WHITE,
            ), // Spotify Red #E91429
            ToastKind::Info => (
                Color::from_rgb(0.24, 0.24, 0.24),
                icons::INFO,
                Color::from_rgb(0.90, 0.90, 0.90),
            ),
        };

        // Round icon badge (20x20 circle)
        let icon_badge =
            container(
                text(icon_str)
                    .font(FA_SOLID)
                    .size(14)
                    .style(move |_| text::Style {
                        color: Some(Color {
                            r: icon_color.r,
                            g: icon_color.g,
                            b: icon_color.b,
                            a: alpha,
                        }),
                    }),
            )
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .center_x(Length::Fixed(20.0))
            .center_y(Length::Fixed(20.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color {
                    r: icon_bg.r,
                    g: icon_bg.g,
                    b: icon_bg.b,
                    a: alpha,
                })),
                border: Border {
                    radius: border::Radius::from(10.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..container::Style::default()
            });

        // Crisp Spotify white text
        let clean_msg = toast
            .message
            .trim_start_matches("✓ ")
            .trim_start_matches("⚠ ");
        let msg_label = text(clean_msg).size(16).style(move |_| text::Style {
            color: Some(Color::from_rgba(1.0, 1.0, 1.0, alpha)),
        });

        // Close button: subtle FontAwesome xmark
        let close_btn =
            button(
                text(icons::XMARK)
                    .font(FA_SOLID)
                    .size(14)
                    .style(move |_| text::Style {
                        color: Some(Color::from_rgba(0.70, 0.70, 0.70, alpha * 0.8)),
                    }),
            )
            .on_press(Message::DismissToast)
            .padding(Padding::from([2, 5]))
            .style(move |_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(Color::from_rgba(
                        0.35,
                        0.35,
                        0.35,
                        alpha * 0.5,
                    ))),
                    _ => None,
                },
                border: Border {
                    radius: border::Radius::from(4.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..button::Style::default()
            });

        // Spotify's text action ("Change"), bold, before the close button
        let mut toast_row = row![icon_badge, msg_label]
            .spacing(10)
            .align_y(iced::Alignment::Center);
        if let Some((label, action)) = &toast.action {
            toast_row = toast_row.push(
                button(
                    text(label.as_str())
                        .size(14)
                        .font(UI_BOLD)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| text::Style {
                            color: Some(Color::from_rgba(1.0, 1.0, 1.0, alpha)),
                        }),
                )
                .on_press((**action).clone())
                .padding(Padding::from([2, 6]))
                .style(move |_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(Color::from_rgba(
                            1.0,
                            1.0,
                            1.0,
                            alpha * 0.10,
                        ))),
                        _ => None,
                    },
                    border: round(4.0),
                    ..button::Style::default()
                }),
            );
        }
        toast_row = toast_row.push(close_btn);

        // Floating Spotify snackbar card
        let card = container(toast_row)
            .padding(Padding::from([8, 14]))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    0.16,
                    0.16,
                    0.16,
                    alpha * 0.96,
                ))),
                border: Border {
                    radius: border::Radius::from(8.0),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, alpha * 0.12),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, alpha * 0.60),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 18.0,
                },
                ..container::Style::default()
            });

        // Floating overlay pinned horizontally to center and vertically above bottom player bar (92px)
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(pad4(0.0, 0.0, 92.0, 0.0))
            .into()
    }

    /// Spotify-style window header containing navigation buttons, centered Home + search, user profile, and window controls.
    fn view_drag_bar(&self) -> Element<'_, Message> {
        // Bar geometry. Every control shares the TITLEBAR_CTRL (32px) hit box
        // and is vertically centred in the 46px strip.
        const BAR_H: f32 = 46.0;
        const PAD_L: f32 = 12.0;
        const PAD_R: f32 = 10.0;
        // Home + search live on their own layer, centred on the window and kept
        // clear of both side groups by the same reserve on each side. They no
        // longer move when the profile chip changes width (login / logout,
        // username length), and on narrow windows the search field shrinks
        // instead of pushing the window controls off-screen.
        //   left group  = PAD_L + 32 + 6 + 32                       =  82
        //   right group = PAD_R + chip (<= 160) + 8 + 3 x 32 + 4 x 4 = 290
        const SIDE_RESERVE: f32 = 306.0;

        // --- 1. Left Section: History Navigation Buttons ---
        let can_back = !self.nav_history.is_empty();
        let can_forward = !self.nav_future.is_empty();

        let mut back_btn = button(
            container(
                text(icons::CHEVRON_LEFT)
                    .font(FA_SOLID)
                    .size(TITLEBAR_ICON)
                    .style(move |_| text::Style {
                        color: Some(if can_back { TEXT } else { TEXT_MUTED }),
                    }),
            )
            .center_x(Length::Fixed(TITLEBAR_CTRL))
            .center_y(Length::Fixed(TITLEBAR_CTRL)),
        )
        .padding(0)
        .style(move |_, status| button::Style {
            background: if can_back {
                match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => Some(Background::Color(BG_CARD)),
                }
            } else {
                Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03)))
            },
            border: round(TITLEBAR_CTRL / 2.0),
            ..button::Style::default()
        });
        if can_back {
            back_btn = back_btn.on_press(Message::NavBack);
        }

        let mut forward_btn = button(
            container(
                text(icons::CHEVRON_RIGHT)
                    .font(FA_SOLID)
                    .size(TITLEBAR_ICON)
                    .style(move |_| text::Style {
                        color: Some(if can_forward { TEXT } else { TEXT_MUTED }),
                    }),
            )
            .center_x(Length::Fixed(TITLEBAR_CTRL))
            .center_y(Length::Fixed(TITLEBAR_CTRL)),
        )
        .padding(0)
        .style(move |_, status| button::Style {
            background: if can_forward {
                match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => Some(Background::Color(BG_CARD)),
                }
            } else {
                Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03)))
            },
            border: round(TITLEBAR_CTRL / 2.0),
            ..button::Style::default()
        });
        if can_forward {
            forward_btn = forward_btn.on_press(Message::NavForward);
        }

        let left_group = row![back_btn, forward_btn]
            .spacing(6)
            .align_y(iced::Alignment::Center);

        // --- 2. Center Section: Home Button + Search Bar ---
        let is_home = self.tab == Tab::Home;
        let home_btn = button(
            container(
                text(icons::HOUSE)
                    .font(FA_SOLID)
                    .size(TITLEBAR_ICON)
                    .style(move |_| text::Style {
                        color: Some(if is_home { TEXT } else { TEXT_DIM }),
                    }),
            )
            .center_x(Length::Fixed(TITLEBAR_CTRL))
            .center_y(Length::Fixed(TITLEBAR_CTRL)),
        )
        .on_press(Message::Tab(Tab::Home))
        .padding(0)
        .style(move |_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => Some(Background::Color(BG_CARD)),
            },
            // The inherited button text colour must track the icon, otherwise it
            // repaints the glyph and the inactive (grey) state never shows.
            text_color: if is_home { TEXT } else { TEXT_DIM },
            border: round(TITLEBAR_CTRL / 2.0),
            ..button::Style::default()
        });

        let search_input = text_input("What do you want to play?", &self.search_query)
            .icon(iced::widget::text_input::Icon {
                font: FA_SOLID,
                code_point: '\u{f002}',
                size: Some(iced::Pixels(16.0)),
                spacing: 10.0,
                side: iced::widget::text_input::Side::Left,
            })
            .on_input(Message::SearchChanged)
            .on_submit(Message::SearchSubmit)
            .padding(Padding::from([7, 16]))
            .size(16)
            .style(|_, status| iced::widget::text_input::Style {
                background: Background::Color(match status {
                    iced::widget::text_input::Status::Focused => BG_HOVER,
                    iced::widget::text_input::Status::Hovered => BG_HOVER,
                    _ => BG_INPUT,
                }),
                border: Border {
                    width: match status {
                        iced::widget::text_input::Status::Focused => 1.0,
                        _ => 0.0,
                    },
                    color: match status {
                        iced::widget::text_input::Status::Focused => ORANGE,
                        _ => Color::TRANSPARENT,
                    },
                    radius: border::Radius::from(20.0),
                },
                icon: TEXT_MUTED,
                placeholder: TEXT_MUTED,
                value: TEXT,
                selection: ORANGE,
            });

        // A fixed width is clamped to the space actually available, so the
        // field keeps its 440px when there is room and shrinks (never
        // overflows) on a narrow window.
        let search_box = container(search_input)
            .width(Length::Fixed(440.0))
            .align_x(iced::alignment::Horizontal::Center);

        let center_group = row![home_btn, search_box]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        // --- Draggable area (everything in the bar that is not a control) ---
        // On press, not release (a button's click): the OS moves a window
        // only while the button is still down.
        let drag_area = iced::widget::mouse_area(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::WindowDragStart);

        // --- 3. Right Section: Profile & Window Controls ---
        let wm_btn = |glyph: &'static str,
                      font: iced::Font,
                      sz: u16,
                      msg: Message|
         -> Element<'_, Message> {
            button(
                container(text(glyph).font(font).size(sz).style(|_| text::Style {
                    color: Some(TEXT_DIM),
                }))
                .center_x(Length::Fixed(TITLEBAR_CTRL))
                .center_y(Length::Fixed(TITLEBAR_CTRL)),
            )
            .on_press(msg)
            .padding(0)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(4.0),
                ..button::Style::default()
            })
            .into()
        };

        let close_btn = button(
            container(
                text(icons::XMARK)
                    .font(FA_SOLID)
                    .size(TITLEBAR_ICON)
                    .style(|_| text::Style {
                        color: Some(TEXT_DIM),
                    }),
            )
            .center_x(Length::Fixed(TITLEBAR_CTRL))
            .center_y(Length::Fixed(TITLEBAR_CTRL)),
        )
        .on_press(Message::CloseButton)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => {
                    Some(Background::Color(Color::from_rgb(0.65, 0.13, 0.13)))
                }
                _ => None,
            },
            border: round(4.0),
            ..button::Style::default()
        });

        let right_group = row![
            self.view_profile_chip(),
            horizontal_space().width(Length::Fixed(8.0)),
            wm_btn(
                icons::MINUS,
                FA_SOLID,
                TITLEBAR_ICON,
                Message::WindowMinimize
            ),
            wm_btn(
                icons::SQUARE,
                FA_REGULAR,
                TITLEBAR_ICON,
                Message::WindowToggleMaximize
            ),
            close_btn,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        // Base layer: nav buttons pinned left, window controls pinned right,
        // the drag area fills everything in between (also behind the search).
        let base = row![left_group, drag_area, right_group]
            .spacing(8)
            .padding(pad4(0.0, PAD_R, 0.0, PAD_L))
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fixed(BAR_H));

        // Top layer: Home + search centred on the whole bar. Only its controls
        // take the mouse; clicks on its empty area fall through to the drag area.
        let center_layer = container(center_group)
            .padding(pad4(0.0, SIDE_RESERVE, 0.0, SIDE_RESERVE))
            .center_x(Length::Fill)
            .center_y(Length::Fixed(BAR_H));

        let bar = stack![base, center_layer]
            .width(Length::Fill)
            .height(Length::Fixed(BAR_H));

        container(bar)
            .width(Length::Fill)
            .height(Length::Fixed(BAR_H))
            .style(|_| container::Style {
                background: Some(Background::Color(BG)),
                border: Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: border::Radius::from(0.0),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// The "..." menu of a track, playlist, artist or Liked Tracks: a card
    /// dropped down from the button that opened it (see dropdown.rs).
    /// Picking an item closes it, like a native menu.
    fn view_action_menu(&self, menu: &ActionMenu, anchor: MenuAnchor) -> Element<'static, Message> {
        // Spotify's context menu: 280px card, 36px rows, 4px inset.
        const MENU_W: f32 = 280.0;
        const ROW_H: f32 = 36.0;
        const ICON_SLOT: f32 = 24.0;

        type Item = (&'static str, iced::Font, String, Message, Color);
        let item = |glyph: &'static str,
                    font: iced::Font,
                    label: &str,
                    msg: Message,
                    color: Color|
         -> Item { (glyph, font, label.to_string(), msg, color) };
        let mut items: Vec<Item> = Vec::new();

        match menu {
            ActionMenu::Track(t) => {
                let tr = t.clone();
                let tid = t.id;
                let liked = self.liked_ids.contains(&tid);
                let reposted = self.reposted_ids.contains(&tid);
                let on_its_page = self.tab == Tab::Track
                    && self.track_page.as_ref().is_some_and(|p| p.track.id == tid);
                let permalink = t
                    .permalink_url
                    .clone()
                    .unwrap_or_else(|| format!("https://soundcloud.com/tracks/{}", tid));

                // Spotify-style order: library and queue first, then
                // navigation, then SoundCloud-specific actions at the bottom.
                items.push(if liked {
                    item(
                        icons::HEART,
                        FA_SOLID,
                        "Remove from Liked Tracks",
                        Message::LikeTrack(tid),
                        HEART,
                    )
                } else {
                    item(
                        icons::HEART,
                        FA_REGULAR,
                        "Save to Liked Tracks",
                        Message::LikeTrack(tid),
                        TEXT_DIM,
                    )
                });
                items.push(item(
                    icons::FOLDER_PLUS,
                    FA_SOLID,
                    "Add to Playlist…",
                    Message::OpenAddPopover(tr.clone(), anchor),
                    ORANGE_DIM,
                ));
                items.push(item(
                    icons::QUEUE,
                    FA_SOLID,
                    "Add to Queue",
                    Message::AddToQueue(tr.clone()),
                    TEXT_DIM,
                ));
                items.push(item(
                    icons::RADIO,
                    FA_SOLID,
                    "Radio",
                    Message::StartTrackRadio(tid, t.title.clone()),
                    ORANGE,
                ));
                if !on_its_page {
                    items.push(item(
                        icons::COMPACT_DISC,
                        FA_SOLID,
                        "Go to Track Page",
                        Message::OpenTrackPage(Box::new(tr.clone())),
                        ORANGE,
                    ));
                }
                if let Some(uid) = t.user.as_ref().map(|u| u.id) {
                    items.push(item(
                        icons::USER,
                        FA_SOLID,
                        "View Artist Profile",
                        Message::OpenProfile(uid),
                        TEXT_DIM,
                    ));
                }
                items.push(if reposted {
                    item(
                        icons::REPOST,
                        FA_SOLID,
                        "Remove Repost",
                        Message::TrackRepostToggle(tid),
                        ORANGE,
                    )
                } else {
                    item(
                        icons::REPOST,
                        FA_SOLID,
                        "Repost to Profile",
                        Message::TrackRepostToggle(tid),
                        TEXT_DIM,
                    )
                });
                items.push(item(
                    icons::COMMENTS,
                    FA_SOLID,
                    "Comments & Details",
                    Message::OpenTrackInspector(tr.clone()),
                    TEXT_DIM,
                ));
                if self.offline_single.contains(&tid) {
                    items.push(item(
                        icons::XMARK,
                        FA_SOLID,
                        "Cancel download",
                        Message::CancelOfflineTrack(tid),
                        ORANGE,
                    ));
                } else if !self.downloaded_track_ids.contains(&tid) {
                    items.push(item(
                        icons::DOWNLOAD,
                        FA_SOLID,
                        "Download",
                        Message::DownloadTrackOffline(tr.clone()),
                        TEXT_DIM,
                    ));
                }
                items.push(item(
                    icons::FOLDER_PLUS,
                    FA_SOLID,
                    "Export MP3 to Downloads",
                    Message::DownloadTrack(tr.clone()),
                    TEXT_DIM,
                ));
                items.push(item(
                    icons::COPY,
                    FA_SOLID,
                    "Copy Track Link",
                    Message::CopyTrackLink(tr),
                    ORANGE,
                ));
                items.push(item(
                    icons::BRAND_SOUNDCLOUD,
                    FA_BRANDS,
                    "Open on soundcloud.com",
                    Message::OpenExternalLink(permalink),
                    TEXT_DIM,
                ));
            }
            ActionMenu::Collection(c @ Collection::Playlist(id)) => {
                let id = *id;
                let is_owned = self.my_playlists.iter().any(|p| p.id == id)
                    || self.current_playlist.as_ref().is_some_and(|p| {
                        p.id == id
                            && self
                                .state
                                .me
                                .as_ref()
                                .is_some_and(|m| m.username.eq_ignore_ascii_case(&p.author))
                    });
                let pl_title = self
                    .my_playlists
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.title.clone())
                    .or_else(|| {
                        self.current_playlist
                            .as_ref()
                            .filter(|p| p.id == id)
                            .map(|p| p.title.clone())
                    })
                    .unwrap_or_else(|| "Playlist".to_string());

                // a mix or station is SoundCloud's own: nothing to like or
                // delete, and its page lives under /discover, not /playlists
                let page = self.current_playlist.as_ref().filter(|p| p.id == id);
                let system = id < 0 || page.is_some_and(|p| is_system_playlist(&p.id_or_urn));
                let link = page
                    .and_then(|p| p.permalink_url.clone())
                    .or_else(|| {
                        self.my_playlists
                            .iter()
                            .chain(&self.liked_playlists)
                            .find(|p| p.id == id)
                            .and_then(|p| p.permalink_url.clone())
                    })
                    .or_else(|| {
                        (!system).then(|| format!("https://soundcloud.com/playlists/{id}"))
                    });

                // a mix or station is saved by its URN
                if let Some(urn) = page
                    .filter(|_| system && self.state.authenticated)
                    .map(|p| p.id_or_urn.clone())
                {
                    items.push(if self.liked_system_urns.contains(&urn) {
                        item(
                            icons::HEART,
                            FA_SOLID,
                            "Remove from Your Library",
                            Message::SystemPlaylistLikeToggle(urn),
                            HEART,
                        )
                    } else {
                        item(
                            icons::HEART,
                            FA_REGULAR,
                            "Save to Your Library",
                            Message::SystemPlaylistLikeToggle(urn),
                            TEXT_DIM,
                        )
                    });
                }
                if self.state.authenticated && !system {
                    items.push(if self.liked_playlist_ids.contains(&id) {
                        item(
                            icons::HEART,
                            FA_SOLID,
                            "Remove from Your Library",
                            Message::PlaylistLikeToggle(id),
                            HEART,
                        )
                    } else {
                        item(
                            icons::HEART,
                            FA_REGULAR,
                            "Save to Your Library",
                            Message::PlaylistLikeToggle(id),
                            TEXT_DIM,
                        )
                    });
                }
                items.push(item(
                    icons::SHUFFLE,
                    FA_SOLID,
                    "Shuffle Play",
                    Message::PlayCollection(*c, true),
                    ORANGE,
                ));
                items.push(item(
                    icons::QUEUE,
                    FA_SOLID,
                    "Add to Queue",
                    Message::QueueCollection(*c),
                    TEXT_DIM,
                ));
                if let Some(uid) = page.and_then(|p| p.author_id) {
                    items.push(item(
                        icons::USER,
                        FA_SOLID,
                        "Go to Author",
                        Message::OpenProfile(uid),
                        TEXT_DIM,
                    ));
                }
                if let Some(link) = link {
                    items.push(item(
                        icons::COPY,
                        FA_SOLID,
                        "Copy Link",
                        Message::CopyPlaylistLink(id, Some(link.clone())),
                        ORANGE,
                    ));
                    items.push(item(
                        icons::BRAND_SOUNDCLOUD,
                        FA_BRANDS,
                        "Open on soundcloud.com",
                        Message::OpenExternalLink(link),
                        TEXT_DIM,
                    ));
                }
                if is_owned && !system {
                    items.push(item(
                        icons::TRASH,
                        FA_SOLID,
                        "Delete Playlist",
                        Message::RequestDeletePlaylist(id, pl_title),
                        DANGER_RED,
                    ));
                }
            }
            ActionMenu::Collection(c @ Collection::Profile(id)) => {
                let id = *id;
                let profile = self.profile.as_ref().filter(|p| p.id == id);
                let name = profile.map(|p| p.username.clone()).unwrap_or_default();
                match self.collection_dl_state(*c) {
                    DlState::Idle => items.push(item(
                        icons::DOWNLOAD,
                        FA_SOLID,
                        "Download",
                        Message::DownloadCollection(*c),
                        TEXT_DIM,
                    )),
                    DlState::Progress(done, total) => items.push(item(
                        icons::XMARK,
                        FA_SOLID,
                        &format!("Cancel download ({done}/{total})"),
                        Message::CancelOfflineDownload,
                        ORANGE,
                    )),
                    DlState::Busy | DlState::Done => {}
                }
                items.push(item(
                    icons::SHUFFLE,
                    FA_SOLID,
                    "Shuffle Play",
                    Message::PlayCollection(*c, true),
                    ORANGE,
                ));
                items.push(item(
                    icons::QUEUE,
                    FA_SOLID,
                    "Add to Queue",
                    Message::QueueCollection(*c),
                    TEXT_DIM,
                ));
                items.push(item(
                    icons::RADIO,
                    FA_SOLID,
                    "Radio",
                    Message::StartArtistRadio(id, name),
                    ORANGE,
                ));
                if let Some(p) = profile {
                    let url = p.permalink_url.clone().unwrap_or_else(|| {
                        format!(
                            "https://soundcloud.com/{}",
                            p.username.trim_start_matches('@')
                        )
                    });
                    items.push(item(
                        icons::COPY,
                        FA_SOLID,
                        "Copy Link",
                        Message::CopyProfileLink(p.id, p.username.clone(), p.permalink_url.clone()),
                        ORANGE,
                    ));
                    items.push(item(
                        icons::BRAND_SOUNDCLOUD,
                        FA_BRANDS,
                        "Open on soundcloud.com",
                        Message::OpenExternalLink(url),
                        TEXT_DIM,
                    ));
                }
            }
            ActionMenu::Collection(c @ Collection::Liked) => {
                items.push(item(
                    icons::SHUFFLE,
                    FA_SOLID,
                    "Shuffle Play",
                    Message::PlayCollection(*c, true),
                    ORANGE,
                ));
                items.push(item(
                    icons::QUEUE,
                    FA_SOLID,
                    "Add to Queue",
                    Message::QueueCollection(*c),
                    TEXT_DIM,
                ));
            }
        }

        let mut rows = column![].spacing(0);
        for (glyph, font, label, msg, icon_color) in items {
            rows = rows.push(
                button(
                    row![
                        container(
                            text(glyph)
                                .font(font)
                                .size(15)
                                .style(move |_| t_color(icon_color))
                        )
                        .center_x(Length::Fixed(ICON_SLOT)),
                        container(
                            text(label)
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright()),
                        )
                        .width(Length::Fill)
                        .clip(true),
                    ]
                    .spacing(10)
                    .height(Length::Fill)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::MenuPick(Box::new(msg)))
                .padding(pad4(0.0, 12.0, 0.0, 8.0))
                .width(Length::Fill)
                .height(Length::Fixed(ROW_H))
                .clip(true)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(BG_TINT)),
                        _ => None,
                    },
                    border: round(2.0),
                    ..button::Style::default()
                }),
            );
        }

        container(rows)
            .width(Length::Fixed(MENU_W))
            .padding(4)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_CARD)),
                border: round(6.0),
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
                ..container::Style::default()
            })
            .into()
    }

    /// What drops down from the menu anchor `anchor` right now: the "..."
    /// menu opened there, or the playlist picker opened from it.
    fn anchored_menu(&self, anchor: MenuAnchor) -> Option<Element<'_, Message>> {
        if let Some(pop) = self
            .add_popover
            .as_ref()
            .filter(|_| self.add_popover_anchor == anchor)
        {
            return Some(self.view_add_popover(pop));
        }
        match (&self.action_menu, self.menu_anchor) {
            (Some(menu), Some(a)) if a == anchor => Some(self.view_action_menu(menu, anchor)),
            _ => None,
        }
    }

    /// `anchor_widget` with whatever menu belongs to `anchor` dropped down
    /// from it; a click outside closes that menu.
    fn with_menu<'a>(
        &'a self,
        anchor_widget: impl Into<Element<'a, Message>>,
        anchor: MenuAnchor,
    ) -> Element<'a, Message> {
        let menu = self.anchored_menu(anchor);
        let dismiss = if self.add_popover.is_some() && self.add_popover_anchor == anchor {
            Message::CloseAddPopover
        } else {
            Message::CloseActionMenu
        };
        crate::dropdown::dropdown(anchor_widget, menu)
            .on_dismiss(dismiss)
            .into()
    }

    fn view_inspector_panel(&self) -> Element<'_, Message> {
        let Some(t) = &self.inspector_track else {
            return column![].into();
        };

        // Fixed geometry. The panel is 320 wide with 12px side padding, so the
        // content column is 296px. Every label is single-line, truncated to the
        // width it really gets and clipped by a fixed-width parent, so a long
        // name can never wrap, grow a row or push its neighbours.
        //   Summary card: 296 - 2*10 padding = 276 inner.
        //     text column: 276 - 48 art - 10 = 218px
        //       -> 16px title 24 chars, 14px artist 26 chars
        //     height: 48 art + 8 + 28 chips + 2*10 padding = 104
        //   Comment card: 296 - 2*8 padding = 280 inner.
        //     author slot: 280 - 28 avatar - 48 time - 2*6 = 192px
        //       -> 12px author 18 chars (fits)
        const HEADER_H: f32 = 32.0; // header row = close button hit box
        const ART: f32 = 48.0;
        const CHIP_H: f32 = 28.0; // quick-action chips and tab pills
        const CHIP_ICON_W: f32 = 16.0; // radio (15.75px) / copy / plus glyphs differ in width
        const SUMMARY_PAD: f32 = 10.0;
        const SUMMARY_GAP: f32 = 8.0;
        const SUMMARY_H: f32 = ART + SUMMARY_GAP + CHIP_H + SUMMARY_PAD * 2.0; // 104
        const AVATAR: f32 = 28.0;
        const TS_W: f32 = 48.0; // fits "120:00" at 12px + 4px each side
        const TS_H: f32 = 20.0;

        let header = container(
            row![
                text("Track Details")
                    .size(18)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright()),
                horizontal_space(),
                button(
                    container(
                        text(icons::XMARK)
                            .font(FA_SOLID)
                            .size(16)
                            .style(|_| bright())
                    )
                    .center_x(Length::Fixed(HEADER_H))
                    .center_y(Length::Fixed(HEADER_H)),
                )
                .on_press(Message::CloseTrackInspector)
                .padding(0)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    },
                    text_color: TEXT,
                    border: round(HEADER_H / 2.0),
                    ..button::Style::default()
                }),
            ]
            .height(Length::Fixed(HEADER_H))
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding(pad4(12.0, 8.0, 8.0, 12.0));

        let (artist_name, display_title) =
            t.display_artist_and_title(self.settings.prefer_artist_from_name);

        // The artist label has no colour of its own so it follows the button's
        // text_color: muted at rest, orange on hover. It opens the artist
        // shown, which may not be the uploader (see artist_link_msg).
        let artist_btn: Element<Message> = button(
            text(trunc(clean_username(&artist_name), 26))
                .size(14)
                .wrapping(text::Wrapping::None),
        )
        .on_press(artist_link_msg(t, &artist_name))
        .padding(0)
        .style(|_, status| button::Style {
            background: None,
            text_color: match status {
                button::Status::Hovered => ORANGE,
                _ => TEXT_MUTED,
            },
            ..button::Style::default()
        })
        .into();

        // Pill chip: fixed height, fixed-width icon slot, single-line label.
        // Tinted fill so it reads on the elevated summary card (BG_CARD is
        // the same shade as BG_ELEV and made the chips invisible at rest).
        let chip = |icon: &'static str, icon_color: Color, label: &'static str, msg: Message| {
            button(
                container(
                    row![
                        container(
                            text(icon)
                                .font(FA_SOLID)
                                .size(14)
                                .style(move |_| t_color(icon_color)),
                        )
                        .center_x(Length::Fixed(CHIP_ICON_W)),
                        text(label)
                            .size(12)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center),
                )
                .center_y(Length::Fixed(CHIP_H))
                .padding(Padding::from([0, 8])),
            )
            .on_press(msg)
            .padding(0)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_TINT_HI)),
                    _ => Some(Background::Color(BG_TINT)),
                },
                border: round(CHIP_H / 2.0),
                ..button::Style::default()
            })
        };

        let quick_actions = row![
            chip(
                icons::RADIO,
                ORANGE,
                "Radio",
                Message::StartTrackRadio(t.id, t.title.clone())
            ),
            chip(
                icons::COPY,
                TEXT_DIM,
                "Copy",
                Message::CopyTrackLink(t.clone())
            ),
            chip(
                icons::PLUS,
                TEXT_DIM,
                "Playlist",
                Message::OpenAddPopover(t.clone(), MenuAnchor::Inspector)
            ),
            button(
                container(
                    text(icons::ELLIPSIS)
                        .font(FA_SOLID)
                        .size(14)
                        .style(|_| t_color(TEXT_DIM))
                )
                .center_x(Length::Fixed(CHIP_H))
                .center_y(Length::Fixed(CHIP_H)),
            )
            .on_press(Message::OpenActionMenu(
                ActionMenu::Track(t.clone()),
                MenuAnchor::Inspector
            ))
            .padding(0)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_TINT_HI)),
                    _ => Some(Background::Color(BG_TINT)),
                },
                border: round(CHIP_H / 2.0),
                ..button::Style::default()
            }),
        ]
        .spacing(6)
        .height(Length::Fixed(CHIP_H))
        .align_y(iced::Alignment::Center);
        let quick_actions = self.with_menu(quick_actions, MenuAnchor::Inspector);

        // cover and title open the track page, like the player bar's
        let open_page = Message::OpenTrackPage(Box::new(t.clone()));
        let track_summary = container(
            column![
                row![
                    button(
                        self.artwork_tile(
                            t.artwork_url
                                .as_deref()
                                .or_else(|| t.user.as_ref().and_then(|u| u.avatar_url.as_deref())),
                            t.id,
                            &t.title,
                            ART,
                            false,
                        )
                    )
                    .on_press(open_page.clone())
                    .padding(0)
                    .style(|_, _| button::Style {
                        background: None,
                        ..button::Style::default()
                    }),
                    container(
                        column![
                            button(
                                text(trunc(&display_title, 24))
                                    .size(16)
                                    .wrapping(text::Wrapping::None),
                            )
                            .on_press(open_page)
                            .padding(0)
                            .style(|_, status| button::Style {
                                background: None,
                                text_color: match status {
                                    button::Status::Hovered => ORANGE,
                                    _ => TEXT,
                                },
                                ..button::Style::default()
                            }),
                            artist_btn,
                        ]
                        .spacing(2),
                    )
                    .width(Length::Fill)
                    .clip(true),
                ]
                .spacing(10)
                .height(Length::Fixed(ART))
                .align_y(iced::Alignment::Center),
                quick_actions,
            ]
            .spacing(SUMMARY_GAP),
        )
        .padding(SUMMARY_PAD)
        .width(Length::Fill)
        .height(Length::Fixed(SUMMARY_H))
        .style(|_| panel(BG_ELEV, 8.0));

        let comments_active = self.inspector_tab == InspectorTab::Comments;
        let likers_active = self.inspector_tab == InspectorTab::Likers;
        let reposters_active = self.inspector_tab == InspectorTab::Reposters;

        let tab_btn = |label: &'static str, active: bool, tab: InspectorTab| {
            button(
                container(
                    text(label)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| if active { orange_t() } else { dim() }),
                )
                .center_y(Length::Fixed(CHIP_H))
                .padding(Padding::from([0, 12])),
            )
            .on_press(Message::InspectorTabSelected(tab))
            .padding(0)
            .style(move |_, status| button::Style {
                background: if active {
                    Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.12)))
                } else {
                    match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    }
                },
                border: round(CHIP_H / 2.0),
                ..button::Style::default()
            })
        };

        let tabs = row![
            tab_btn("Comments", comments_active, InspectorTab::Comments),
            tab_btn("Likers", likers_active, InspectorTab::Likers),
            tab_btn("Reposters", reposters_active, InspectorTab::Reposters),
        ]
        .spacing(6)
        .height(Length::Fixed(CHIP_H))
        .align_y(iced::Alignment::Center);

        let content: Element<'_, Message> = if self.inspector_tasks_pending > 0 {
            container(horizontal_space())
                .padding(20)
                .center_x(Length::Fill)
                .into()
        } else if self.inspector_tab == InspectorTab::Comments {
            if self.inspector_comments.is_empty() {
                container(text("No comments yet").size(14).style(|_| muted()))
                    .padding(20)
                    .center_x(Length::Fill)
                    .into()
            } else {
                let mut col = column![].spacing(8);
                for c in &self.inspector_comments {
                    let author = c.author().to_string();
                    let author_id = c.user.as_ref().map(|u| u.id);
                    let ts = c.timestamp.unwrap_or(0).max(0) as u64;

                    // Linked author: no own colour, so it follows the button's
                    // text_color (white at rest, orange on hover).
                    let author_btn: Element<Message> = if let Some(uid) = author_id {
                        button(
                            text(trunc(&author, 18))
                                .size(12)
                                .wrapping(text::Wrapping::None),
                        )
                        .on_press(Message::OpenProfile(uid))
                        .padding(0)
                        .style(|_, status| button::Style {
                            background: None,
                            text_color: match status {
                                button::Status::Hovered => ORANGE,
                                _ => TEXT,
                            },
                            ..button::Style::default()
                        })
                        .into()
                    } else {
                        text(trunc(&author, 18))
                            .size(12)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright())
                            .into()
                    };

                    // Fixed-width, right-aligned timestamp so "0:45" and "12:34"
                    // line up down the list and share one hover box.
                    let ts_btn = button(
                        container(
                            text(fmt_time(ts))
                                .size(12)
                                .wrapping(text::Wrapping::None)
                                .style(|_| orange_t()),
                        )
                        .width(Length::Fixed(TS_W))
                        .height(Length::Fixed(TS_H))
                        .padding(Padding::from([0, 4]))
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Center)
                        .clip(true),
                    )
                    .on_press(Message::InspectorSeek(t.id, ts))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                            _ => None,
                        },
                        border: round(4.0),
                        ..button::Style::default()
                    });

                    let avatar = self.artwork_tile(
                        c.user.as_ref().and_then(|u| u.avatar_url.as_deref()),
                        c.user.as_ref().map(|u| u.id).unwrap_or(0),
                        c.author(),
                        AVATAR,
                        true,
                    );
                    // the avatar opens the commenter too, as on the track page
                    let avatar: Element<Message> = match author_id.filter(|id| *id != 0) {
                        Some(uid) => button(avatar)
                            .on_press(Message::OpenProfile(uid))
                            .padding(0)
                            .style(|_, _| button::Style {
                                background: None,
                                ..button::Style::default()
                            })
                            .into(),
                        None => avatar,
                    };

                    col = col.push(
                        container(
                            column![
                                row![
                                    avatar,
                                    container(author_btn).width(Length::Fill).clip(true),
                                    ts_btn,
                                ]
                                .spacing(6)
                                .height(Length::Fixed(AVATAR))
                                .align_y(iced::Alignment::Center),
                                text(&c.body).size(14).style(|_| dim()),
                            ]
                            .spacing(4),
                        )
                        .padding(8)
                        .width(Length::Fill)
                        .style(|_| panel(BG_CARD, 8.0)),
                    );
                }
                self.v_scrollable(col).height(Length::Fill).into()
            }
        } else if self.inspector_tab == InspectorTab::Likers {
            if self.inspector_favoriters.is_empty() {
                container(text("No likers found").size(14).style(|_| muted()))
                    .padding(20)
                    .center_x(Length::Fill)
                    .into()
            } else {
                let mut col = column![].spacing(6);
                for u in &self.inspector_favoriters {
                    col = col.push(self.user_card(u, 14));
                }
                self.v_scrollable(col).height(Length::Fill).into()
            }
        } else {
            if self.inspector_reposters.is_empty() {
                container(text("No reposters found").size(14).style(|_| muted()))
                    .padding(20)
                    .center_x(Length::Fill)
                    .into()
            } else {
                let mut col = column![].spacing(6);
                for u in &self.inspector_reposters {
                    col = col.push(self.user_card(u, 14));
                }
                self.v_scrollable(col).height(Length::Fill).into()
            }
        };

        let panel = container(
            column![
                header,
                container(column![track_summary, tabs, content].spacing(10))
                    .padding(Padding {
                        top: 0.0,
                        right: 12.0,
                        bottom: 12.0,
                        left: 12.0,
                    })
                    .height(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .width(Length::Fixed(320.0))
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_SIDE)),
            border: glass_border(8.0),
            shadow: Shadow::default(),
            ..container::Style::default()
        });

        container(panel)
            .padding(pad4(4.0, 8.0, 8.0, 0.0))
            .height(Length::Fill)
            .into()
    }

    fn view_queue_panel(&self) -> Element<'_, Message> {
        // Fixed geometry. The panel is 280 wide with 16px side padding, so the
        // content column is 248px. Every label is single-line, truncated to the
        // width it really gets and clipped by a fixed-width parent, so a long
        // title can never wrap, grow a row or push its neighbours.
        //   Now Playing: a centred cover up to 232px (8px free each side),
        //     then the 16px title and 12px artist, centred, cut to 248px.
        //   Next-up row: 248 - 2*8 padding = 232 inner.
        //     232 - 36 art - 40 duration - 2*8 = 140px text
        //       -> 14px title 17 chars, 12px artist 18 chars
        //     row height = 36 art + 2*6 padding = 48 (text: 18.2 + 2 + 15.6 = 35.8)
        const HEADER_H: f32 = 32.0; // header row = close button hit box
        const PILL_H: f32 = 28.0; // "Clear" pill
        const NP_ART_MAX: f32 = 232.0;
        const NP_TEXT_W: f32 = 248.0;
        const ROW_ART: f32 = 36.0;
        const ROW_PAD_Y: f32 = 6.0;
        const ROW_PAD_X: f32 = 8.0;
        const ROW_H: f32 = ROW_ART + ROW_PAD_Y * 2.0; // 48
        const DUR_W: f32 = 40.0; // fits "120:00" at 12px
        const SECTION_H: f32 = 20.0; // "Next Up" header line (14 * 1.3 = 18.2)
        const COUNT_W: f32 = 96.0; // fits "10000 tracks" at 12px

        let mut header_row = row![
            text("Queue")
                .size(18)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
            horizontal_space(),
        ]
        .spacing(8)
        .height(Length::Fixed(HEADER_H))
        .align_y(iced::Alignment::Center);

        if self.queue_pos + 1 < self.queue.len() {
            header_row = header_row.push(
                button(
                    // No own colour: the label follows the button's text_color,
                    // so it brightens on hover.
                    container(text("Clear").size(14).wrapping(text::Wrapping::None))
                        .center_y(Length::Fixed(PILL_H))
                        .padding(Padding::from([0, 12])),
                )
                .on_press(Message::ClearUpcomingQueue)
                .padding(0)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    },
                    text_color: match status {
                        button::Status::Hovered => TEXT,
                        _ => TEXT_MUTED,
                    },
                    border: round(PILL_H / 2.0),
                    ..button::Style::default()
                }),
            );
        }

        let close_btn = button(
            container(
                text(icons::XMARK)
                    .font(FA_SOLID)
                    .size(16)
                    .style(|_| bright()),
            )
            .center_x(Length::Fixed(HEADER_H))
            .center_y(Length::Fixed(HEADER_H)),
        )
        .on_press(Message::ToggleQueue)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: TEXT,
            border: round(HEADER_H / 2.0),
            ..button::Style::default()
        });

        let header = container(header_row.push(close_btn))
            .width(Length::Fill)
            .padding(pad4(12.0, 12.0, 8.0, 16.0));

        // Now Playing, like Spotify's now-playing view: the cover big and
        // centred, the title (with its saved speed) and the artist centred
        // under it, all of them links. The cover shrinks in a short window so
        // Next Up keeps room for a few rows. With nothing loaded there's no
        // section at all.
        let now_playing_section: Option<Element<'_, Message>> = self.playing_id.map(|id| {
            let track = self.playing_track();
            let art = (self.window_size.height - 479.0).clamp(96.0, NP_ART_MAX);
            let speed_label = self.track_speeds.get(&id).map(|&s| speed_badge_label(s));
            let badge_room = speed_label.as_deref().map_or(0.0, speed_badge_room);
            let title_label = trunc_px(&self.playing_title, NP_TEXT_W - badge_room, 16.0);
            let title: Element<'_, Message> = match &track {
                Some(t) => hover_link(
                    title_label,
                    16,
                    ORANGE,
                    ORANGE,
                    self.pb_hover == Some(PbLink::QueueTitle),
                    Message::OpenTrackPage(Box::new(t.clone())),
                    Message::PlayerBarHover(Some(PbLink::QueueTitle)),
                    Message::PlayerBarHover(None),
                ),
                None => text(title_label)
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(|_| orange_t())
                    .into(),
            };
            let title: Element<'_, Message> = match speed_label {
                Some(label) => row![title, speed_badge(label)]
                    .spacing(SPEED_BADGE_GAP)
                    .align_y(iced::Alignment::Center)
                    .into(),
                None => title,
            };
            let artist_label = trunc_px(clean_username(&self.playing_artist), NP_TEXT_W, 12.0);
            let artist: Element<'_, Message> = match &track {
                Some(t) if !self.playing_artist.is_empty() => hover_link(
                    artist_label,
                    12,
                    TEXT_MUTED,
                    TEXT,
                    self.pb_hover == Some(PbLink::QueueArtist),
                    artist_link_msg(t, &self.playing_artist),
                    Message::PlayerBarHover(Some(PbLink::QueueArtist)),
                    Message::PlayerBarHover(None),
                ),
                _ => text(artist_label)
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted())
                    .into(),
            };
            let cover: Element<'_, Message> = match &track {
                Some(t) => {
                    button(self.artwork_tile(t.artwork_or_avatar(), t.id, &t.title, art, false))
                        .on_press(Message::OpenTrackPage(Box::new(t.clone())))
                        .padding(0)
                        .style(|_, _| button::Style {
                            background: None,
                            ..button::Style::default()
                        })
                        .into()
                }
                None => cover_tile(id, &self.playing_title, art, false),
            };
            column![
                text("Now Playing")
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| orange_t()),
                container(cover).center_x(Length::Fill),
                column![
                    container(title).center_x(Length::Fill).clip(true),
                    container(artist).center_x(Length::Fill).clip(true),
                ]
                .spacing(2),
            ]
            .spacing(10)
            .into()
        });

        // Next Up can be the whole queue (all of Liked Tracks after playing
        // it): only the rows on screen are built (see virtual_rows).
        let queue_row = |idx: usize, track: &Track| -> Element<'_, Message> {
            let artist = track
                .user
                .as_ref()
                .map(|u| clean_username(&u.username).to_string())
                .unwrap_or_default();
            let dur = track.duration.map(fmt_time).unwrap_or_else(|| "—:—".into());
            // 140px of text; a saved speed's badge follows the title
            let speed_label = self
                .track_speeds
                .get(&track.id)
                .map(|&s| speed_badge_label(s));
            let badge_room = speed_label.as_deref().map_or(0.0, speed_badge_room);
            let title = text(trunc_px(&track.title, 140.0 - badge_room, 14.0))
                .size(14)
                .wrapping(text::Wrapping::None)
                .style(|_| bright());
            let title: Element<'_, Message> = match speed_label {
                Some(label) => row![title, speed_badge(label)]
                    .spacing(SPEED_BADGE_GAP)
                    .align_y(iced::Alignment::Center)
                    .into(),
                None => title.into(),
            };
            button(
                row![
                    self.artwork_tile(
                        track.artwork_or_avatar(),
                        track.id,
                        &track.title,
                        ROW_ART,
                        false
                    ),
                    container(
                        column![
                            title,
                            // the uploader, a link to their profile (it
                            // takes its click before the row's play)
                            match track.user.as_ref().filter(|u| u.id != 0) {
                                Some(user) => self.owner_link(
                                    user,
                                    &format!("queue:{idx}:{}", track.id),
                                    140.0,
                                ),
                                None => text(trunc_px(&artist, 140.0, 12.0))
                                    .size(12)
                                    .wrapping(text::Wrapping::None)
                                    .style(|_| muted())
                                    .into(),
                            },
                        ]
                        .spacing(2),
                    )
                    .width(Length::Fill)
                    .clip(true),
                    container(
                        text(dur)
                            .size(12)
                            .wrapping(text::Wrapping::None)
                            .style(|_| muted()),
                    )
                    .width(Length::Fixed(DUR_W))
                    .align_x(iced::alignment::Horizontal::Right)
                    .clip(true),
                ]
                .spacing(8)
                .height(Length::Fixed(ROW_ART))
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::PlayQueueTrack(idx))
            .padding(pad4(ROW_PAD_Y, ROW_PAD_X, ROW_PAD_Y, ROW_PAD_X))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_H))
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(4.0),
                text_color: TEXT,
                ..button::Style::default()
            })
            .into()
        };
        let upcoming_start = (self.queue_pos + 1).min(self.queue.len());
        let upcoming = &self.queue[upcoming_start..];
        let upcoming_content: Element<'_, Message> = if upcoming.is_empty() {
            container(
                text("No upcoming tracks in queue")
                    .size(14)
                    .style(|_| muted()),
            )
            .padding(16)
            .center_x(Length::Fill)
            .into()
        } else {
            let list = self.virtual_rows(ListKey::Queue, upcoming, ROW_H, 4.0, |k, track| {
                queue_row(upcoming_start + k, track)
            });
            self.v_scrollable(list).height(Length::Fill).into()
        };

        let next_section = column![
            row![
                text("Next Up")
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
                horizontal_space(),
                {
                    let upcoming = self.queue.len().saturating_sub(self.queue_pos + 1);
                    container(
                        text(if upcoming > 0 {
                            format!("{upcoming} tracks")
                        } else {
                            String::new()
                        })
                        .size(12)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                    )
                    .width(Length::Fixed(COUNT_W))
                    .align_x(iced::alignment::Horizontal::Right)
                    .clip(true)
                },
            ]
            .height(Length::Fixed(SECTION_H))
            .align_y(iced::Alignment::Center),
            upcoming_content,
        ]
        .spacing(8)
        .height(Length::Fill);

        let panel = container(
            column![
                header,
                container(
                    column![]
                        .push_maybe(now_playing_section)
                        .push(next_section)
                        .spacing(14),
                )
                .padding(Padding {
                    top: 0.0,
                    right: 16.0,
                    bottom: 16.0,
                    left: 16.0,
                })
                .height(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .width(Length::Fixed(280.0))
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_SIDE)),
            border: glass_border(8.0),
            shadow: Shadow::default(),
            ..container::Style::default()
        });

        container(panel)
            .padding(pad4(4.0, 8.0, 8.0, 0.0))
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        // Full-height Spotify-style "Your Library" ("Моя медиатека"): Liked
        // Tracks, your and liked playlists, mixes, radio stations and followed
        // artists, with Spotify's filter chips over them.
        //
        // Every row has a fixed geometry so long names never reflow the list:
        //   sidebar 260 - 2 x 8 gutter - 2 x 8 card padding = 228px row width
        //   row  = 6px padding on every side + 44px cover  = 56px tall
        //   text = 228 - 2 x 6 - 44 - 10 gap              = 162px wide
        //   16px title: 162 / (16 * 0.56) ~ 18 glyphs     -> 17 + ellipsis
        //   12px meta : 162 / (12 * 0.56) ~ 24 glyphs for
        //               "Playlist • {author} • N songs"; the author gets what
        //               is left so the song count is never pushed out.
        const COVER: f32 = 44.0;
        const ROW_PAD: f32 = 6.0;
        const TEXT_W: f32 = 162.0; // 228 row - 12 padding - 44 cover - 10 gap
        const HEADER_BTN: f32 = 32.0;
        const CHIP_H: f32 = 32.0;

        let library_active = self.tab == Tab::Library;
        let lib_color = if library_active { TEXT } else { TEXT_DIM };

        // What Your Library holds: playlists (yours, liked, then downloaded
        // ones that are neither), mixes and stations (offline only what's
        // downloaded plays, so those hide), followed artists.
        let mut seen_ids = std::collections::HashSet::new();
        let mut playlists: Vec<Playlist> = Vec::new();
        for pl in self.my_playlists.iter().chain(&self.liked_playlists) {
            if seen_ids.insert(pl.id) {
                playlists.push(pl.clone());
            }
        }
        // downloaded playlists that aren't yours or saved (someone's
        // "digicore type"), so they can be found again
        playlists.extend(
            self.offline_store
                .playlists
                .iter()
                .filter(|pd| {
                    self.downloaded_playlist_ids.contains(&pd.id.to_string())
                        && !seen_ids.contains(&pd.id)
                })
                .map(|pd| Playlist {
                    id: pd.id,
                    urn: Some(pd.id_or_urn.clone()),
                    title: pd.title.clone(),
                    artwork_url: pd.artwork_url.clone(),
                    user: Some(UserMini {
                        username: pd.author.clone(),
                        ..Default::default()
                    }),
                    track_count: Some(pd.track_count as u64),
                    ..Default::default()
                }),
        );
        let online = !self.settings.offline_mode;
        // mixes and stations only once saved (the (+) on their page)
        let saved = |items: &[LibraryItem]| -> Vec<LibraryItem> {
            items
                .iter()
                .filter(|i| online && self.liked_system_urns.contains(&i.urn))
                .cloned()
                .collect()
        };
        let saved_mixes = saved(&self.library_mixes);
        let saved_radios = saved(&self.library_radios);
        let mixes: &[LibraryItem] = &saved_mixes;
        let radios: &[LibraryItem] = &saved_radios;
        let artists = &self.my_followings;
        let shows = |kind: LibraryFilter| self.library_filter.is_none_or(|f| f == kind);

        // Spotify collapse: the whole sidebar shrinks to a 72px icon rail —
        // covers only, no text. Every row keeps a big enough hit box to click.
        if self.library_collapsed {
            let chevron = if library_active { TEXT } else { TEXT_DIM };
            let header_btn = |glyph: &'static str, msg: Message| {
                button(
                    container(
                        text(glyph)
                            .font(FA_SOLID)
                            .size(16)
                            .style(move |_| t_color(chevron)),
                    )
                    .center_x(Length::Fixed(40.0))
                    .center_y(Length::Fixed(40.0)),
                )
                .on_press(msg)
                .padding(0)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(BG_HOVER))
                        }
                        _ => None,
                    },
                    border: round(8.0),
                    ..button::Style::default()
                })
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
            };
            let cover_btn = |tile: Element<'static, Message>, msg: Message| {
                button(tile)
                    .on_press(msg)
                    .padding(0)
                    .style(|_, _| button::Style {
                        background: None,
                        ..button::Style::default()
                    })
            };

            let mut rail = column![].spacing(6);
            rail = rail.push(header_btn(icons::BARS, Message::ToggleLibraryCollapsed));
            rail = rail.push(header_btn(icons::PLUS, Message::SidebarCreatePlaylist));
            rail = rail.push(header_btn(icons::HEART, Message::Tab(Tab::Library)));
            if shows(LibraryFilter::Playlists) {
                for pl in &playlists {
                    rail = rail.push(cover_btn(
                        self.artwork_tile(pl.artwork_or_avatar(), pl.id, &pl.title, COVER, false),
                        Message::OpenPlaylist(pl.id_or_urn()),
                    ));
                }
            }
            for (kind, items) in [
                (LibraryFilter::Mixes, mixes),
                (LibraryFilter::Radio, radios),
            ] {
                if shows(kind) {
                    for item in items {
                        rail = rail.push(cover_btn(
                            self.artwork_tile(
                                item.artwork_url.as_deref(),
                                parse_trailing_id(&item.urn),
                                &item.title,
                                COVER,
                                false,
                            ),
                            Message::OpenPlaylist(item.urn.clone()),
                        ));
                    }
                }
            }
            if shows(LibraryFilter::Artists) {
                for artist in artists {
                    rail = rail.push(cover_btn(
                        self.artwork_tile(
                            artist.avatar_url.as_deref(),
                            artist.id,
                            clean_username(&artist.username),
                            COVER,
                            true,
                        ),
                        Message::OpenProfile(artist.id),
                    ));
                }
            }

            let rail_card = container(
                self.v_scrollable(rail)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_MUTED)),
                border: Border {
                    radius: border::Radius::from(8.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..container::Style::default()
            });

            return container(rail_card)
                .width(Length::Fixed(72.0))
                .height(Length::Fill)
                .padding(pad4(4.0, 8.0, 8.0, 8.0))
                .style(|_| container::Style {
                    background: Some(Background::Color(BG)),
                    ..container::Style::default()
                })
                .into();
        }

        // Header: 32px controls, 4px above/below -> fixed 40px strip. The
        // left inset matches the rows' padding so the icon lines up with
        // the covers below.
        let library_header = row![
            button(
                row![
                    container(
                        text(icons::BARS)
                            .font(FA_SOLID)
                            .size(16)
                            .style(move |_| t_color(lib_color)),
                    )
                    .center_x(Length::Fixed(20.0)),
                    text("Your Library")
                        .size(16)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| t_color(lib_color)),
                ]
                .spacing(10)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center)
            )
            .on_press(Message::ToggleLibraryCollapsed)
            .padding(0)
            .height(Length::Fixed(HEADER_BTN))
            .clip(true)
            .style(|_, _| button::Style {
                background: None,
                ..button::Style::default()
            }),
            horizontal_space(),
            button(
                container(
                    text(icons::PLUS)
                        .font(FA_SOLID)
                        .size(16)
                        .style(|_| text::Style {
                            color: Some(TEXT_MUTED)
                        })
                )
                .center_x(Length::Fixed(HEADER_BTN))
                .center_y(Length::Fixed(HEADER_BTN))
            )
            .on_press(Message::SidebarCreatePlaylist)
            .padding(0)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed =>
                        Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(HEADER_BTN / 2.0),
                ..button::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center)
        .height(Length::Fixed(HEADER_BTN + 8.0))
        .padding(pad4(4.0, 0.0, 4.0, ROW_PAD));

        // Spotify's filter chips: tinted pills, the one on white with black
        // text; again turns it off. A chip shows only when it has something.
        // They scroll sideways (the wheel works) under a fade into the card.
        let chip = |label: &'static str, kind: LibraryFilter| {
            let on = self.library_filter == Some(kind);
            button(
                container(text(label).size(14).wrapping(text::Wrapping::None))
                    .padding(pad4(0.0, 12.0, 0.0, 12.0))
                    .center_y(Length::Fixed(CHIP_H)),
            )
            .on_press(Message::LibraryFilterPicked(kind))
            .padding(0)
            .style(move |_, status| button::Style {
                background: Some(Background::Color(match (on, status) {
                    (true, _) => TEXT,
                    (false, button::Status::Hovered | button::Status::Pressed) => BG_TINT_HI,
                    (false, _) => BG_TINT,
                })),
                text_color: if on { Color::BLACK } else { TEXT },
                border: round(CHIP_H / 2.0),
                ..button::Style::default()
            })
        };
        let mut chips = row![].spacing(8);
        for (label, kind, has) in [
            ("Playlists", LibraryFilter::Playlists, true),
            ("Mixes", LibraryFilter::Mixes, !mixes.is_empty()),
            ("Radio", LibraryFilter::Radio, !radios.is_empty()),
            ("Artists", LibraryFilter::Artists, !artists.is_empty()),
        ] {
            if has || self.library_filter == Some(kind) {
                chips = chips.push(chip(label, kind));
            }
        }
        let chip_rail = stack(vec![
            scrollable(container(chips).padding(pad4(0.0, 24.0, 0.0, ROW_PAD)))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(0).scroller_width(0),
                ))
                .width(Length::Fill)
                .into(),
            row![
                horizontal_space(),
                container(iced::widget::Space::new(
                    Length::Fixed(24.0),
                    Length::Fixed(CHIP_H)
                ))
                .style(|_| container::Style {
                    background: Some(Background::Gradient(iced::Gradient::Linear(
                        iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                            .add_stop(0.0, Color { a: 0.0, ..BG_MUTED })
                            .add_stop(1.0, BG_MUTED),
                    ))),
                    ..container::Style::default()
                }),
            ]
            .into(),
        ])
        .height(Length::Fixed(CHIP_H));

        // Inline create form: pressing "+" swaps the list header for a title
        // input; Enter creates, Cancel closes — simple Spotify-like
        // inline flow without a modal.
        let create_form = if self.sidebar_create_mode {
            let mut form = column![text_input("Playlist name…", &self.sidebar_create_title)
                .on_input(Message::SidebarCreatePlaylistTitle)
                .on_submit(Message::SidebarCreatePlaylistSubmit)
                .padding(Padding::from([6, 10]))
                .size(14)
                .style(|_, status| text_input::Style {
                    background: Background::Color(BG_INPUT),
                    border: match status {
                        text_input::Status::Focused => Border {
                            radius: border::Radius::from(6.0),
                            width: 1.0,
                            color: ORANGE,
                        },
                        _ => round(6.0),
                    },
                    icon: Color::WHITE,
                    placeholder: TEXT_MUTED,
                    value: TEXT,
                    selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
                }),]
            .spacing(6)
            .padding(pad4(4.0, ROW_PAD, 4.0, ROW_PAD));
            form = form.push(
                row![
                    button(text("Create").size(13).style(|_| orange_t()))
                        .on_press(Message::SidebarCreatePlaylistSubmit)
                        .padding(pad4(4.0, 10.0, 4.0, 10.0))
                        .style(|_, status| button::Style {
                            background: match status {
                                button::Status::Hovered | button::Status::Pressed =>
                                    Some(Background::Color(BG_HOVER)),
                                _ => Some(Background::Color(BG_TINT)),
                            },
                            border: round(6.0),
                            ..button::Style::default()
                        }),
                    button(text("Cancel").size(13).style(|_| muted()))
                        .on_press(Message::SidebarCreatePlaylistCancel)
                        .padding(pad4(4.0, 10.0, 4.0, 10.0))
                        .style(|_, status| button::Style {
                            background: match status {
                                button::Status::Hovered | button::Status::Pressed =>
                                    Some(Background::Color(BG_HOVER)),
                                _ => None,
                            },
                            border: round(6.0),
                            ..button::Style::default()
                        }),
                ]
                .spacing(8),
            );
            form
        } else {
            column![].spacing(4)
        };

        let mut list = column![create_form].spacing(4);

        if shows(LibraryFilter::Playlists) {
            // Pinned Liked Tracks
            let liked_meta = if self.library.is_empty() {
                "Playlist".to_string()
            } else {
                format!("Playlist • {} tracks", self.library.len())
            };
            list = list.push(library_row(
                liked_tracks_tile(COVER),
                "Liked Tracks",
                false,
                &liked_meta,
                Message::Tab(Tab::Library),
                self.settings.compact_library,
            ));

            for pl in &playlists {
                let author = pl
                    .user
                    .as_ref()
                    .map(|u| clean_username(&u.username))
                    .unwrap_or("SoundCloud");
                let count_label = pl
                    .track_count
                    .filter(|c| *c > 0)
                    .map(|c| format!(" • {} songs", c))
                    .unwrap_or_default();
                // the song count always shows; the author gets what's left
                let author_w =
                    (TEXT_W - text_px("Playlist • ", 12.0) - text_px(&count_label, 12.0)).max(40.0);
                let meta = format!(
                    "Playlist • {}{}",
                    trunc_px(author, author_w, 12.0),
                    count_label
                );
                // Your Library stays non-destructive: no delete on a row.
                list = list.push(library_row(
                    self.artwork_tile(pl.artwork_or_avatar(), pl.id, &pl.title, COVER, false),
                    &pl.title,
                    // downloaded: the green check before the title
                    self.downloaded_playlist_ids.contains(&pl.id.to_string()),
                    &meta,
                    Message::OpenPlaylist(pl.id_or_urn()),
                    self.settings.compact_library,
                ));
            }
        }

        // Mixes and stations: SoundCloud system playlists, opened by URN.
        for (kind, items, what) in [
            (LibraryFilter::Mixes, mixes, "Mix • SoundCloud"),
            (LibraryFilter::Radio, radios, "Radio • SoundCloud"),
        ] {
            if !shows(kind) {
                continue;
            }
            for item in items {
                list = list.push(library_row(
                    // a station shows its artist's round avatar, as Spotify
                    // shows an artist's radio
                    self.artwork_tile(
                        item.artwork_url.as_deref(),
                        parse_trailing_id(&item.urn),
                        &item.title,
                        COVER,
                        is_station_urn(&item.urn),
                    ),
                    &item.title,
                    false,
                    what,
                    Message::OpenPlaylist(item.urn.clone()),
                    self.settings.compact_library,
                ));
            }
        }

        // Followed artists, most tracks first (sorted when they load)
        if shows(LibraryFilter::Artists) {
            for artist in artists {
                // the number the list is sorted by
                let count_label = match self.settings.artist_sort {
                    crate::config::ArtistSort::Subscribers => artist
                        .followers_count
                        .filter(|c| *c > 0)
                        .map(|c| format!("Artist • {} followers", fmt_count(c))),
                    _ => artist
                        .track_count
                        .filter(|c| *c > 0)
                        .map(|c| format!("Artist • {} tracks", c)),
                }
                .unwrap_or_else(|| "Artist".to_string());
                list = list.push(library_row(
                    self.artwork_tile(
                        artist.avatar_url.as_deref(),
                        artist.id,
                        clean_username(&artist.username),
                        COVER,
                        true,
                    ),
                    clean_username(&artist.username),
                    false,
                    &count_label,
                    Message::OpenProfile(artist.id),
                    self.settings.compact_library,
                ));
            }
        }

        let library_scroll = self
            .v_scrollable(list)
            .width(Length::Fill)
            .height(Length::Fill);

        let library_card = container(
            column![library_header, chip_rail, library_scroll]
                .spacing(6)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(8)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_MUTED)),
            border: Border {
                radius: border::Radius::from(8.0),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        });

        // Top inset 4 = the content island's top inset in view(), so the two
        // panels start on the same line under the title bar.
        container(library_card)
            .width(Length::Fixed(260.0))
            .height(Length::Fill)
            .padding(pad4(4.0, 8.0, 8.0, 8.0))
            .style(|_| container::Style {
                background: Some(Background::Color(BG)),
                ..container::Style::default()
            })
            .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        match self.tab {
            Tab::Home => self.view_home(),
            Tab::Search => self.view_search(),
            Tab::Library => self.view_library(),
            Tab::Settings => self.view_settings(),
            Tab::Playlist => self.view_playlist(),
            Tab::Profile => self.view_profile(),
            Tab::Track => self.view_track_page(),
        }
    }

    /// A page (or part of one) still loading: a spinning ring, centred.
    fn loading_view(&self) -> Element<'static, Message> {
        let turn = (self.anim_start.elapsed().as_secs_f32() * 1.1).fract();
        container(
            canvas(Spinner { turn })
                .width(Length::Fixed(32.0))
                .height(Length::Fixed(32.0)),
        )
        .width(Length::Fill)
        .padding(48)
        .center_x(Length::Fill)
        .into()
    }

    /// The open page couldn't load: why, and a Retry.
    fn error_view(&self, why: &str, retry: Message) -> Element<'static, Message> {
        container(
            column![
                text(why.to_string()).size(16).style(|_| dim()),
                pill_btn("Retry", ORANGE, retry),
            ]
            .spacing(16)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding(48)
        .center_x(Length::Fill)
        .into()
    }

    /// The error view, if the open page failed to load.
    fn page_error_view(&self) -> Option<Element<'static, Message>> {
        self.page_error
            .as_ref()
            .filter(|(tab, _, _)| *tab == self.tab)
            .map(|(_, why, retry)| self.error_view(why, (**retry).clone()))
    }

    fn section_header(&self, title: &str) -> Element<'_, Message> {
        // One line, never wraps: an over-long heading is cut (trunc + clip)
        // instead of dropping to a second line and pushing everything below
        // (or a sibling in the same row) around. 24px * 1.3 = 31.2px line.
        container(
            text(trunc(title, 48))
                .size(24)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
        )
        .clip(true)
        .into()
    }

    /// The stories strip's bubbles, one per artist (theirs are together in
    /// the list): where their stories start, the one a click opens (their
    /// first unread, else their first), and whether any is unread.
    fn story_groups(&self) -> Vec<(usize, usize, bool)> {
        let mut groups: Vec<(usize, usize, bool)> = Vec::new();
        for (i, s) in self.stories.iter().enumerate() {
            let unread = !self.stories_read.contains(&s.track_id);
            match groups.last_mut() {
                Some((first, open, any)) if self.stories[*first].user_id == s.user_id => {
                    if unread && !*any {
                        *open = i;
                    }
                    *any |= unread;
                }
                _ => groups.push((i, i, unread)),
            }
        }
        groups
    }

    /// Where the artist of story `idx` has their stories in the list.
    fn story_group_range(&self, idx: usize) -> std::ops::Range<usize> {
        let Some(user) = self.stories.get(idx).map(|s| s.user_id) else {
            return 0..0;
        };
        let mut start = idx;
        while start > 0 && self.stories[start - 1].user_id == user {
            start -= 1;
        }
        let mut end = idx + 1;
        while end < self.stories.len() && self.stories[end].user_id == user {
            end += 1;
        }
        start..end
    }

    fn view_stories_widget(&self) -> Option<Element<'_, Message>> {
        if self.stories.is_empty() {
            return None;
        }
        let groups = self.story_groups();

        if !self.stories_expanded {
            // Collapsed: Telegram Mobile style capsule with 3 overlapping circles + "X Stories" + ⌄
            // Geometry: 32px avatar + 2px ring padding = 36px circle, pill is
            // 36 + 2 * 4 = 44px tall and fully round.
            const AV: f32 = 32.0;
            const RING: f32 = AV + 4.0;
            const PILL_PAD_Y: f32 = 4.0;
            const PILL_H: f32 = RING + 2.0 * PILL_PAD_Y;
            // "N Stories" + chevron live in a fixed slot, so a new count
            // (9 -> 10 -> 100) never changes the pill's width. Slot = label
            // (<= 11 chars, "999 Stories") + 8 gap + 14px chevron.
            const LABEL_SLOT_W: f32 = 112.0;

            let mut avatar_row = row![].spacing(6).align_y(iced::Alignment::Center);
            for (idx, &(first, _, unread)) in groups.iter().take(3).enumerate() {
                let story = &self.stories[first];
                let av = self.artwork_tile(
                    story.avatar_url.as_deref().or(story.artwork_url.as_deref()),
                    story.user_id + idx as i64,
                    clean_username(&story.username),
                    AV,
                    true,
                );
                // Orange ring = an unread update, grey ring = all seen.
                let ring = if unread {
                    ORANGE
                } else {
                    Color::from_rgb(0.35, 0.35, 0.35)
                };
                let bordered_av = container(av)
                    .padding(2)
                    .width(Length::Fixed(RING))
                    .height(Length::Fixed(RING))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::BLACK)),
                        border: Border {
                            radius: border::Radius::from(RING / 2.0),
                            width: 2.0,
                            color: ring,
                        },
                        ..container::Style::default()
                    });
                avatar_row = avatar_row.push(bordered_av);
            }

            let count_label = format!("{} Stories", self.stories.len());
            let label_slot = container(
                row![
                    text(trunc(&count_label, 11))
                        .size(16)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                    text(icons::CHEVRON_DOWN)
                        .font(FA_SOLID)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| orange_t()),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .center_x(Length::Fixed(LABEL_SLOT_W))
            .clip(true);

            let capsule = button(
                row![
                    avatar_row,
                    horizontal_space().width(Length::Fixed(4.0)),
                    label_slot,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleStoriesExpanded)
            .padding(pad4(PILL_PAD_Y, 12.0, PILL_PAD_Y, 12.0))
            .height(Length::Fixed(PILL_H))
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => Some(Background::Color(BG_CARD)),
                },
                border: Border {
                    radius: border::Radius::from(PILL_H / 2.0),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 0.33, 0.0, 0.35),
                },
                ..button::Style::default()
            });

            Some(capsule.into())
        } else {
            // Expanded: Row of up to 12 story circles with artist nicknames below each + collapse button
            // Every item is the same fixed tile: 6px padding on all sides, a
            // 56px ring centred over an 84px one-line nickname slot. A long
            // nickname is cut (trunc + clip) instead of widening its tile and
            // shifting every circle after it.
            // Height = 6 + 56 ring + 6 + 16 nick (12px * 1.3) + 6 = 90.
            const AV: f32 = 52.0;
            const RING: f32 = AV + 4.0;
            const ITEM_PAD: f32 = 6.0;
            const ITEM_INNER_W: f32 = 84.0;
            const NICK_GAP: f32 = 6.0;
            const NICK_H: f32 = 16.0;
            const ITEM_W: f32 = ITEM_INNER_W + 2.0 * ITEM_PAD;
            const ITEM_H: f32 = ITEM_PAD + RING + NICK_GAP + NICK_H + ITEM_PAD;

            let mut story_items = row![].spacing(4).align_y(iced::Alignment::Start);
            for (idx, &(first, open, unread)) in groups.iter().take(12).enumerate() {
                let story = &self.stories[first];
                let av = self.artwork_tile(
                    story.avatar_url.as_deref().or(story.artwork_url.as_deref()),
                    story.user_id + idx as i64,
                    clean_username(&story.username),
                    AV,
                    true,
                );
                // Orange ring = an unread update, grey ring = all seen.
                let ring = if unread {
                    ORANGE
                } else {
                    Color::from_rgb(0.35, 0.35, 0.35)
                };
                let bordered_circle = container(av)
                    .padding(2)
                    .width(Length::Fixed(RING))
                    .height(Length::Fixed(RING))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::BLACK)),
                        border: Border {
                            radius: border::Radius::from(RING / 2.0),
                            width: 2.5,
                            color: ring,
                        },
                        ..container::Style::default()
                    });

                let nick = clean_username(&story.username).to_string();

                let item_btn = button(
                    column![
                        bordered_circle,
                        container(
                            text(trunc(&nick, 11))
                                .size(12)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright()),
                        )
                        .center_x(Length::Fixed(ITEM_INNER_W))
                        .height(Length::Fixed(NICK_H))
                        .clip(true),
                    ]
                    .spacing(NICK_GAP)
                    .width(Length::Fixed(ITEM_INNER_W))
                    .align_x(iced::Alignment::Center),
                )
                .on_press(Message::OpenStory(open))
                .padding(ITEM_PAD)
                .width(Length::Fixed(ITEM_W))
                .height(Length::Fixed(ITEM_H))
                .clip(true)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    },
                    border: Border {
                        radius: border::Radius::from(8.0),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..button::Style::default()
                });

                story_items = story_items.push(item_btn);
            }

            let collapse_btn = button(
                row![
                    text("Collapse")
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                    text(icons::CHEVRON_UP)
                        .font(FA_SOLID)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| orange_t()),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleStoriesExpanded)
            .padding(Padding::from([4, 8]))
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(4.0),
                ..button::Style::default()
            });

            let container_box = container(
                column![
                    row![
                        text("SoundCloud Stories")
                            .size(16)
                            .wrapping(text::Wrapping::None)
                            .style(|_| orange_t()),
                        horizontal_space(),
                        collapse_btn,
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                    self.h_scrollable(story_items).width(Length::Fill),
                ]
                .spacing(10),
            )
            .padding(12)
            .width(Length::Fill)
            .clip(true)
            .style(|_| panel(BG_CARD, 8.0));

            Some(container_box.into())
        }
    }

    fn view_home(&self) -> Element<'_, Message> {
        // Shelf card geometry. The card is 188px wide with the same 8px padding
        // on every side, so the 172px cover fills the inner width exactly (no
        // strip on the right). The fixed height is exactly the content:
        // 8 + 172 cover + 8 + 21 title (16px * 1.3) + 2 + 16 subtitle (12px * 1.3) + 8 = 235.
        const CARD_W: f32 = 188.0;
        const CARD_PAD: f32 = 8.0;
        const COVER: f32 = CARD_W - 2.0 * CARD_PAD;
        const COVER_GAP: f32 = 8.0;
        const TITLE_H: f32 = 21.0;
        const LINE_GAP: f32 = 2.0;
        const SUB_H: f32 = 16.0;
        const CARD_H: f32 = CARD_PAD + COVER + COVER_GAP + TITLE_H + LINE_GAP + SUB_H + CARD_PAD;

        let offline = self.settings.offline_mode;
        let header = column![
            text(if offline { "Offline" } else { "Good day" })
                .size(32)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
            text(if offline {
                "Your downloaded music"
            } else {
                "SoundCloud feed & featured picks"
            })
            .size(16)
            .wrapping(text::Wrapping::None)
            .style(|_| muted()),
        ]
        .spacing(4)
        .width(Length::Fill)
        .clip(true);

        if self.home.is_empty() && !offline {
            let body = match &self.home_error {
                Some(why) => self.error_view(why, Message::ReloadHome),
                None => self.loading_view(),
            };
            return column![header, body]
                .spacing(20)
                .padding(pad4(8.0, 24.0, 24.0, 24.0))
                .into();
        }
        if offline && self.home.is_empty() {
            return column![
                header,
                text("Nothing downloaded yet. While online, use Download on a track, playlist, artist or Liked Tracks.")
                    .size(16)
                    .style(|_| dim()),
            ]
            .spacing(20)
            .padding(pad4(8.0, 24.0, 24.0, 24.0))
            .into();
        }

        let mut sections = column![].spacing(24);
        for sec in &self.home {
            // Skip empty shelves entirely — no orphaned section headers.
            let is_empty = match &sec.shelf {
                HomeShelf::Playlists(playlists) => playlists.is_empty(),
                HomeShelf::Tracks(tracks) => tracks.is_empty(),
            };
            if is_empty {
                continue;
            }
            let title_text = if sec.title.is_empty() {
                "Featured".to_string()
            } else {
                sec.title.clone()
            };
            // One line: a long shelf title is cut instead of wrapping and
            // pushing the shelf below it down.
            let title_header = container(
                text(trunc(&title_text, 40))
                    .size(22)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright()),
            )
            .width(Length::Fill)
            .clip(true);
            match &sec.shelf {
                HomeShelf::Playlists(playlists) => {
                    let mut cards = row![].spacing(14).padding(pad4(4.0, 12.0, 8.0, 0.0));
                    for sp in playlists.iter().take(20) {
                        cards = cards.push(
                            button(
                                column![
                                    self.artwork_tile(
                                        sp.artwork_or_avatar(),
                                        1,
                                        &sp.title,
                                        COVER,
                                        false,
                                    ),
                                    column![
                                        container(
                                            text(trunc_px(&sp.title, COVER, 16.0))
                                                .size(16)
                                                .wrapping(text::Wrapping::None)
                                                .style(|_| bright()),
                                        )
                                        .width(Length::Fixed(COVER))
                                        .height(Length::Fixed(TITLE_H))
                                        .clip(true),
                                        container(match &sp.owner {
                                            Some(owner) => {
                                                self.owner_link(owner, &sp.id_or_urn, COVER)
                                            }
                                            None => text(trunc_px(&sp.subtitle, COVER, 12.0))
                                                .size(12)
                                                .wrapping(text::Wrapping::None)
                                                .style(|_| muted())
                                                .into(),
                                        })
                                        .width(Length::Fixed(COVER))
                                        .height(Length::Fixed(SUB_H))
                                        .clip(true),
                                    ]
                                    .spacing(LINE_GAP),
                                ]
                                .spacing(COVER_GAP)
                                .width(Length::Fixed(COVER)),
                            )
                            .on_press(Message::OpenPlaylist(sp.id_or_urn.clone()))
                            .padding(CARD_PAD)
                            .width(Length::Fixed(CARD_W))
                            .height(Length::Fixed(CARD_H))
                            .clip(true)
                            .style(|_, status| button::Style {
                                background: match status {
                                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                                    _ => None,
                                },
                                border: Border {
                                    radius: border::Radius::from(8.0),
                                    width: 0.0,
                                    color: Color::TRANSPARENT,
                                },
                                shadow: Shadow::default(),
                                ..button::Style::default()
                            }),
                        );
                    }
                    let shelf_scroll = self.h_scrollable(cards).width(Length::Fill);
                    sections = sections.push(column![title_header, shelf_scroll].spacing(10));
                }
                HomeShelf::Tracks(tracks) => {
                    // offline, Home is the only list of every download
                    let list: Element<'_, Message> = if offline {
                        self.virtual_track_rows(ListKey::OfflineHome, tracks)
                    } else {
                        let mut list = column![].spacing(2);
                        for (i, t) in tracks.iter().take(20).enumerate() {
                            list = list.push(self.track_row(i, t));
                        }
                        list.into()
                    };
                    sections = sections.push(column![title_header, list].spacing(10));
                }
            }
        }

        let mut home_col = column![header].spacing(16);
        // stories are the follow feed's latest uploads: online only
        if let Some(stories_widget) = self.view_stories_widget().filter(|_| !offline) {
            home_col = home_col.push(stories_widget);
        }
        home_col = home_col.push(sections);
        home_col.padding(pad4(8.0, 24.0, 28.0, 24.0)).into()
    }

    fn view_search(&self) -> Element<'_, Message> {
        // Card geometry: symmetric padding on every side, artwork filling the
        // card's inner width exactly, and a fixed height equal to the content
        // (pad + art + gap + one 1.3x text line + pad) so every card in a shelf
        // is identical no matter how long its label is.
        const PAD: f32 = 8.0;
        const GAP: f32 = 6.0;
        // People: 106 wide -> 90px round avatar, 14px name.
        const PERSON_W: f32 = 106.0;
        const PERSON_ART: f32 = PERSON_W - PAD * 2.0;
        const PERSON_H: f32 = PAD * 2.0 + PERSON_ART + GAP + 14.0 * 1.3;
        // Playlists: 188 wide -> 172px cover, 16px title, 12px owner line.
        const CARD_W: f32 = 188.0;
        const CARD_ART: f32 = CARD_W - PAD * 2.0;
        const OWNER_H: f32 = 12.0 * 1.3;
        const CARD_H: f32 = PAD * 2.0 + CARD_ART + GAP + 16.0 * 1.3 + 2.0 + OWNER_H;

        let mut col = column![text("Search")
            .size(28)
            .wrapping(text::Wrapping::None)
            .style(|_| bright())]
        .spacing(12)
        .padding(Padding {
            top: 8.0,
            right: 24.0,
            bottom: 24.0,
            left: 24.0,
        });

        if let Some(err) = self.page_error_view() {
            return col.push(err).into();
        }
        if self.search_loading {
            return col.push(self.loading_view()).into();
        }
        if self.search.tracks.is_empty()
            && self.search.users.is_empty()
            && self.search.playlists.is_empty()
        {
            if !self.search_query.trim().is_empty() {
                col = col.push(text("Nothing found").size(16).style(|_| dim()));
            }
        }
        if !self.search.tracks.is_empty() {
            col = col.push(self.section_header("Tracks"));
            col = col.push(self.virtual_track_rows(ListKey::Search, &self.search.tracks));
            if self.search_next.is_some() {
                col = col.push(
                    button(
                        text("Load more")
                            .size(16)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    )
                    .on_press(Message::SearchMore)
                    .padding(Padding::from([8, 20]))
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(match status {
                            button::Status::Hovered => BG_HOVER,
                            _ => BG_ELEV,
                        })),
                        border: round(20.0),
                        text_color: TEXT,
                        ..button::Style::default()
                    }),
                );
            }
        }
        if !self.search.users.is_empty() {
            col = col.push(self.section_header("People"));
            let mut urel = row![].spacing(14).padding(pad4(4.0, 12.0, 8.0, 0.0));
            for u in self.search.users.iter().take(20) {
                urel = urel.push(
                    button(
                        column![
                            self.artwork_tile(
                                u.avatar_url.as_deref(),
                                u.id,
                                clean_username(&u.username),
                                PERSON_ART,
                                true,
                            ),
                            // 90px at 14px -> 11 chars; clipped column is the backstop.
                            text(trunc(clean_username(&u.username), 11))
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| dim()),
                        ]
                        .spacing(GAP)
                        .align_x(iced::Alignment::Center)
                        .width(Length::Fixed(PERSON_ART))
                        .clip(true),
                    )
                    .on_press(Message::OpenProfile(u.id))
                    .padding(pad4(PAD, PAD, PAD, PAD))
                    .width(Length::Fixed(PERSON_W))
                    .height(Length::Fixed(PERSON_H))
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                            _ => None,
                        },
                        border: Border {
                            radius: border::Radius::from(8.0),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: Shadow::default(),
                        ..button::Style::default()
                    }),
                );
            }
            let urel_scroll = self.h_scrollable(urel).width(Length::Fill);
            col = col.push(urel_scroll);
        }
        if !self.search.playlists.is_empty() {
            col = col.push(self.section_header("Playlists"));
            let mut prels = row![].spacing(14).padding(pad4(4.0, 12.0, 8.0, 0.0));
            for pl in self.search.playlists.iter().take(20) {
                prels = prels.push(
                    button(
                        column![
                            self.artwork_tile(
                                pl.artwork_or_avatar(),
                                pl.id,
                                &pl.title,
                                CARD_ART,
                                false,
                            ),
                            column![
                                // cut to the cover's width; the column clips as a backstop
                                text(trunc_px(&pl.title, CARD_ART, 16.0))
                                    .size(16)
                                    .wrapping(text::Wrapping::None)
                                    .style(|_| bright()),
                                // the owner, a link to their profile
                                container(match pl.user.as_ref().filter(|u| u.id != 0) {
                                    Some(owner) =>
                                        self.owner_link(owner, &pl.id_or_urn(), CARD_ART),
                                    None => horizontal_space().into(),
                                })
                                .height(Length::Fixed(OWNER_H)),
                            ]
                            .spacing(2),
                        ]
                        .spacing(GAP)
                        .width(Length::Fixed(CARD_ART))
                        .clip(true),
                    )
                    .on_press(Message::OpenPlaylist(pl.id_or_urn()))
                    .padding(pad4(PAD, PAD, PAD, PAD))
                    .width(Length::Fixed(CARD_W))
                    .height(Length::Fixed(CARD_H))
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                            _ => None,
                        },
                        border: Border {
                            radius: border::Radius::from(8.0),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: Shadow::default(),
                        ..button::Style::default()
                    }),
                );
            }
            let prels_scroll = self.h_scrollable(prels).width(Length::Fill);
            col = col.push(prels_scroll);
        }
        if self.search.tracks.is_empty() && self.search.users.is_empty() {
            col = col.push(container(text("No results").size(16).style(|_| dim())).padding(16));
        }
        col.into()
    }

    fn view_library(&self) -> Element<'_, Message> {
        // Liked Tracks holds tracks only; liked playlists live in the
        // sidebar's library list.
        let mut col = column![
            // Page title stays on one line; clipped instead of wrapping on a
            // narrow window so the content below never jumps.
            container(
                text("Liked Tracks")
                    .size(34)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright()),
            )
            .width(Length::Fill)
            .clip(true),
            text(format!("{} tracks", self.library.len()))
                .size(16)
                .wrapping(text::Wrapping::None)
                .style(|_| muted()),
            self.collection_actions(Collection::Liked),
        ]
        .spacing(12)
        .padding(Padding {
            top: 8.0,
            right: 24.0,
            bottom: 24.0,
            left: 24.0,
        });

        if self.library.is_empty() && self.library_loading {
            col = col.push(self.loading_view());
        } else if self.library.is_empty() {
            col = col.push(
                text("Tracks you like will appear here")
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(|_| dim()),
            );
        }
        col = col.push(self.virtual_track_rows(ListKey::Library, &self.library));

        col.into()
    }

    fn view_playlist(&self) -> Element<'_, Message> {
        // Spotify's entity page is full-bleed: the tinted header runs edge to
        // edge and only the track list is inset.
        //
        // Every header label is single-line: the title and author are cut to
        // the room they really have, and the Download pill keeps one fixed
        // size in every state, so nothing reflows or nudges its neighbours
        // when data or state changes.

        const COVER: f32 = 192.0;

        let mut col = column![].spacing(0);
        if let Some(pl) = self.current_playlist.as_ref() {
            let station_art = station_avatar(pl);
            let cover_url = station_art.as_deref().or_else(|| {
                pl.artwork_url
                    .as_deref()
                    .or(pl.author_avatar.as_deref())
                    .or_else(|| pl.tracks.first().and_then(|t| t.artwork_or_avatar()))
            });
            let tint = cover_url
                .and_then(|u| self.art_colors.get(u).copied())
                .unwrap_or(Color::from_rgb(0.32, 0.32, 0.34));
            let total_ms: u64 = pl.tracks.iter().filter_map(|t| t.duration).sum();

            // Room the header's text column really gets: the window minus the
            // sidebar and island gutter (268), any open side panel, the
            // header's own padding (72) and the cover plus its gap (216).
            // Labels are cut to that (≈0.56em per glyph, one slot kept for
            // the ellipsis) so they never wrap; the column also clips, as a
            // backstop for a window narrower than the one it opened at.
            let text_w = (self.content_w() - 72.0 - (COVER + 24.0)).max(160.0);
            // shrink the title before cutting it; cut only if 24px still overflows
            let title_px = fit_title_px(&pl.title, text_w);
            let title = trunc_px(&pl.title, text_w, f32::from(title_px));
            let meta = format!(
                " • {} tracks{}",
                pl.tracks.len(),
                if total_ms > 0 {
                    format!(", {}", fmt_duration_long(total_ms))
                } else {
                    String::new()
                }
            );
            // the author's avatar (24) and name open their profile, as on Spotify
            const AVATAR: f32 = 24.0;
            let author_room = (text_w - text_px(&meta, 14.0) - (AVATAR + 8.0)).max(96.0);
            let author = trunc_px(&pl.author, author_room, 14.0);
            let author_line: Element<'_, Message> = match pl.author_id {
                Some(uid) => row![
                    button(self.artwork_tile(
                        pl.author_avatar.as_deref(),
                        uid,
                        &pl.author,
                        AVATAR,
                        true
                    ))
                    .on_press(Message::OpenProfile(uid))
                    .padding(0)
                    .style(|_, _| button::Style {
                        background: None,
                        ..button::Style::default()
                    }),
                    hover_link(
                        author,
                        14,
                        TEXT,
                        TEXT,
                        self.pb_hover == Some(PbLink::PlaylistAuthor),
                        Message::OpenProfile(uid),
                        Message::PlayerBarHover(Some(PbLink::PlaylistAuthor)),
                        Message::PlayerBarHover(None),
                    ),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
                None => text(author)
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright())
                    .into(),
            };

            let hero = container(
                column![
                    row![
                        self.art_button(cover_url, pl.id, &pl.title, COVER, station_art.is_some()),
                        column![
                            text(if pl.is_album { "Album" } else { "Playlist" })
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright()),
                            text(title)
                                .size(title_px)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright()),
                            row![
                                author_line,
                                text(meta)
                                    .size(14)
                                    .wrapping(text::Wrapping::None)
                                    .style(|_| dim()),
                            ]
                            .align_y(iced::Alignment::Center),
                        ]
                        .spacing(12)
                        .width(Length::Fill)
                        .clip(true),
                    ]
                    .spacing(24)
                    .height(Length::Fixed(COVER))
                    .align_y(iced::Alignment::End),
                    // Action bar sits on the same tinted band, like Spotify.
                    self.collection_actions(Collection::Playlist(pl.id)),
                ]
                .spacing(28),
            )
            .padding(pad4(44.0, 36.0, 24.0, 36.0))
            .width(Length::Fill)
            .style(move |_| container::Style {
                border: hero_border(),
                background: Some(hero_gradient(tint)),
                ..container::Style::default()
            });

            col = col.push(hero);
            if self.loading_playlist.is_some() {
                // the preview's list may be partial: wait for the full one
                col = col.push(self.loading_view());
            } else if let Some(err) = self.page_error_view() {
                col = col.push(err);
            } else {
                col = col.push(
                    container(self.virtual_track_rows(ListKey::Playlist, &pl.tracks))
                        .padding(pad4(8.0, 28.0, 32.0, 28.0)),
                );
            }
        } else if self.loading_playlist.is_some() {
            col = col.push(self.loading_view());
        } else if let Some(err) = self.page_error_view() {
            col = col.push(err);
        } else {
            col = col
                .push(container(text("No playlist loaded").size(16).style(|_| dim())).padding(24));
        }
        col.into()
    }

    /// Artist profile view: header, clickable followers/following, sub-tabs (Overview, Tracks, Playlists, Likes, Followers, Following).
    fn view_profile(&self) -> Element<'_, Message> {
        // Full-bleed tinted header, like Spotify's artist page.
        let mut root = column![].spacing(16);

        let Some(profile) = self.profile.as_ref() else {
            if self.loading_profile.is_some() {
                return root.push(self.loading_view()).into();
            }
            if let Some(err) = self.page_error_view() {
                return root.push(err).into();
            }
            return root.into();
        };

        // Playlist shelf card (same geometry as the Home shelves): 188px wide
        // with the same 8px padding on every side, so the 172px cover fills the
        // inner width exactly. The fixed height is exactly the content:
        // 8 + 172 cover + 8 + 21 title line (16px * 1.3) + 8 = 217.
        const CARD_W: f32 = 188.0;
        const CARD_PAD: f32 = 8.0;
        const COVER: f32 = CARD_W - 2.0 * CARD_PAD;
        const COVER_GAP: f32 = 8.0;
        const TITLE_H: f32 = 21.0;
        const CARD_H: f32 = CARD_PAD + COVER + COVER_GAP + TITLE_H + CARD_PAD;
        // Related-artist tile: round avatar filling the inner width, one name
        // line. 6 + 92 avatar + 6 + 19 name line (14px * 1.3) + 6 = 129.
        const ARTIST_PAD: f32 = 6.0;
        const AVATAR: f32 = 92.0;
        const ARTIST_W: f32 = AVATAR + 2.0 * ARTIST_PAD;
        const NAME_GAP: f32 = 6.0;
        const NAME_H: f32 = 19.0;
        const ARTIST_H: f32 = ARTIST_PAD + AVATAR + NAME_GAP + NAME_H + ARTIST_PAD;
        let active_tab = profile.active_tab;

        // Clickable followers / following / tracks / playlists badges; the one
        // matching the open sub-tab turns orange. Counts are fixed per profile
        // load, so the row never changes width while the page is open.
        let stat_badge = |label: String, tab: Option<ProfileSubTab>| -> Element<'static, Message> {
            let active = tab == Some(active_tab);
            let badge = button(
                text(label)
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| if active { orange_t() } else { dim() }),
            )
            .padding(Padding::from([2, 6]))
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(4.0),
                ..button::Style::default()
            });
            match tab {
                Some(t) => badge.on_press(Message::ProfileSubTabSelected(t)).into(),
                None => badge.into(),
            }
        };

        // Hide zero stats entirely (no "0 followers" noise).
        let mut badges: Vec<Element<'static, Message>> = Vec::new();
        if profile.followers > 0 {
            badges.push(stat_badge(
                format!("{} followers", profile.followers),
                Some(ProfileSubTab::Followers),
            ));
        }
        if profile.followings > 0 {
            badges.push(stat_badge(
                format!("{} following", profile.followings),
                Some(ProfileSubTab::Following),
            ));
        }
        if profile.track_count > 0 {
            badges.push(stat_badge(
                format!("{} tracks", profile.track_count),
                Some(ProfileSubTab::Tracks),
            ));
        }
        if profile.likes_count > 0 {
            badges.push(stat_badge(
                format!("{} likes", fmt_count(profile.likes_count)),
                Some(ProfileSubTab::Likes),
            ));
        }
        if profile.playlist_count > 0 {
            badges.push(stat_badge(
                format!("{} playlists", fmt_count(profile.playlist_count)),
                Some(ProfileSubTab::Playlists),
            ));
        }
        let has_badges = !badges.is_empty();
        let mut stats_row = row![].spacing(6).align_y(iced::Alignment::Center);
        for (i, b) in badges.into_iter().enumerate() {
            if i > 0 {
                stats_row = stats_row.push(
                    text("•")
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                );
            }
            stats_row = stats_row.push(b);
        }

        // External links as small chips: 14px glyph + 12px label, 24px tall.
        let mut links_row = row![].spacing(6).align_y(iced::Alignment::Center);
        for wp in &profile.web_profiles {
            let (glyph, font) = web_profile_icon(wp.service.as_deref(), &wp.url);
            let label: &str = if !wp.title.is_empty() {
                wp.title.as_str()
            } else {
                wp.service.as_deref().unwrap_or("Link")
            };
            let url = wp.url.clone();
            links_row = links_row.push(
                button(
                    row![
                        text(glyph)
                            .font(font)
                            .size(14)
                            .wrapping(text::Wrapping::None)
                            .style(|_| dim()),
                        text(trunc(label, 18))
                            .size(12)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::OpenExternalLink(url))
                .padding(Padding::from([3, 10]))
                .clip(true)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => Some(Background::Color(BG_ELEV)),
                    },
                    border: round(13.0),
                    ..button::Style::default()
                }),
            );
        }

        let tint = profile
            .avatar_url
            .as_deref()
            .and_then(|u| self.art_colors.get(u).copied())
            .unwrap_or(Color::from_rgb(0.32, 0.32, 0.34));

        let profile_hero_card: Element<'_, Message> = if profile.banner_url.is_none() {
            // No banner of their own: Spotify's header, the avatar's tint
            // fading into the page, the big round avatar and the name on it.
            // Every header line is single-line; the column clips, so a long
            // name or stats/links row is cut at the band's edge.
            let hero_name = clean_username(&profile.username).to_string();
            let hero_text_w = (self.content_w() - 72.0 - (192.0 + 24.0)).max(160.0);
            let hero_name_px = fit_title_px(&hero_name, hero_text_w);
            let mut hero_meta =
                column![
                    text(trunc_px(&hero_name, hero_text_w, f32::from(hero_name_px)))
                        .size(hero_name_px)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                ]
                .spacing(12)
                .width(Length::Fill)
                .clip(true);
            if has_badges {
                hero_meta = hero_meta.push(stats_row);
            }
            if !profile.web_profiles.is_empty() {
                hero_meta = hero_meta.push(links_row);
            }
            container(
                column![
                    row![
                        self.art_button(
                            profile.avatar_url.as_deref(),
                            profile.id,
                            clean_username(&profile.username),
                            192.0,
                            true,
                        ),
                        hero_meta,
                    ]
                    .spacing(24)
                    .align_y(iced::Alignment::End),
                    // Spotify keeps the entity actions on the tinted band.
                    self.collection_actions(Collection::Profile(profile.id)),
                ]
                .spacing(28),
            )
            .padding(pad4(44.0, 36.0, 24.0, 36.0))
            .width(Length::Fill)
            .clip(true)
            .style(move |_| container::Style {
                border: hero_border(),
                background: Some(hero_gradient(tint)),
                ..container::Style::default()
            })
            .into()
        } else {
            // SoundCloud's profile header: the banner the artist set (or a
            // gradient in their avatar's colours), the round avatar on it, and
            // the name, real name and place as white on black labels.
            const BANNER_H: f32 = 260.0;
            const BANNER_PAD: f32 = 30.0;
            const AVATAR_PX: f32 = 200.0;
            let label_w =
                (self.content_w() - AVATAR_PX - 2.0 * BANNER_PAD - 28.0 - 16.0).max(120.0);
            let label = |content: String, size: u16, bold: bool| -> Element<'static, Message> {
                let t = text(trunc_px(&content, label_w, f32::from(size)))
                    .size(size)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright());
                container(if bold { t.font(UI_BOLD) } else { t })
                    .padding(pad4(4.0, 8.0, 4.0, 8.0))
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.85))),
                        ..container::Style::default()
                    })
                    .into()
            };
            let mut labels = column![label(
                clean_username(&profile.username).to_string(),
                24,
                true
            )]
            .spacing(8)
            .align_x(iced::Alignment::Start);
            if !profile.full_name.is_empty() {
                labels = labels.push(label(profile.full_name.clone(), 16, false));
            }
            if !profile.location.is_empty() {
                labels = labels.push(label(profile.location.clone(), 14, false));
            }
            let front = container(
                row![
                    self.art_button(
                        profile.avatar_url.as_deref(),
                        profile.id,
                        clean_username(&profile.username),
                        AVATAR_PX,
                        true,
                    ),
                    labels,
                ]
                .spacing(28)
                .align_y(iced::Alignment::Start),
            )
            .padding(BANNER_PAD)
            .width(Length::Fill)
            .height(Length::Fixed(BANNER_H))
            .clip(true);
            let backdrop: Element<'_, Message> = match &self.profile_banner {
                Some((uid, handle)) if *uid == profile.id => stack![
                    image(handle.clone())
                        .content_fit(iced::ContentFit::Cover)
                        .width(Length::Fill)
                        .height(Length::Fixed(BANNER_H)),
                    // the page's rounded top corners, over the photo
                    canvas(TopCorners {
                        radius: 8.0,
                        color: BG,
                    })
                    .width(Length::Fill)
                    .height(Length::Fixed(BANNER_H)),
                ]
                .into(),
                _ => {
                    let (from, to) = banner_colors(tint);
                    container(iced::widget::Space::new(
                        Length::Fill,
                        Length::Fixed(BANNER_H),
                    ))
                    .width(Length::Fill)
                    .style(move |_| container::Style {
                        border: hero_border(),
                        background: Some(Background::Gradient(iced::Gradient::Linear(
                            iced::gradient::Linear::new(2.2)
                                .add_stop(0.0, from)
                                .add_stop(1.0, to),
                        ))),
                        ..container::Style::default()
                    })
                    .into()
                }
            };
            let mut under =
                column![self.collection_actions(Collection::Profile(profile.id))].spacing(16);
            if has_badges {
                under = under.push(stats_row);
            }
            if !profile.web_profiles.is_empty() {
                under = under.push(links_row);
            }
            column![
                stack![backdrop, front],
                container(under)
                    .padding(pad4(20.0, 32.0, 0.0, 32.0))
                    .width(Length::Fill)
                    .clip(true),
            ]
            .into()
        };

        root = root.push(profile_hero_card);
        // Everything below the header keeps Spotify's page inset.
        let mut body = column![].spacing(16).padding(pad4(20.0, 32.0, 32.0, 32.0));

        // Sub-tabs navigation bar: 16px label (20.8 line) + 2 * 6 = 32.8 tall pills.
        let subtab_button = |label: &'static str, tab: ProfileSubTab| {
            let active = active_tab == tab;
            button(
                text(label)
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| if active { orange_t() } else { dim() }),
            )
            .on_press(Message::ProfileSubTabSelected(tab))
            .padding(Padding::from([6, 14]))
            .style(move |_, status| button::Style {
                background: if active {
                    Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.12)))
                } else {
                    match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    }
                },
                border: round(17.0),
                ..button::Style::default()
            })
        };

        // Hide tabs whose content is known to be empty (counts come with the
        // profile JSON, so lazy-loaded tabs are still decided correctly).
        let mut tab_bar = row![subtab_button("Overview", ProfileSubTab::Overview)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .clip(true);
        if profile.track_count > 0 {
            tab_bar = tab_bar.push(subtab_button("Tracks", ProfileSubTab::Tracks));
        }
        if profile.playlist_count > 0 || !profile.playlists.is_empty() {
            tab_bar = tab_bar.push(subtab_button("Playlists", ProfileSubTab::Playlists));
        }
        if profile.likes_count > 0 {
            tab_bar = tab_bar.push(subtab_button("Likes", ProfileSubTab::Likes));
        }
        if profile.followers > 0 {
            tab_bar = tab_bar.push(subtab_button("Followers", ProfileSubTab::Followers));
        }
        if profile.followings > 0 {
            tab_bar = tab_bar.push(subtab_button("Following", ProfileSubTab::Following));
        }

        body = body.push(tab_bar);

        // Playlist card, shared by the Overview shelf and the Playlists tab.
        let playlist_card = |pl: &Playlist| -> Element<'static, Message> {
            button(
                column![
                    self.artwork_tile(pl.artwork_or_avatar(), pl.id, &pl.title, COVER, false),
                    container(
                        text(trunc(&pl.title, 18))
                            .size(16)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    )
                    .width(Length::Fixed(COVER))
                    .height(Length::Fixed(TITLE_H))
                    .clip(true),
                ]
                .spacing(COVER_GAP)
                .width(Length::Fixed(COVER)),
            )
            .on_press(Message::OpenPlaylist(pl.id_or_urn()))
            .padding(CARD_PAD)
            .width(Length::Fixed(CARD_W))
            .height(Length::Fixed(CARD_H))
            .clip(true)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(8.0),
                ..button::Style::default()
            })
            .into()
        };

        // Related-artist tile: name centred under the round avatar.
        let artist_tile = |a: &UserMini| -> Element<'static, Message> {
            button(
                column![
                    self.artwork_tile(
                        a.avatar_url.as_deref(),
                        a.id,
                        clean_username(&a.username),
                        AVATAR,
                        true
                    ),
                    container(
                        text(trunc(clean_username(&a.username), 11))
                            .size(14)
                            .wrapping(text::Wrapping::None)
                            .style(|_| dim()),
                    )
                    .center_x(Length::Fixed(AVATAR))
                    .height(Length::Fixed(NAME_H))
                    .clip(true),
                ]
                .spacing(NAME_GAP)
                .align_x(iced::Alignment::Center)
                .width(Length::Fixed(AVATAR)),
            )
            .on_press(Message::OpenProfile(a.id))
            .padding(ARTIST_PAD)
            .width(Length::Fixed(ARTIST_W))
            .height(Length::Fixed(ARTIST_H))
            .clip(true)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => None,
                },
                border: round(8.0),
                ..button::Style::default()
            })
            .into()
        };

        match profile.active_tab {
            ProfileSubTab::Overview => {
                if !profile.top_tracks.is_empty() {
                    body = body.push(self.section_header("Top tracks"));
                    let mut list = column![].spacing(2);
                    for (i, t) in profile.top_tracks.iter().enumerate() {
                        list = list.push(self.track_row(i, t));
                    }
                    body = body.push(list);
                }

                if !profile.playlists.is_empty() {
                    body = body.push(self.section_header("Playlists"));
                    let mut prels = row![].spacing(14).padding(pad4(4.0, 12.0, 8.0, 0.0));
                    for pl in profile.playlists.iter().take(20) {
                        prels = prels.push(playlist_card(pl));
                    }
                    let prels_scroll = self.h_scrollable(prels).width(Length::Fill);
                    body = body.push(prels_scroll);
                }

                if !profile.reposts.is_empty() {
                    body = body.push(self.section_header("Reposts"));
                    let mut list = column![].spacing(2);
                    for (i, t) in profile.reposts.iter().take(20).enumerate() {
                        list = list.push(self.track_row(i, t));
                    }
                    body = body.push(list);
                }

                if !profile.related.is_empty() {
                    body = body.push(self.section_header("Related artists"));
                    let mut rel = row![].spacing(16).padding(pad4(4.0, 12.0, 8.0, 0.0));
                    for a in profile.related.iter().take(20) {
                        rel = rel.push(artist_tile(a));
                    }
                    let rel_scroll = self.h_scrollable(rel).width(Length::Fill);
                    body = body.push(rel_scroll);
                }
            }
            ProfileSubTab::Tracks => {
                let tracks = if !profile.all_tracks.is_empty() {
                    &profile.all_tracks
                } else {
                    &profile.top_tracks
                };
                if !tracks.is_empty() {
                    body = body.push(self.virtual_track_rows(ListKey::ProfileTracks, tracks));
                } else {
                    body = body.push(if self.profile_tab_loading() {
                        self.loading_view()
                    } else {
                        text("No tracks available").size(16).style(|_| dim()).into()
                    });
                }
            }
            ProfileSubTab::Playlists => {
                if !profile.playlists.is_empty() {
                    let mut prels = row![].spacing(14).padding(pad4(4.0, 12.0, 8.0, 0.0));
                    for pl in &profile.playlists {
                        prels = prels.push(playlist_card(pl));
                    }
                    let prels_scroll = self.h_scrollable(prels).width(Length::Fill);
                    body = body.push(prels_scroll);
                } else {
                    body = body.push(text("No playlists available").size(16).style(|_| dim()));
                }
            }
            ProfileSubTab::Likes => {
                if !profile.likes.is_empty() {
                    body =
                        body.push(self.virtual_track_rows(ListKey::ProfileLikes, &profile.likes));
                } else {
                    body = body.push(if self.profile_tab_loading() {
                        self.loading_view()
                    } else {
                        text("No liked tracks found")
                            .size(16)
                            .style(|_| dim())
                            .into()
                    });
                }
            }
            ProfileSubTab::Followers => {
                if !profile.followers_list.is_empty() {
                    let mut ucol = column![].spacing(6);
                    for u in &profile.followers_list {
                        ucol = ucol.push(self.user_card(u, 24));
                    }
                    body = body.push(ucol);
                } else {
                    body = body.push(if self.profile_tab_loading() {
                        self.loading_view()
                    } else {
                        text("No followers found").size(16).style(|_| dim()).into()
                    });
                }
            }
            ProfileSubTab::Following => {
                if !profile.followings_list.is_empty() {
                    let mut ucol = column![].spacing(6);
                    for u in &profile.followings_list {
                        ucol = ucol.push(self.user_card(u, 24));
                    }
                    body = body.push(ucol);
                } else {
                    body = body.push(if self.profile_tab_loading() {
                        self.loading_view()
                    } else {
                        text("No following users found")
                            .size(16)
                            .style(|_| dim())
                            .into()
                    });
                }
            }
        }

        root.push(body).into()
    }

    fn view_track_page(&self) -> Element<'_, Message> {
        let Some(page) = self.track_page.as_ref() else {
            return container(
                column![text("No track selected").size(18).style(|_| dim()),]
                    .align_x(iced::alignment::Horizontal::Center)
                    .spacing(12),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        };

        // Every label on this page is one line. Heights below are exact
        // content heights (text line = 1.3 x font size), and every label
        // whose text depends on state (Play/Pause, Like/Liked, tab counters,
        // playback time) sits in a fixed-width slot, so nothing reflows or
        // nudges its neighbours when data or state changes.
        const HERO_ART: f32 = 192.0;
        // "Play All Related" pill: Spotify's --encore-control-size-smaller,
        // 14px glyph in a fixed 18px slot.
        const PILL_H: f32 = 32.0;
        const PILL_ICON: u16 = 14;
        const PILL_ICON_SLOT: f32 = 18.0;
        // Sub-tab pills: sized for "<label> (9999)" so a count arriving or
        // growing never moves the tabs after it.
        const TAB_H: f32 = 32.0;
        const TAB_RELATED_W: f32 = 184.0;
        const TAB_COMMENTS_W: f32 = 160.0;
        const TAB_LIKERS_W: f32 = 124.0;
        const TAB_REPOSTERS_W: f32 = 152.0;
        // Header meta: 12px badge (15.6 line), 24px avatar line, 14px chips.
        const BADGE_H: f32 = 22.0;
        const META_H: f32 = 24.0;
        const CHIP_H: f32 = 26.0;
        // Comment composer (16px text: 20.8 line + 2 x 7.6) and comment rows.
        const INPUT_H: f32 = 36.0;
        const COMMENT_AVATAR: f32 = 32.0;
        const TS_H: f32 = 24.0;
        // "123:45/123:45" at 12px.
        const TIME_W: f32 = 88.0;
        const SECTION_H: f32 = 32.0;

        let tr = &page.track;
        let tid = tr.id;
        let is_playing = self.playing_id == Some(tid);
        let is_playing_active = is_playing && !self.is_paused;
        let dur = tr.full_duration.or(tr.duration).unwrap_or(0);

        // Room the header's text column really gets: the window minus the
        // sidebar and island gutter (268), any open side panel, the header's
        // own padding (72) and the artwork plus its gap. Labels are cut to
        // that (~0.56em per glyph, one slot kept for the ellipsis); the column
        // also clips, as a backstop for a window narrower than it opened at.
        let text_w = (self.content_w() - 72.0 - (HERO_ART + 24.0)).max(160.0);

        // High resolution artwork. The frame follows the tile's own corner
        // radius so the border and shadow hug the image exactly.
        let art_url = tr
            .artwork_or_avatar()
            .map(|s| s.replace("-large.jpg", "-t500x500.jpg"));
        let art_r = art_radius(art_px(HERO_ART));
        let artwork_element =
            container(self.art_button(art_url.as_deref(), tid, &tr.title, HERO_ART, false))
                .width(Length::Fixed(HERO_ART))
                .height(Length::Fixed(HERO_ART))
                .style(move |_| container::Style {
                    border: Border {
                        radius: border::Radius::from(art_r),
                        width: 1.0,
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                        offset: iced::Vector::new(0.0, 6.0),
                        blur_radius: 16.0,
                    },
                    ..container::Style::default()
                });

        let (artist_name, display_title) =
            tr.display_artist_and_title(self.settings.prefer_artist_from_name);
        let artist_id = tr.user.as_ref().map(|u| u.id);
        let artist_avatar = tr.user.as_ref().and_then(|u| u.avatar_url.as_deref());

        // The artist shown opens their profile: the uploader's, or one looked
        // up by name when the name isn't theirs (see artist_link_msg); the
        // avatar is the uploader's, so it shows only in the first case.
        let artist_msg = artist_link_msg(tr, &artist_name);
        let uploader_shown = matches!(artist_msg, Message::OpenProfile(_));
        // The rest of the meta line ("• 2024-01-01 • 3:45") needs ~190px.
        let artist_label = trunc_px(
            clean_username(&artist_name),
            (text_w - 190.0).max(96.0),
            14.0,
        );

        // The label carries no colour of its own, so it inherits the button's
        // text colour and the orange hover actually shows.
        let mut artist_row = row![].spacing(8).align_y(iced::Alignment::Center);
        if uploader_shown {
            artist_row = artist_row.push(self.artwork_tile(
                artist_avatar,
                artist_id.unwrap_or(0),
                &artist_name,
                META_H,
                true,
            ));
        }
        let artist_link: Element<Message> = button(
            artist_row.push(
                text(artist_label.clone())
                    .size(14)
                    .wrapping(text::Wrapping::None),
            ),
        )
        .on_press(artist_msg)
        .padding(0)
        .style(|_, status| button::Style {
            background: None,
            text_color: match status {
                button::Status::Hovered => ORANGE,
                _ => TEXT,
            },
            ..button::Style::default()
        })
        .into();

        // Tags, as soundcloud.com shows them: "# Hip-Hop & Rap" pills (the
        // genre first), each a search for that tag; "TRACK" when it has none.
        let tags = track_tags(tr);
        let cat_badge: Element<'_, Message> = if tags.is_empty() {
            container(
                text("TRACK")
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(|_| orange_t()),
            )
            .padding(pad4(0.0, 8.0, 0.0, 8.0))
            .center_y(Length::Fixed(BADGE_H))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.12))),
                border: round(4.0),
                ..container::Style::default()
            })
            .into()
        } else {
            let mut pills = row![].spacing(6).height(Length::Fixed(BADGE_H)).clip(true);
            for tag in tags {
                pills = pills.push(
                    button(
                        container(
                            text(format!("# {}", trunc(&tag, 32)))
                                .size(12)
                                .wrapping(text::Wrapping::None),
                        )
                        .padding(pad4(0.0, 10.0, 0.0, 10.0))
                        .center_y(Length::Fixed(BADGE_H)),
                    )
                    .on_press(Message::SearchTag(tag))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(match status {
                            button::Status::Hovered => BG_TINT_HI,
                            _ => BG_TINT,
                        })),
                        text_color: match status {
                            button::Status::Hovered => TEXT,
                            _ => TEXT_DIM,
                        },
                        border: round(BADGE_H / 2.0),
                        ..button::Style::default()
                    }),
                );
            }
            container(pills)
                .width(Length::Fixed(text_w))
                .clip(true)
                .into()
        };

        // Release date string (if available)
        let date_str = tr
            .created_at
            .as_deref()
            .map(|s| s.split('T').next().unwrap_or(s));

        let mut meta_row = row![artist_link]
            .spacing(8)
            .height(Length::Fixed(META_H))
            .align_y(iced::Alignment::Center);
        if let Some(ds) = date_str {
            meta_row = meta_row.push(
                text("•")
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            );
            meta_row = meta_row.push(
                text(ds)
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            );
        }
        if dur > 0 {
            meta_row = meta_row.push(
                text("•")
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            );
            meta_row = meta_row.push(
                text(fmt_time(dur))
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            );
        }
        // the speed this track keeps (see remember_speed)
        if let Some(&speed) = self.track_speeds.get(&tid) {
            meta_row = meta_row.push(speed_badge(speed_badge_label(speed)));
        }

        // Stats metrics chips: one-line pills of one fixed height. The row
        // wraps whole chips onto a second line in a narrow window instead of
        // hiding them or breaking a label.
        // Likes, reposts and comments open their tab below; plays is just a count.
        let chip = |glyph: &'static str,
                    glyph_color: Color,
                    label: String,
                    tab: Option<TrackSubTab>|
         -> Element<'static, Message> {
            let body = container(
                row![
                    text(glyph)
                        .font(FA_SOLID)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| t_color(glyph_color)),
                    text(label)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| dim()),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding(pad4(0.0, 10.0, 0.0, 10.0))
            .center_y(Length::Fixed(CHIP_H));
            match tab {
                Some(tab) => button(body)
                    .on_press(Message::TrackPageSubTabSelected(tab))
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(match status {
                            button::Status::Hovered | button::Status::Pressed => BG_HOVER,
                            _ => BG_CARD,
                        })),
                        border: round(CHIP_H / 2.0),
                        ..button::Style::default()
                    })
                    .into(),
                None => body.style(|_| panel(BG_CARD, CHIP_H / 2.0)).into(),
            }
        };
        let mut stats_chips = row![].spacing(8).align_y(iced::Alignment::Center);
        if let Some(pc) = tr.playback_count.filter(|n| *n > 0) {
            stats_chips = stats_chips.push(chip(
                icons::PLAY,
                TEXT_MUTED,
                format!("{} plays", fmt_count(pc)),
                None,
            ));
        }
        if let Some(lc) = tr.likes_count.filter(|n| *n > 0) {
            stats_chips = stats_chips.push(chip(
                icons::HEART,
                HEART,
                format!("{} likes", fmt_count(lc)),
                Some(TrackSubTab::Likers),
            ));
        }
        if let Some(rc) = tr.reposts_count.filter(|n| *n > 0) {
            stats_chips = stats_chips.push(chip(
                icons::REPOST,
                ORANGE,
                format!("{} reposts", fmt_count(rc)),
                Some(TrackSubTab::Reposters),
            ));
        }
        if let Some(cc) = tr.comment_count.filter(|n| *n > 0) {
            stats_chips = stats_chips.push(chip(
                icons::COMMENTS,
                TEXT_MUTED,
                format!("{} comments", fmt_count(cc)),
                Some(TrackSubTab::Comments),
            ));
        }

        let title_px = fit_title_px(&display_title, text_w);
        let hero_meta = column![
            cat_badge,
            text(trunc_px(&display_title, text_w, f32::from(title_px)))
                .size(title_px)
                .wrapping(text::Wrapping::None)
                .style(|_| bright()),
            meta_row,
            stats_chips.wrap(),
        ]
        .spacing(10)
        .width(Length::Fill)
        .clip(true);

        let hero_banner = row![artwork_element, hero_meta,]
            .spacing(24)
            .align_y(iced::Alignment::End);

        // Play / Download / "..."; everything else is in the "..." panel.
        let dl = if self.offline_single.contains(&tid) {
            DlState::Busy
        } else if self.downloaded_track_ids.contains(&tid) {
            DlState::Done
        } else {
            DlState::Idle
        };
        let actions_bar = entity_actions(
            Message::TrackPagePlayToggle,
            is_playing_active,
            download_button(
                dl,
                if self.offline_single.contains(&tid) {
                    Message::CancelOfflineTrack(tid)
                } else {
                    Message::DownloadTrackOffline(tr.clone())
                },
            ),
            Message::OpenActionMenu(ActionMenu::Track(tr.clone()), MenuAnchor::Page),
            |b| self.with_menu(b, MenuAnchor::Page),
        );

        // Optional description card
        let description_card: Option<Element<'_, Message>> = tr
            .description
            .as_ref()
            .filter(|d| !d.trim().is_empty())
            .map(|desc| {
                let expanded = self.expanded_description_track == Some(tid);
                let full = desc.trim();
                let body = if expanded {
                    full.to_string()
                } else {
                    trunc(full, 400)
                };
                let mut details = column![
                    text("About this track")
                        .size(16)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                    text(body).size(14).style(|_| dim()),
                ]
                .spacing(6);
                if full.chars().count() > 400 {
                    details = details.push(
                        text(if expanded { "Show less" } else { "Show more" })
                            .size(12)
                            .wrapping(text::Wrapping::None)
                            .style(|_| orange_t()),
                    );
                }
                iced::widget::mouse_area(
                    container(details)
                        .padding(14)
                        .width(Length::Fill)
                        .style(|_| panel(BG_CARD, 8.0)),
                )
                .on_press(Message::ToggleTrackDescription(tid))
                .interaction(mouse::Interaction::Pointer)
                .into()
            });

        // Waveform card if currently playing this track!
        // The ticking position sits in a fixed right-aligned slot.
        let waveform_card: Option<Element<'_, Message>> = if is_playing {
            Some(
                container(
                    column![
                        row![
                            text("Now Playing Timeline")
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| orange_t()),
                            horizontal_space(),
                            container(
                                text(format!(
                                    "{}/{}",
                                    fmt_time(self.pos_ms),
                                    fmt_time(self.dur_ms)
                                ))
                                .size(12)
                                .wrapping(text::Wrapping::None)
                                .style(|_| muted()),
                            )
                            .width(Length::Fixed(TIME_W))
                            .align_x(iced::alignment::Horizontal::Right)
                            .clip(true),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center)
                        .clip(true),
                        self.view_waveform(),
                    ]
                    .spacing(8),
                )
                .padding(14)
                .width(Length::Fill)
                .style(|_| panel(BG_CARD, 8.0))
                .into(),
            )
        } else {
            None
        };

        // Sub-tabs bar:
        let rel_count = page.related_tracks.len();
        let com_count = page.comments.len();
        let lik_count = page.likers.len();
        let rep_count = page.reposters.len();

        let subtab_button = |label: &'static str,
                             count: usize,
                             tab: TrackSubTab,
                             active: bool,
                             width: f32|
         -> Element<'static, Message> {
            let label_text = if count > 0 {
                format!("{} ({})", label, count)
            } else {
                label.to_string()
            };
            button(
                container(
                    text(label_text)
                        .size(16)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| if active { orange_t() } else { dim() }),
                )
                .center_x(Length::Fixed(width))
                .center_y(Length::Fixed(TAB_H))
                .clip(true),
            )
            .on_press(Message::TrackPageSubTabSelected(tab))
            .padding(0)
            .style(move |_, status| button::Style {
                background: if active {
                    Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.12)))
                } else {
                    match status {
                        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                        _ => None,
                    }
                },
                border: round(TAB_H / 2.0),
                ..button::Style::default()
            })
            .into()
        };

        let subtabs_bar = row![
            subtab_button(
                "Related Tracks",
                rel_count,
                TrackSubTab::Related,
                page.active_tab == TrackSubTab::Related,
                TAB_RELATED_W
            ),
            subtab_button(
                "Comments",
                com_count,
                TrackSubTab::Comments,
                page.active_tab == TrackSubTab::Comments,
                TAB_COMMENTS_W
            ),
            subtab_button(
                "Likers",
                lik_count,
                TrackSubTab::Likers,
                page.active_tab == TrackSubTab::Likers,
                TAB_LIKERS_W
            ),
            subtab_button(
                "Reposters",
                rep_count,
                TrackSubTab::Reposters,
                page.active_tab == TrackSubTab::Reposters,
                TAB_REPOSTERS_W
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .wrap();

        // Sub-tab content:
        let subtab_content: Element<'_, Message> = match page.active_tab {
            TrackSubTab::Related => {
                if page.related_tracks.is_empty() {
                    container(if page.pending.contains(&TrackSubTab::Related) {
                        self.loading_view()
                    } else {
                        text("No related tracks found")
                            .size(16)
                            .style(|_| muted())
                            .into()
                    })
                    .padding(24)
                    .center_x(Length::Fill)
                    .into()
                } else {
                    let mut col = column![].spacing(8);
                    let play_all_rel = button(
                        container(
                            row![
                                container(
                                    text(icons::PLAY)
                                        .font(FA_SOLID)
                                        .size(PILL_ICON)
                                        .wrapping(text::Wrapping::None)
                                        .style(|_| t_color(Color::BLACK)),
                                )
                                .center_x(Length::Fixed(PILL_ICON_SLOT)),
                                text("Play All Related")
                                    .size(14)
                                    .wrapping(text::Wrapping::None)
                                    .style(|_| t_color(Color::BLACK)),
                            ]
                            .spacing(6)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding(pad4(0.0, 14.0, 0.0, 14.0))
                        .center_y(Length::Fixed(PILL_H))
                        .clip(true),
                    )
                    .on_press(Message::TrackPagePlayAllRelated)
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: Some(Background::Color(match status {
                            button::Status::Hovered => Color::from_rgb(1.0, 0.45, 0.1),
                            _ => ORANGE,
                        })),
                        text_color: Color::BLACK,
                        border: round(PILL_H / 2.0),
                        ..button::Style::default()
                    });

                    // The heading gets whatever the button leaves and is cut
                    // at one line, so a narrow window can't make the row taller.
                    col = col.push(
                        row![
                            container(self.section_header("Recommended based on this track"))
                                .width(Length::Fill)
                                .center_y(Length::Fixed(SECTION_H))
                                .clip(true),
                            play_all_rel,
                        ]
                        .spacing(12)
                        .height(Length::Fixed(SECTION_H))
                        .align_y(iced::Alignment::Center),
                    );

                    let mut list = column![].spacing(2);
                    for (i, t) in page.related_tracks.iter().enumerate() {
                        list = list.push(self.track_row(i, t));
                    }
                    col = col.push(list);
                    col.into()
                }
            }
            TrackSubTab::Comments => {
                let mut col = column![].spacing(10);
                // Comment input row: field and button share one exact height.
                let input_pad_y = (INPUT_H - 16.0 * 1.3) / 2.0;
                let react_buttons =
                    QUICK_REACTIONS
                        .iter()
                        .fold(row![].spacing(6), |r, (codepoint, _)| {
                            let glyph: Element<'_, Message> = match reaction_image(codepoint) {
                                Some(handle) => image(handle)
                                    .width(Length::Fixed(20.0))
                                    .height(Length::Fixed(20.0))
                                    .into(),
                                None => text(
                                    WaveReaction {
                                        second: 0,
                                        codepoint: (*codepoint).to_string(),
                                    }
                                    .emoji(),
                                )
                                .size(18)
                                .font(iced::Font::with_name("Segoe UI Emoji"))
                                .into(),
                            };
                            r.push(
                                button(
                                    container(glyph)
                                        .center_x(Length::Fixed(36.0))
                                        .center_y(Length::Fixed(INPUT_H)),
                                )
                                .on_press(Message::TrackPageReact((*codepoint).to_string()))
                                .padding(0)
                                .style(|_, status| button::Style {
                                    background: match status {
                                        button::Status::Hovered => {
                                            Some(Background::Color(BG_HOVER))
                                        }
                                        _ => Some(Background::Color(BG_ELEV)),
                                    },
                                    border: round(8.0),
                                    ..button::Style::default()
                                }),
                            )
                        });

                let input_row = row![
                    text_input("Leave a comment…", &page.comment_input)
                        .on_input(Message::TrackPageCommentInput)
                        .on_submit(Message::TrackPagePostComment)
                        .padding(pad4(input_pad_y, 12.0, input_pad_y, 12.0))
                        .size(16)
                        .width(Length::Fill)
                        .style(|_, _| text_input::Style {
                            background: Background::Color(BG_CARD),
                            border: Border {
                                radius: border::Radius::from(8.0),
                                width: 1.0,
                                color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                            },
                            icon: Color::TRANSPARENT,
                            placeholder: TEXT_MUTED,
                            value: TEXT,
                            selection: ORANGE,
                        }),
                    button(
                        container(
                            text("Post")
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright())
                        )
                        .padding(pad4(0.0, 16.0, 0.0, 16.0))
                        .center_y(Length::Fixed(INPUT_H))
                    )
                    .on_press(Message::TrackPagePostComment)
                    .padding(0)
                    .style(|_, status| button::Style {
                        background: match status {
                            button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                            _ => Some(Background::Color(BG_ELEV)),
                        },
                        border: round(8.0),
                        ..button::Style::default()
                    }),
                    react_buttons,
                ]
                .spacing(8)
                .height(Length::Fixed(INPUT_H))
                .align_y(iced::Alignment::Center);

                col = col.push(input_row);

                if page.comments.is_empty() {
                    col = col.push(
                        container(if page.pending.contains(&TrackSubTab::Comments) {
                            self.loading_view()
                        } else {
                            text("No comments yet. Be the first to comment!")
                                .size(16)
                                .style(|_| muted())
                                .into()
                        })
                        .padding(24)
                        .center_x(Length::Fill),
                    );
                } else {
                    let mut com_list = column![].spacing(6);
                    for c in &page.comments {
                        let author = c.author().to_string();
                        let author_id = c.user.as_ref().map(|u| u.id);
                        let ts = c.timestamp.unwrap_or(0).max(0) as u64;

                        // Unstyled label inherits the button's colour, so the
                        // orange hover shows.
                        let author_btn: Element<Message> = if let Some(uid) = author_id {
                            button(
                                text(trunc(&author, 22))
                                    .size(14)
                                    .wrapping(text::Wrapping::None),
                            )
                            .on_press(Message::OpenProfile(uid))
                            .padding(0)
                            .style(|_, status| button::Style {
                                background: None,
                                text_color: match status {
                                    button::Status::Hovered => ORANGE,
                                    _ => TEXT,
                                },
                                ..button::Style::default()
                            })
                            .into()
                        } else {
                            text(trunc(&author, 22))
                                .size(14)
                                .wrapping(text::Wrapping::None)
                                .style(|_| bright())
                                .into()
                        };

                        let ts_btn = button(
                            container(
                                row![
                                    text(icons::PLAY)
                                        .font(FA_SOLID)
                                        .size(14)
                                        .wrapping(text::Wrapping::None)
                                        .style(|_| orange_t()),
                                    text(fmt_time(ts))
                                        .size(14)
                                        .wrapping(text::Wrapping::None)
                                        .style(|_| orange_t()),
                                ]
                                .spacing(4)
                                .align_y(iced::Alignment::Center),
                            )
                            .padding(pad4(0.0, 8.0, 0.0, 8.0))
                            .center_y(Length::Fixed(TS_H)),
                        )
                        .on_press(Message::InspectorSeek(tid, ts))
                        .padding(0)
                        .style(|_, status| button::Style {
                            background: match status {
                                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                                _ => {
                                    Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.10)))
                                }
                            },
                            border: round(4.0),
                            ..button::Style::default()
                        });

                        let avatar_tile = self.artwork_tile(
                            c.user.as_ref().and_then(|u| u.avatar_url.as_deref()),
                            c.user.as_ref().map(|u| u.id).unwrap_or(0),
                            c.author(),
                            COMMENT_AVATAR,
                            true,
                        );

                        let avatar: Element<Message> = if let Some(uid) = author_id {
                            button(avatar_tile)
                                .on_press(Message::OpenProfile(uid))
                                .padding(0)
                                .style(|_, _| button::Style {
                                    background: None,
                                    ..button::Style::default()
                                })
                                .into()
                        } else {
                            avatar_tile
                        };

                        // Header line is exactly the avatar's height; the
                        // author takes the free width and is clipped there.
                        com_list = com_list.push(
                            container(
                                column![
                                    row![
                                        avatar,
                                        container(author_btn).width(Length::Fill).clip(true),
                                        ts_btn,
                                    ]
                                    .spacing(8)
                                    .height(Length::Fixed(COMMENT_AVATAR))
                                    .align_y(iced::Alignment::Center),
                                    text(&c.body).size(14).style(|_| dim()),
                                ]
                                .spacing(6),
                            )
                            .padding(10)
                            .width(Length::Fill)
                            .style(|_| panel(BG_CARD, 8.0)),
                        );
                    }
                    col = col.push(com_list);
                }
                col.into()
            }
            TrackSubTab::Likers => {
                if page.likers.is_empty() {
                    container(if page.pending.contains(&TrackSubTab::Likers) {
                        self.loading_view()
                    } else {
                        text("No likes recorded yet")
                            .size(16)
                            .style(|_| muted())
                            .into()
                    })
                    .padding(24)
                    .center_x(Length::Fill)
                    .into()
                } else {
                    let mut col = column![].spacing(6);
                    for u in &page.likers {
                        col = col.push(self.user_card(u, 24));
                    }
                    col.into()
                }
            }
            TrackSubTab::Reposters => {
                if page.reposters.is_empty() {
                    container(if page.pending.contains(&TrackSubTab::Reposters) {
                        self.loading_view()
                    } else {
                        text("No reposts recorded yet")
                            .size(16)
                            .style(|_| muted())
                            .into()
                    })
                    .padding(24)
                    .center_x(Length::Fill)
                    .into()
                } else {
                    let mut col = column![].spacing(6);
                    for u in &page.reposters {
                        col = col.push(self.user_card(u, 24));
                    }
                    col.into()
                }
            }
        };

        let head_tint = art_url
            .as_deref()
            .and_then(|u| self.art_colors.get(u).copied())
            .unwrap_or(Color::from_rgb(0.32, 0.32, 0.34));
        let head = container(column![hero_banner, actions_bar].spacing(28))
            .padding(pad4(44.0, 36.0, 24.0, 36.0))
            .width(Length::Fill)
            .style(move |_| container::Style {
                border: hero_border(),
                background: Some(hero_gradient(head_tint)),
                ..container::Style::default()
            });

        let main_col = column![head].spacing(0);
        // Everything under the header keeps Spotify's page inset.
        let mut body = column![].spacing(18).padding(pad4(20.0, 32.0, 32.0, 32.0));

        if let Some(desc) = description_card {
            body = body.push(desc);
        }

        if let Some(wf) = waveform_card {
            body = body.push(wf);
        }

        body = body.push(subtabs_bar);
        body = body.push(subtab_content);

        self.v_scrollable(main_col.push(body)).into()
    }

    fn view_profile_chip(&self) -> Element<'_, Message> {
        // Same 32px height as every other title-bar control, pill radius = half.
        // Logged-in chip: 2px ring around the 28px avatar, the name capped at
        // NAME_W (14px text: 96 / (14 * 0.56) ~ 12 glyphs -> 11 + ellipsis) and
        // clipped, the caret in a fixed 12px box. Max width is therefore
        // 2 + 28 + 6 + 96 + 6 + 12 + 10 = 160px, whatever the username.
        const NAME_W: f32 = 96.0;
        const CARET_W: f32 = 12.0;
        let arrow = if self.show_user_menu {
            icons::CARET_UP
        } else {
            icons::CARET_DOWN
        };
        if let Some(me) = &self.state.me {
            let name = me.username.trim_start_matches('@').to_string();
            button(
                row![
                    self.artwork_tile(me.avatar_url.as_deref(), me.id, &name, 28.0, true),
                    container(
                        text(trunc(&name, 11))
                            .size(14)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    )
                    .max_width(NAME_W)
                    .clip(true),
                    container(text(arrow).font(FA_SOLID).size(14).style(|_| muted()))
                        .center_x(Length::Fixed(CARET_W)),
                ]
                .spacing(6)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleUserMenu)
            .height(Length::Fixed(TITLEBAR_CTRL))
            .padding(pad4(0.0, 10.0, 0.0, 2.0))
            .clip(true)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                    _ => Some(Background::Color(BG_CARD)),
                },
                border: round(TITLEBAR_CTRL / 2.0),
                text_color: TEXT,
                ..button::Style::default()
            })
            .into()
        } else {
            button(
                row![
                    text("Log in")
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| bright()),
                    container(text(arrow).font(FA_SOLID).size(14).style(|_| bright()))
                        .center_x(Length::Fixed(CARET_W)),
                ]
                .spacing(6)
                .height(Length::Fill)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleUserMenu)
            .height(Length::Fixed(TITLEBAR_CTRL))
            .padding(pad4(0.0, 12.0, 0.0, 14.0))
            .clip(true)
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => {
                        Some(Background::Color(Color::from_rgb(1.0, 0.45, 0.1)))
                    }
                    _ => Some(Background::Color(ORANGE)),
                },
                border: round(TITLEBAR_CTRL / 2.0),
                text_color: Color::WHITE,
                ..button::Style::default()
            })
            .into()
        }
    }

    fn view_user_menu_dropdown(&self) -> Element<'_, Message> {
        // 180px card - 2 x 6 padding = 168px items. Each item is a fixed 36px
        // row (16px text line = 20.8px + ~8px above/below). Icons sit in a
        // fixed 20px box so the labels line up whatever the glyph width
        // (user 14px, gear / logout 16px).
        const ITEM_H: f32 = 36.0;
        const ICON_W: f32 = 20.0;
        let menu_item =
            |glyph: &'static str, label: &'static str, msg: Message| -> Element<'_, Message> {
                button(
                    row![
                        container(text(glyph).font(FA_SOLID).size(16).style(|_| muted()))
                            .center_x(Length::Fixed(ICON_W)),
                        text(label)
                            .size(16)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    ]
                    .spacing(10)
                    .height(Length::Fill)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(msg)
                .padding(pad4(0.0, 12.0, 0.0, 12.0))
                .width(Length::Fill)
                .height(Length::Fixed(ITEM_H))
                .clip(true)
                .style(|_, status| button::Style {
                    background: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(Background::Color(BG_HOVER))
                        }
                        _ => None,
                    },
                    border: round(4.0),
                    text_color: TEXT,
                    ..button::Style::default()
                })
                .into()
            };

        let mut items = column![
            menu_item(icons::USER, "Account", Message::UserMenuAccount),
            menu_item(icons::GEAR, "Settings", Message::UserMenuSettings),
        ]
        .spacing(2);

        if self.state.authenticated {
            // Borders are transparent app-wide, so the separator uses the tinted
            // surface colour to actually show; 3px breathing room each side.
            let divider = container(
                container(horizontal_space())
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .style(|_| container::Style {
                        background: Some(Background::Color(BG_TINT)),
                        ..container::Style::default()
                    }),
            )
            .width(Length::Fill)
            .padding(pad4(3.0, 4.0, 3.0, 4.0));
            items = items.push(divider);
            items = items.push(menu_item(icons::LOGOUT, "Log out", Message::Logout));
        }

        container(items)
            .width(Length::Fixed(180.0))
            .padding(6)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_CARD)),
                border: Border {
                    radius: border::Radius::from(8.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 20.0,
                },
                ..container::Style::default()
            })
            .into()
    }

    /// Full-resolution photo overlay, laid out like Spotify's: the image
    /// centred on a dark scrim with a plain "Close" control underneath.
    fn view_image_viewer<'a>(&'a self, v: &'a ImageViewer) -> Element<'a, Message> {
        let body: Element<'a, Message> = match &v.handle {
            Some(h) => image(h.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            // Still downloading the original — keep the scrim empty rather
            // than flashing a placeholder tile.
            None => container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        };

        let close_btn = button(text("Close").size(16).style(|_| bright()))
            .on_press(Message::CloseImageViewer)
            .padding(Padding::from([8, 20]))
            .style(|_, status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(BG_TINT)),
                    _ => None,
                },
                border: round(20.0),
                ..button::Style::default()
            });

        let backdrop = button(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::CloseImageViewer)
        .padding(0)
        .style(|_, _| button::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.82))),
            ..button::Style::default()
        });

        let content = column![
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            container(close_btn).center_x(Length::Fill),
        ]
        .spacing(20)
        .align_x(iced::Alignment::Center)
        // bottom inset keeps "Close" clear of the player bar underneath
        .padding(pad4(64.0, 28.0, 108.0, 28.0));

        stack![backdrop, content].into()
    }

    /// An open story stays inside the app window: the page behind it is softly
    /// dimmed, with the story in the middle drawn as SoundCloud draws one (who
    /// posted it and when, the
    /// artwork with the title and artist on it, likes / reposts / add /
    /// "..."). Arrows beside it step through the stories, X in the corner or
    /// a click on the dimmed part closes. Nothing plays until its Play.
    fn view_story_fullscreen(&self, idx: usize) -> Element<'_, Message> {
        let Some(story) = self.stories.get(idx) else {
            return container(horizontal_space()).into();
        };
        let total = self.stories.len();
        // the bars count this artist's stories
        let group = self.story_group_range(idx);

        const PAD: f32 = 16.0;
        const GAP: f32 = 14.0;
        const HEADER_H: f32 = 44.0;
        const BAR_H: f32 = 40.0;
        const NAV: f32 = 48.0;
        // The artwork is as big as the screen allows: the card's other rows
        // (progress, header, bar, gaps, padding) and a margin fit around it
        // vertically, the arrows beside it horizontally.
        let (win_w, win_h) = (self.window_size.width, self.window_size.height);
        let art = (win_h - 3.0 - HEADER_H - BAR_H - 3.0 * GAP - 2.0 * PAD - 64.0)
            .min(win_w - 2.0 * (NAV + 24.0) - 2.0 * PAD - 64.0)
            .clamp(220.0, 640.0);

        // 1. which of this artist's stories: seen ones and this one white
        let seg_gap = 4.0;
        let mut segments = row![]
            .spacing(seg_gap)
            .width(Length::Fill)
            .height(Length::Fixed(3.0))
            .clip(true);
        for i in group {
            let c = if i <= idx {
                Color::WHITE
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.25)
            };
            segments = segments.push(
                container(horizontal_space())
                    .width(Length::Fill)
                    .height(Length::Fixed(3.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(c)),
                        border: round(1.5),
                        ..container::Style::default()
                    }),
            );
        }

        // 2. who posted it, and when
        let hovered = |l: PbLink| self.pb_hover == Some(l);
        let enter = |l: PbLink| Message::PlayerBarHover(Some(l));
        let posted = if story.reposted {
            "reposted a track"
        } else {
            "posted a track"
        };
        let when = match time_ago(story.created_at_ms) {
            Some(ago) => format!("{posted} • {ago}"),
            None => posted.to_string(),
        };
        let open_poster = Message::CloseStoryThen(Box::new(Message::OpenProfile(story.user_id)));
        let text_w = art - HEADER_H - 12.0;
        let header = row![
            button(self.artwork_tile(
                story.avatar_url.as_deref(),
                story.user_id,
                clean_username(&story.username),
                HEADER_H,
                true,
            ))
            .on_press(open_poster.clone())
            .padding(0)
            .style(|_, _| button::Style::default()),
            column![
                hover_link(
                    trunc_px(clean_username(&story.username), text_w, 16.0),
                    16,
                    TEXT,
                    TEXT,
                    hovered(PbLink::StoryUser),
                    open_poster,
                    enter(PbLink::StoryUser),
                    Message::PlayerBarHover(None),
                ),
                text(trunc_px(&when, text_w, 13.0))
                    .size(13)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted()),
            ]
            .spacing(2)
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(12)
        .height(Length::Fixed(HEADER_H))
        .align_y(iced::Alignment::Center);

        // 3. the artwork, title and artist on black labels at its lower left.
        // Audio starts as soon as the story opens; there is no per-story toggle.
        let artwork = self.artwork_tile(
            story.artwork_url.as_deref().or(story.avatar_url.as_deref()),
            story.track_id,
            &story.track_title,
            art,
            false,
        );
        let art_tile: Element<'_, Message> = artwork.into();
        let on_black = |content: Element<'static, Message>| -> Element<'static, Message> {
            container(content)
                .padding(Padding::from([3, 8]))
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::BLACK)),
                    ..container::Style::default()
                })
                .into()
        };
        let label_w = art - 2.0 * 12.0 - 16.0;
        let title_link = hover_link(
            trunc_px(&story.track_title, label_w, 18.0),
            18,
            TEXT,
            TEXT,
            hovered(PbLink::StoryTitle),
            Message::CloseStoryThen(Box::new(Message::OpenTrackPage(Box::new(
                story.track.clone(),
            )))),
            enter(PbLink::StoryTitle),
            Message::PlayerBarHover(None),
        );
        let artist: Element<'_, Message> = match story.track.user.as_ref() {
            Some(u) => hover_link(
                trunc_px(clean_username(&u.username), label_w, 16.0),
                16,
                TEXT_MUTED,
                TEXT,
                hovered(PbLink::StoryArtist),
                Message::CloseStoryThen(Box::new(Message::OpenProfile(u.id))),
                enter(PbLink::StoryArtist),
                Message::PlayerBarHover(None),
            ),
            None => text(trunc_px(clean_username(&story.username), label_w, 16.0))
                .size(16)
                .wrapping(text::Wrapping::None)
                .style(|_| muted())
                .into(),
        };
        let art_block = stack![
            art_tile,
            container(column![on_black(title_link), on_black(artist)].spacing(4))
                .width(Length::Fixed(art))
                .height(Length::Fixed(art))
                .align_y(iced::alignment::Vertical::Bottom)
                .padding(12),
        ];

        // 4. likes, reposts, add to playlist ... and "..."
        let liked = self.liked_ids.contains(&story.track_id);
        let reposted = self.reposted_ids.contains(&story.track_id);
        let pill_style = |_: &iced::Theme, status: button::Status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => BG_HOVER,
                _ => BG_CARD,
            })),
            text_color: TEXT,
            border: round(6.0),
            ..button::Style::default()
        };
        let pill = |glyph: &'static str,
                    font: iced::Font,
                    color: Color,
                    count: u64,
                    msg: Message|
         -> Element<'static, Message> {
            let mut content = row![text(glyph)
                .font(font)
                .size(15)
                .wrapping(text::Wrapping::None)
                .style(move |_| t_color(color))]
            .spacing(8)
            .align_y(iced::Alignment::Center);
            if count > 0 {
                content = content.push(
                    text(fmt_count(count))
                        .size(14)
                        .font(UI_BOLD)
                        .wrapping(text::Wrapping::None),
                );
            }
            button(
                container(content)
                    .center_y(Length::Fixed(BAR_H))
                    .padding(Padding::from([0, 14])),
            )
            .on_press(msg)
            .padding(0)
            .style(pill_style)
            .into()
        };
        let icon_btn = |glyph: &'static str, msg: Message| {
            button(
                container(
                    text(glyph)
                        .font(FA_SOLID)
                        .size(15)
                        .wrapping(text::Wrapping::None),
                )
                .center_x(Length::Fixed(BAR_H))
                .center_y(Length::Fixed(BAR_H)),
            )
            .on_press(msg)
            .padding(0)
            .style(pill_style)
        };
        let bar = row![
            pill(
                icons::HEART,
                if liked { FA_SOLID } else { FA_REGULAR },
                if liked { HEART } else { TEXT },
                story.track.likes_count.unwrap_or(0),
                Message::LikeTrack(story.track_id),
            ),
            pill(
                icons::REPOST,
                FA_SOLID,
                if reposted { ORANGE } else { TEXT },
                story.track.reposts_count.unwrap_or(0),
                Message::TrackRepostToggle(story.track_id),
            ),
            self.with_menu(
                icon_btn(
                    icons::FOLDER_PLUS,
                    Message::OpenAddPopover(story.track.clone(), MenuAnchor::Story)
                ),
                MenuAnchor::Story,
            ),
            horizontal_space(),
            self.with_menu(
                icon_btn(
                    icons::ELLIPSIS,
                    Message::OpenActionMenu(
                        ActionMenu::Track(story.track.clone()),
                        MenuAnchor::StoryMore
                    ),
                ),
                MenuAnchor::StoryMore,
            ),
        ]
        .spacing(8)
        .height(Length::Fixed(BAR_H))
        .align_y(iced::Alignment::Center);

        // The card takes its own clicks: only the dimmed part closes.
        let card = iced::widget::mouse_area(
            container(
                column![segments, header, art_block, bar]
                    .spacing(GAP)
                    .width(Length::Fixed(art)),
            )
            .padding(PAD)
            .style(|_| container::Style {
                background: Some(Background::Color(BG_MUTED)),
                border: Border {
                    radius: border::Radius::from(12.0),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                    offset: Vector::new(0.0, 12.0),
                    blur_radius: 40.0,
                },
                ..container::Style::default()
            }),
        )
        .on_press(Message::Noop);

        let round_btn =
            |glyph: &'static str, size: u16, msg: Message| -> Element<'static, Message> {
                button(
                    container(
                        text(glyph)
                            .font(FA_SOLID)
                            .size(size)
                            .wrapping(text::Wrapping::None)
                            .style(|_| bright()),
                    )
                    .center_x(Length::Fixed(NAV))
                    .center_y(Length::Fixed(NAV)),
                )
                .on_press(msg)
                .padding(0)
                .style(|_, status| button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.22),
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                    })),
                    border: round(NAV / 2.0),
                    ..button::Style::default()
                })
                .into()
            };
        let nav = |glyph: &'static str, shown: bool, msg: Message| -> Element<'static, Message> {
            if shown {
                round_btn(glyph, 18, msg)
            } else {
                iced::widget::Space::new(Length::Fixed(NAV), Length::Fixed(NAV)).into()
            }
        };
        let middle = row![
            nav(icons::CHEVRON_LEFT, idx > 0, Message::PrevStory),
            card,
            nav(icons::CHEVRON_RIGHT, idx + 1 < total, Message::NextStory),
        ]
        .spacing(24)
        .align_y(iced::Alignment::Center);

        let backdrop = iced::widget::mouse_area(
            container(horizontal_space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.02, 0.02, 0.02, 0.76))),
                    ..container::Style::default()
                }),
        )
        .on_press(Message::CloseStory)
        .on_scroll(|_| Message::Noop);

        stack![
            backdrop,
            container(middle)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            container(round_btn(icons::XMARK, 20, Message::CloseStory))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .padding(20),
        ]
        .into()
    }

    fn track_row(&self, i: usize, t: &Track) -> Element<'_, Message> {
        // Fixed geometry so no row ever changes size or shifts its columns:
        // index | 44px cover | title+artist (fills, clipped) | duration | actions.
        // Height = cover (44) + 6px padding top and bottom = 56. Text column is
        // 16*1.3 + 2 + 12*1.3 = 38.4px, so it sits inside the cover height.
        const ART: f32 = 44.0;
        const PAD_Y: f32 = 6.0;
        const PAD_X: f32 = 8.0;
        const ROW_H: f32 = ART + PAD_Y * 2.0;
        const INDEX_W: f32 = 32.0;
        const INDEX_H: f32 = 20.0;
        const TIME_W: f32 = 48.0;
        // Square hit box for row icon buttons so glyphs of different widths line up.
        const ACT: f32 = 32.0;
        // Title column width: content island minus page padding (~80) and the
        // fixed columns (index, cover, duration, actions, gaps ~300), with a
        // little slack for the "downloaded" check. Labels are cut to it in
        // pixels; the column still clips as a backstop.
        let title_w = (self.content_w() - 432.0).max(120.0);

        let playing = self.playing_id == Some(t.id);
        let (artist_name, display_title) =
            t.display_artist_and_title(self.settings.prefer_artist_from_name);
        // the blue badge of a track with its own saved speed, after the title
        let speed_label = self.track_speeds.get(&t.id).map(|&s| speed_badge_label(s));
        let badge_room = speed_label.as_deref().map_or(0.0, speed_badge_room);
        // offline, a track that isn't on disk can't play: shown greyed
        let unplayable = self.settings.offline_mode && !self.downloaded_track_ids.contains(&t.id);
        let tr_clone = t.clone();

        // The artist is a link to the uploader's profile, underlined and
        // brighter under the pointer (like the player bar's); the title is
        // plain text.
        let artist_msg = artist_link_msg(t, &artist_name);
        let artist_hovered = self.row_artist_hover == Some((t.id, i));
        // on this computer (downloaded, or kept by Cache tracks locally): the
        // green arrow before the artist, as Spotify marks downloads
        let on_disk = self.downloaded_track_ids.contains(&t.id)
            && (self.settings.cache_tracks || self.is_downloaded(t.id));
        let artist_btn = iced::widget::mouse_area(link_text(
            trunc_px(
                clean_username(&artist_name),
                title_w - if on_disk { 17.0 } else { 0.0 },
                12.0,
            ),
            12,
            TEXT_MUTED,
            TEXT,
            artist_hovered,
            Some(artist_msg.clone()),
        ))
        .on_press(artist_msg)
        .on_enter(Message::RowArtistHover(Some((t.id, i))))
        .on_exit(Message::RowArtistHover(None))
        .interaction(mouse::Interaction::Pointer);

        // Keep the index visible even on the playing row; playback is shown
        // by the orange number and title rather than animated equalizer bars.
        let track_indicator: Element<'_, Message> = optical_center(
            container(
                text((i + 1).to_string())
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| if playing { orange_t() } else { muted() }),
            )
            .width(Length::Fixed(INDEX_W))
            .height(Length::Fixed(INDEX_H))
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true),
            14.0,
        )
        .into();

        // Row action buttons: same square box, same 16px glyph, same hover.
        let act_style = |_: &iced::Theme, status: button::Status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            text_color: match status {
                button::Status::Hovered => ORANGE,
                _ => TEXT_MUTED,
            },
            border: round(4.0),
            ..button::Style::default()
        };
        // Now Playing's save button: outline "+", a filled check once the
        // track is saved anywhere (then it opens the playlist picker). Built
        // from a ring and a glyph, not Now Playing's canvas: a canvas in
        // every row of a long list would be re-shaped on every frame.
        let saved = self.is_saved(t.id);
        let glyph: Element<'_, Message> = if saved {
            text(icons::CHECK_CIRCLE)
                .font(FA_SOLID)
                .size(16)
                .wrapping(text::Wrapping::None)
                .style(|_| t_color(ORANGE))
                .into()
        } else {
            container(
                text(icons::PLUS)
                    .font(FA_SOLID)
                    .size(8)
                    .wrapping(text::Wrapping::None),
            )
            .center_x(Length::Fixed(15.0))
            .center_y(Length::Fixed(15.0))
            .style(|_| container::Style {
                text_color: Some(TEXT_MUTED),
                border: Border {
                    radius: border::Radius::from(7.5),
                    width: 1.5,
                    color: TEXT_MUTED,
                },
                ..container::Style::default()
            })
            .into()
        };
        let save_btn = iced::widget::mouse_area(
            button(
                container(glyph)
                    .center_x(Length::Fixed(ACT))
                    .center_y(Length::Fixed(ACT)),
            )
            .on_press(Message::SaveTrackClicked(
                t.clone(),
                MenuAnchor::RowSave(t.id, i),
            ))
            .padding(0)
            .style(act_style),
        )
        // right / middle click always opens the picker, as in the player bar
        .on_right_press(Message::OpenAddPopover(
            t.clone(),
            MenuAnchor::RowSave(t.id, i),
        ))
        .on_middle_press(Message::OpenAddPopover(
            t.clone(),
            MenuAnchor::RowSave(t.id, i),
        ));
        let more_btn = button(
            container(
                text(icons::ELLIPSIS)
                    .font(FA_SOLID)
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(|_| t_color(TEXT_MUTED)),
            )
            .center_x(Length::Fixed(ACT))
            .center_y(Length::Fixed(ACT)),
        )
        .on_press(Message::OpenActionMenu(
            ActionMenu::Track(tr_clone.clone()),
            MenuAnchor::Row(t.id, i),
        ))
        .padding(0)
        .style(act_style);

        let title_badge: Element<'_, Message> = match speed_label {
            Some(label) => container(speed_badge(label))
                .padding(Padding {
                    left: SPEED_BADGE_GAP,
                    ..Padding::ZERO
                })
                .into(),
            None => horizontal_space().width(Length::Fixed(0.0)).into(),
        };

        let content = row![
            track_indicator,
            button(self.artwork_tile(t.artwork_or_avatar(), t.id, &t.title, ART, false,))
                .on_press(Message::PlayTrack(t.id))
                .padding(0)
                .style(|_, _| button::Style {
                    background: None,
                    ..button::Style::default()
                }),
            // Title + artist take all remaining width and clip there, so a long
            // title is cut instead of pushing the duration/actions sideways.
            // Spotify parity: the green downloaded badge sits right before the
            // title, same line.
            optical_center(
                column![
                    container(
                        row![
                            button(
                                text(trunc_px(&display_title, title_w - badge_room, 16.0))
                                    .size(16)
                                    .wrapping(text::Wrapping::None),
                            )
                            .on_press(Message::OpenTrackPage(Box::new(tr_clone.clone())))
                            .padding(0)
                            .style(move |_, status| button::Style {
                                background: None,
                                text_color: if playing {
                                    ORANGE
                                } else if unplayable {
                                    TEXT_MUTED
                                } else if status == button::Status::Hovered {
                                    ORANGE
                                } else {
                                    TEXT
                                },
                                ..button::Style::default()
                            }),
                            title_badge,
                        ]
                        .spacing(0)
                        .align_y(iced::Alignment::Center),
                    )
                    .width(Length::Fill)
                    .clip(true),
                    if on_disk {
                        Element::from(
                            row![
                                text(icons::CIRCLE_DOWN)
                                    .font(FA_SOLID)
                                    .size(12)
                                    .wrapping(text::Wrapping::None)
                                    .style(|_| t_color(DOWNLOADED)),
                                artist_btn,
                            ]
                            .spacing(5)
                            .align_y(iced::Alignment::Center),
                        )
                    } else {
                        Element::from(artist_btn)
                    },
                ]
                .spacing(2)
                .width(Length::Fill)
                .clip(true),
                16.0
            )
            .width(Length::Fill),
            // Fixed-width, right-aligned duration: 3:45 and 1:02:03 line up and
            // the action buttons sit in the same column on every row.
            optical_center(
                container(
                    text(fmt_time(t.duration.unwrap_or(0)))
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                )
                .width(Length::Fixed(TIME_W))
                .align_x(iced::alignment::Horizontal::Right)
                .clip(true),
                14.0,
            ),
            // Spotify parity: rows expose just "+" and "...". Track Page and
            // everything else lives inside the "..." menu.
            row![
                self.with_menu(save_btn, MenuAnchor::RowSave(t.id, i)),
                self.with_menu(more_btn, MenuAnchor::Row(t.id, i)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding(pad4(PAD_Y, PAD_X, PAD_Y, PAD_X))
        .width(Length::Fill);

        let hovered = self.hovered_track_row == Some(t.id);
        iced::widget::mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fixed(ROW_H))
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_| container::Style {
                    background: hovered.then_some(Background::Color(BG_HOVER)),
                    ..container::Style::default()
                }),
        )
        .on_press(Message::PlayTrack(t.id))
        .on_right_press(Message::OpenActionMenu(
            ActionMenu::Track(tr_clone),
            MenuAnchor::Row(t.id, i),
        ))
        .on_enter(Message::TrackRowHover(Some(t.id)))
        .on_exit(Message::TrackRowHover(None))
        .interaction(mouse::Interaction::Pointer)
        .into()
    }

    fn view_player_bar(&self) -> Element<'_, Message> {
        if self.story_playback {
            return container(horizontal_space())
                .height(Length::Fixed(PB_H))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(BG_PLAYER)),
                    ..container::Style::default()
                })
                .into();
        }
        // Fixed geometry: every block, button and label in the bar has a fixed
        // box, so a long title, a volume glyph of a different width or a
        // speed label can never push a sibling around.
        // Left:  cover 40 + info 158 + 3 x 28 buttons + 4 x 6 spacing = 306
        // Right: 56 + 76 + 28 + 72 + 32 + 4 x 6 spacing               = 288
        const SIDE_W: f32 = PB_SIDE_W; // left and right blocks mirror each other
        const GAP: f32 = PB_GAP;
        const BTN: f32 = PB_BTN;
        const COVER: f32 = PB_COVER;
        const INFO_W: f32 = PB_INFO_W;
        const TITLE_H: f32 = 21.0; // 16px text, 1.3 line height
        const ARTIST_H: f32 = 16.0; // 12px text, 1.3 line height
        const SPEED_W: f32 = PB_SPEED_W;
        const QUEUE_W: f32 = 76.0; // list icon + "Queue" (~61px) + side padding
        const VOL_W: f32 = 72.0;
        const VOL_H: f32 = 24.0;
        const PCT_W: f32 = 32.0; // "100%"

        // a track with its own saved speed wears the blue badge after its title
        let speed_label = self
            .playing_id
            .and_then(|id| self.track_speeds.get(&id))
            .map(|&s| speed_badge_label(s));
        let badge_room = speed_label.as_deref().map_or(0.0, speed_badge_room);
        let title = if self.playing_title.is_empty() {
            "Nothing playing".to_string()
        } else {
            trunc_px(&self.playing_title, INFO_W - badge_room, 16.0)
        };
        let artist = if self.yt_ad {
            "Ad (muted) · YouTube Music".to_string()
        } else if self.playing_artist.is_empty() {
            "".to_string()
        } else {
            trunc_px(clean_username(&self.playing_artist), INFO_W, 12.0)
        };
        let cover_id = self.playing_id.unwrap_or(0);
        // looked up once: it walks (and clones out of) every loaded list
        let current_track = self.playing_id.and_then(|id| self.find_track_anywhere(id));

        // Title and artist are links, underlined while the pointer is on
        // them (Spotify's now-playing widget); the artist also brightens.
        let pb_link = |label: String,
                       size: u16,
                       color: Color,
                       hover_color: Color,
                       which: PbLink,
                       msg: Option<Message>|
         -> Element<'static, Message> {
            let hovered = msg.is_some() && self.pb_hover == Some(which);
            let label = link_text(label, size, color, hover_color, hovered, msg.clone());
            match msg {
                Some(msg) => iced::widget::mouse_area(label)
                    .on_press(msg)
                    .on_enter(Message::PlayerBarHover(Some(which)))
                    .on_exit(Message::PlayerBarHover(None))
                    .interaction(mouse::Interaction::Pointer)
                    .into(),
                None => label.into(),
            }
        };

        // Left: cover + title/artist + like
        let artist_msg = match current_track.as_ref() {
            _ if self.playing_artist.is_empty() => None,
            Some(track) => Some(artist_link_msg(track, &self.playing_artist)),
            None => Some(Message::ArtistClicked(self.playing_artist.clone())),
        };
        let artist_widget = pb_link(artist, 12, TEXT_MUTED, TEXT, PbLink::Artist, artist_msg);

        let track_cover = self.artwork_tile(
            current_track
                .as_ref()
                .and_then(|track| track.artwork_or_avatar()),
            cover_id,
            &title,
            COVER,
            false,
        );

        let track_cover_btn: Element<Message> = if let Some(tr) = current_track.as_ref() {
            button(track_cover)
                .on_press(Message::OpenTrackPage(Box::new(tr.clone())))
                .padding(0)
                .style(|_, _| button::Style {
                    background: None,
                    ..button::Style::default()
                })
                .into()
        } else {
            track_cover.into()
        };

        let shuffle_color = if self.shuffle { ORANGE } else { TEXT_MUTED };
        let shuffle_btn = button(
            container(
                text(icons::SHUFFLE)
                    .font(FA_SOLID)
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| text::Style {
                        color: Some(shuffle_color),
                    }),
            )
            .center_x(Length::Fixed(BTN))
            .center_y(Length::Fixed(BTN)),
        )
        .on_press(Message::ToggleShuffle)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            border: round(6.0),
            ..button::Style::default()
        });

        let repeat_color = if self.repeat != RepeatMode::Off {
            ORANGE
        } else {
            TEXT_MUTED
        };
        let repeat_content: Element<Message> = if self.repeat == RepeatMode::One {
            row![
                text(icons::REPEAT)
                    .font(FA_SOLID)
                    .size(14)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| text::Style {
                        color: Some(repeat_color)
                    }),
                text("1")
                    .size(11)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| text::Style {
                        color: Some(repeat_color)
                    }),
            ]
            .spacing(1)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            text(icons::REPEAT)
                .font(FA_SOLID)
                .size(16)
                .wrapping(text::Wrapping::None)
                .style(move |_| text::Style {
                    color: Some(repeat_color),
                })
                .into()
        };
        // Same 28x28 box for "repeat" and "repeat one", so toggling the mode
        // never nudges the save button.
        let repeat_btn = button(
            container(repeat_content)
                .center_x(Length::Fixed(BTN))
                .center_y(Length::Fixed(BTN)),
        )
        .on_press(Message::ToggleRepeat)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            border: round(6.0),
            ..button::Style::default()
        });

        // Spotify's save button: outline "+" until the track is saved
        // anywhere, then a filled check; see Message::SaveCurrentClicked.
        let saved = self.playing_id.is_some_and(|id| self.is_saved(id));
        let save_color = if saved {
            ORANGE
        } else if self.playing_id.is_some() {
            TEXT_MUTED
        } else {
            Color {
                a: 0.5,
                ..TEXT_MUTED
            }
        };
        let mut save_btn = button(
            container(
                canvas(SaveGlyph {
                    kind: if saved {
                        GlyphKind::Saved
                    } else {
                        GlyphKind::Add
                    },
                    color: save_color,
                })
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0)),
            )
            .center_x(Length::Fixed(BTN))
            .center_y(Length::Fixed(BTN)),
        )
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            border: round(6.0),
            ..button::Style::default()
        });
        if self.playing_id.is_some() {
            save_btn = save_btn.on_press(Message::SaveCurrentClicked);
        }
        // right / middle click always opens the picker (left click saves first)
        let save_btn: Element<'_, Message> = if self.playing_id.is_some() {
            iced::widget::mouse_area(save_btn)
                .on_right_press(Message::OpenAddPopoverCurrent)
                .on_middle_press(Message::OpenAddPopoverCurrent)
                .into()
        } else {
            save_btn.into()
        };

        let title_btn = pb_link(
            title.clone(),
            16,
            TEXT,
            TEXT,
            PbLink::Title,
            current_track
                .as_ref()
                .map(|tr| Message::OpenTrackPage(Box::new(tr.clone()))),
        );
        let title_btn: Element<'_, Message> = match speed_label {
            Some(label) => row![title_btn, speed_badge(label)]
                .spacing(SPEED_BADGE_GAP)
                .align_y(iced::Alignment::Center)
                .into(),
            None => title_btn,
        };

        // Title and artist each sit in a fixed-height line; the block is the
        // cover's height and clips, so an over-long glyph run is cut instead
        // of spilling onto the shuffle button.
        // With no artist (e.g. "Nothing playing") the title is the whole
        // block, so it centres on the cover instead of riding the top line.
        let mut info_col = column![container(title_btn)
            .height(Length::Fixed(TITLE_H))
            .align_y(iced::alignment::Vertical::Center)]
        .spacing(2);
        if !self.playing_artist.is_empty() {
            info_col = info_col.push(
                container(artist_widget)
                    .height(Length::Fixed(ARTIST_H))
                    .align_y(iced::alignment::Vertical::Center),
            );
        }
        let track_info = optical_center(
            container(info_col)
                .width(Length::Fixed(INFO_W))
                .height(Length::Fixed(COVER))
                .align_y(iced::alignment::Vertical::Center)
                .clip(true),
            16.0,
        );

        let actions_row = row![
            track_cover_btn,
            track_info,
            shuffle_btn,
            repeat_btn,
            save_btn,
        ]
        .spacing(GAP)
        .align_y(iced::Alignment::Center);

        let left = container(actions_row)
            .width(Length::Fixed(SIDE_W))
            .align_y(iced::alignment::Vertical::Center)
            .clip(true);

        // Center: the waveform gets the full player-bar height again. Right
        // click on any position opens the centered comment/reaction panel.
        let center = container(self.view_waveform())
            .width(Length::Fill)
            .align_y(iced::alignment::Vertical::Center);

        // Right: speed + queue + volume (fixed width SIDE_W to match left side)
        // speeds sit on a 0.01 grid: half a step tells 1.01x from 1.0x
        let is_speed_custom = (self.playback_speed - 1.0).abs() >= 0.005;
        let speed_label = if (self.playback_speed - 1.25).abs() < 0.005 {
            "1.25x ⚡".to_string()
        } else if (self.playback_speed - 1.0).abs() < 0.005 {
            "1.0x".to_string()
        } else {
            format!("{:.2}x", self.playback_speed)
        };
        let speed_color = if is_speed_custom { ORANGE } else { TEXT_MUTED };
        // Fixed-width pill: "1.0x" -> "1.25x ⚡" -> "1.50x" never moves the
        // queue / volume controls.
        let speed_btn = button(
            container(
                text(speed_label)
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| text::Style {
                        color: Some(speed_color),
                    }),
            )
            .center_x(Length::Fixed(SPEED_W))
            .center_y(Length::Fixed(BTN))
            .clip(true),
        )
        .on_press(Message::CyclePlaybackSpeed)
        .padding(0)
        .style(move |_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => {
                    if is_speed_custom {
                        Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.12)))
                    } else {
                        None
                    }
                }
            },
            border: Border {
                radius: border::Radius::from(6.0),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..button::Style::default()
        });

        // wheel over the pill steps 0.05x; right click opens the speed popup
        let speed_btn = iced::widget::mouse_area(speed_btn)
            .on_right_press(Message::OpenSpeedPopup)
            .on_scroll(|d| Message::SpeedWheel(wheel_notches(d)));

        let queue_active = self.show_queue;
        let queue_color = if queue_active { ORANGE } else { TEXT_MUTED };
        let queue_btn = button(
            container(
                row![
                    text(icons::QUEUE)
                        .font(FA_SOLID)
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| text::Style {
                            color: Some(queue_color)
                        }),
                    text("Queue")
                        .size(14)
                        .wrapping(text::Wrapping::None)
                        .style(move |_| text::Style {
                            color: Some(queue_color)
                        }),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .center_x(Length::Fixed(QUEUE_W))
            .center_y(Length::Fixed(BTN))
            .clip(true),
        )
        .on_press(Message::ToggleQueue)
        .padding(0)
        .style(move |_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => {
                    if queue_active {
                        Some(Background::Color(Color::from_rgba(1.0, 0.33, 0.0, 0.10)))
                    } else {
                        None
                    }
                }
            },
            border: Border {
                radius: border::Radius::from(6.0),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..button::Style::default()
        });

        let mute_glyph = if self.is_muted || self.volume < 0.005 {
            icons::VOLUME_XMARK
        } else if self.volume < 0.33 {
            icons::VOLUME_OFF
        } else if self.volume < 0.66 {
            icons::VOLUME_LOW
        } else {
            icons::VOLUME_HIGH
        };
        // The four volume glyphs are 10-20px wide; a fixed square keeps the
        // slider and every control to its left from jumping as volume changes.
        let vol_btn = button(
            container(
                text(mute_glyph)
                    .font(FA_SOLID)
                    .size(16)
                    .wrapping(text::Wrapping::None)
                    .style(move |_| {
                        if self.is_muted {
                            text::Style {
                                color: Some(ORANGE),
                            }
                        } else {
                            muted()
                        }
                    }),
            )
            .center_x(Length::Fixed(BTN))
            .center_y(Length::Fixed(BTN)),
        )
        .on_press(Message::ToggleMute)
        .padding(0)
        .style(|_, status| button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(BG_HOVER)),
                _ => None,
            },
            border: round(6.0),
            ..button::Style::default()
        });

        let vol_canvas = canvas(VolumeCanvas {
            volume: self.volume,
            is_muted: self.is_muted,
            color_t: self.vol_color_t,
        })
        .width(Length::Fixed(VOL_W))
        .height(Length::Fixed(VOL_H));

        let pct_val = if self.is_muted {
            0
        } else {
            (self.volume * 100.0).round() as i32
        };
        let percent_widget = optical_center(
            container(
                text(format!("{pct_val}%"))
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(|_| muted())
                    .align_x(iced::alignment::Horizontal::Right),
            )
            .width(Length::Fixed(PCT_W))
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .clip(true),
            12.0,
        );

        let right = container(
            row![speed_btn, queue_btn, vol_btn, vol_canvas, percent_widget]
                .spacing(GAP)
                .align_y(iced::Alignment::Center),
        )
        .width(Length::Fixed(SIDE_W))
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center)
        .clip(true);

        // 76px bar; the row (48px tall: the waveform) is centred in it, so
        // cover, title, controls, waveform and times share one centre line.
        let bar = container(
            row![left, center, right]
                .spacing(16)
                .align_y(iced::Alignment::Center),
        )
        .padding(Padding::from([6.0, PB_PAD_X]))
        .height(Length::Fixed(PB_H))
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(BG_PLAYER)),
            border: Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: border::Radius::from(0.0),
            },
            ..container::Style::default()
        });

        bar.into()
    }

    /// SoundCloud-style waveform: mirrored bars, played part orange,
    /// click/drag to seek, small `|` comment markers along the bottom.
    fn view_waveform(&self) -> Element<'_, Message> {
        // Position / duration sit in fixed-width slots ("123:45" at 11px is
        // ~32px) inside a fixed-height line (11px text, 1.3 line height), so
        // "9:59" -> "10:00" never moves anything. (PB_WAVE_INSET adds these
        // up to place the visual banner.)
        const WAVE_H: f32 = PB_WAVE_H;
        const TIME_W: f32 = 44.0; // "1:02:03" at 12px
        const TIME_GAP: f32 = 10.0;

        let dur = self.dur_ms.max(1) as f32;
        let pos_frac = (self.pos_ms as f32 / dur).clamp(0.0, 1.0);

        // comment dots: (fraction, played), one per 4px column of the
        // waveform. A popular track has thousands of comments and each dot
        // is two tessellated circles; closer than that they'd overlap anyway.
        let columns = ((self.window_size.width - 2.0 * PB_WAVE_INSET) / 4.0).max(1.0) as usize;
        let mut taken = vec![false; columns + 1];
        let markers: Vec<(f32, bool)> = self
            .wave_comments
            .iter()
            .filter_map(|c| {
                let f = (c.ts_ms as f32 / dur).clamp(0.0, 1.0);
                let col = ((f * columns as f32) as usize).min(columns);
                (!std::mem::replace(&mut taken[col], true)).then_some((f, c.ts_ms <= self.pos_ms))
            })
            .collect();

        // which marker is active: hovered one takes priority, otherwise the one
        // the playhead just reached (within ±1.4s window)
        let hover = self.hover_frac;
        let hover_idx = hover.and_then(|hf| {
            let mut best: Option<(usize, f32)> = None;
            for (idx, c) in self.wave_comments.iter().enumerate() {
                let f = (c.ts_ms as f32 / dur).clamp(0.0, 1.0);
                // ~ marker proximity in fraction space (roughly 14px of 700px)
                let tol = 0.02;
                let d = (f - hf).abs();
                if d <= tol && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((idx, d));
                }
            }
            best.map(|(idx, _)| idx)
        });

        let playing_idx = self
            .wave_comments
            .iter()
            .enumerate()
            .filter(|(_, c)| (c.ts_ms as i64 - self.pos_ms as i64).abs() <= 1400)
            .min_by_key(|(_, c)| (c.ts_ms as i64 - self.pos_ms as i64).abs())
            .map(|(idx, _)| idx);

        let active_idx = hover_idx.or(playing_idx);

        // Single-line overlay label drawn on the canvas: cap the author too so
        // a very long username can't push the comment body off the pill.
        let active_label = active_idx.map(|idx| {
            let c = &self.wave_comments[idx];
            let f = (c.ts_ms as f32 / dur).clamp(0.0, 1.0);
            (
                f,
                format!("{}: {}", trunc(&c.author, 24), trunc(&c.body, 64)),
            )
        });

        // The track's visual banner (SoundCloud "visuals"), baked dark and
        // round (see bake_wave_visual), lies under the bars.
        let visual = self
            .wave_visual
            .as_ref()
            .filter(|v| Some(v.track_id) == self.playing_id);
        let wave_canvas = canvas(WaveCanvas {
            bars: self.wave_bars.clone(),
            progress: pos_frac,
            hover,
            markers: std::sync::Arc::new(markers),
            active: active_label.as_ref().map(|(f, _)| *f),
            active_label,
            color_t: self.wave_color_t,
            on_visual: visual.is_some(),
        })
        .width(Length::Fill)
        .height(Length::Fixed(WAVE_H));
        let wave_widget: Element<'_, Message> = match visual {
            Some(v) => stack(vec![
                image(v.handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fixed(WAVE_H))
                    .content_fit(iced::ContentFit::Fill)
                    .into(),
                wave_canvas.into(),
            ])
            .width(Length::Fill)
            .height(Length::Fixed(WAVE_H))
            .into(),
            None => wave_canvas.into(),
        };

        // Spotify's progress layout: elapsed | bar | total on one centre line,
        // so the waveform sits on the player bar's centre line instead of
        // riding 8px high above a dangling row of timestamps.
        let time = |ms: u64, align: iced::alignment::Horizontal| {
            optical_center(
                container(
                    text(fmt_time(ms))
                        .size(12)
                        .wrapping(text::Wrapping::None)
                        .style(|_| muted()),
                )
                .width(Length::Fixed(TIME_W))
                .align_x(align)
                .clip(true),
                12.0,
            )
        };

        row![
            time(self.pos_ms, iced::alignment::Horizontal::Right),
            wave_widget,
            time(self.dur_ms, iced::alignment::Horizontal::Left),
        ]
        .spacing(TIME_GAP)
        .width(Length::Fill)
        .height(Length::Fixed(WAVE_H))
        .align_y(iced::Alignment::Center)
        .into()
    }
}

/// Custom-drawn mirrored SoundCloud waveform with a comment-marker strip.
/// Loading spinner: a quarter-gap ring turning once per ~0.9 s.
struct Spinner {
    /// Rotation, 0..1 of a turn.
    turn: f32,
}

impl canvas::Program<Message> for Spinner {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 - 2.0;
        frame.stroke(
            &canvas::Path::circle(center, radius),
            canvas::Stroke::default()
                .with_color(Color { a: 0.15, ..TEXT })
                .with_width(3.0),
        );
        let start = self.turn * std::f32::consts::TAU;
        let arc = canvas::Path::new(|b| {
            b.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: iced::Radians(start),
                end_angle: iced::Radians(start + std::f32::consts::FRAC_PI_2 * 1.5),
            });
        });
        frame.stroke(
            &arc,
            canvas::Stroke::default()
                .with_color(ORANGE)
                .with_width(3.0)
                .with_line_cap(canvas::LineCap::Round),
        );
        vec![frame.into_geometry()]
    }
}

struct WaveCanvas {
    bars: std::sync::Arc<Vec<f32>>,
    progress: f32,
    hover: Option<f32>,
    markers: std::sync::Arc<Vec<(f32, bool)>>,
    /// Where the hovered (or just reached) comment is: drawn big on top.
    active: Option<f32>,
    active_label: Option<(f32, String)>,
    color_t: f32,
    /// The track's visual banner is drawn underneath: lighter bars.
    on_visual: bool,
}

impl canvas::Program<Message> for WaveCanvas {
    /// The pointer is over the waveform. The canvas sees every pointer move
    /// in the window; only the ones over it, and the one that leaves it,
    /// become messages (each message rebuilds the whole UI).
    type State = bool;

    fn update(
        &self,
        inside: &mut bool,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let f = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0);
                    return (canvas::event::Status::Captured, Some(Message::WaveSeek(f)));
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if cursor.is_over(bounds) {
                    return (canvas::event::Status::Captured, Some(Message::PlayerToggle));
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let f = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0);
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::WaveContextOpen(f)),
                    );
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y,
                        mouse::ScrollDelta::Pixels { y, .. } => y,
                    };
                    if y.abs() > 0.05 {
                        return (canvas::event::Status::Captured, Some(Message::WaveWheel(y)));
                    }
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    *inside = true;
                    let f = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0);
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::WaveHover(Some(f))),
                    );
                }
                let left = std::mem::take(inside);
                (
                    canvas::event::Status::Ignored,
                    left.then_some(Message::WaveHover(None)),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => (
                canvas::event::Status::Ignored,
                std::mem::take(inside).then_some(Message::WaveHover(None)),
            ),
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        _state: &bool,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let w = bounds.width;
        let h = bounds.height;
        let mid = h / 2.0;
        let max_half = (mid - 4.0) * 0.96;

        // Over a (darkened) banner the grey bars are lighter, so they still
        // stand out from its brightest parts.
        let (bar_grey, placeholder_grey) = if self.on_visual {
            (
                Color::from_rgb(0.62, 0.62, 0.62),
                Color::from_rgb(0.45, 0.45, 0.45),
            )
        } else {
            (
                Color::from_rgb(0.40, 0.40, 0.40),
                Color::from_rgb(0.20, 0.20, 0.20),
            )
        };

        // --- mirrored waveform bars around the center line ---
        if self.bars.is_empty() {
            let placeholder_bars = 160;
            let step = w / placeholder_bars as f32;
            let bar_w = (step * 0.50).max(1.0);
            for i in 0..placeholder_bars {
                let x = i as f32 * step;
                frame.fill_rectangle(
                    Point::new(x, mid - 2.0),
                    Size::new(bar_w, 4.0),
                    placeholder_grey,
                );
            }
        } else {
            let n = self.bars.len().max(1);
            let step = w / n as f32;
            // Whole-pixel bars: fractional widths rasterise as 1 or 2 px and
            // shimmer; snapped ones stay crisp and evenly spaced.
            let bar_w = (step * 0.60).round().max(1.0);
            for (i, amp) in self.bars.iter().enumerate() {
                let x = (i as f32 * step).round();
                let bh = (amp * max_half).max(1.0).round();
                let played = (i as f32 / n as f32) <= self.progress;
                let color = if played {
                    Color {
                        r: bar_grey.r + (ORANGE.r - bar_grey.r) * self.color_t,
                        g: bar_grey.g + (ORANGE.g - bar_grey.g) * self.color_t,
                        b: bar_grey.b + (ORANGE.b - bar_grey.b) * self.color_t,
                        a: 1.0,
                    }
                } else {
                    bar_grey
                };
                frame.fill_rectangle(Point::new(x, mid - bh), Size::new(bar_w, bh * 2.0), color);
            }
        }

        // --- center line (полоска посередине) ---
        frame.fill_rectangle(
            Point::new(0.0, mid.floor()),
            Size::new(w, 1.0),
            Color::from_rgb(0.35, 0.35, 0.35),
        );

        // --- comment circles sitting just above the line ---
        let cy = mid - 7.0;
        let mark_played_color = Color {
            r: 0.40 + (ORANGE.r - 0.40) * self.color_t,
            g: 0.40 + (ORANGE.g - 0.40) * self.color_t,
            b: 0.40 + (ORANGE.b - 0.40) * self.color_t,
            a: 1.0,
        };
        let dot = |frame: &mut canvas::Frame, f: f32, radius: f32, color: Color| {
            let x = (f * w).clamp(3.0, (w - 3.0).max(3.0));
            // dark ring so circles stay visible over bars
            frame.fill(
                &canvas::Path::circle(Point::new(x, cy), radius + 1.5),
                Color::from_rgb(0.06, 0.06, 0.06),
            );
            frame.fill(&canvas::Path::circle(Point::new(x, cy), radius), color);
        };
        for (f, played) in self.markers.iter() {
            let color = if *played {
                mark_played_color
            } else {
                Color::from_rgb(0.72, 0.72, 0.72)
            };
            dot(&mut frame, *f, 3.6, color);
        }
        if let Some(f) = self.active {
            dot(&mut frame, f, 5.5, Color::from_rgb(1.0, 0.70, 0.35));
        }

        // --- hover indicator + playhead ---
        if let Some(hf) = self.hover {
            let x = (hf * w).round().clamp(0.0, w - 1.0);
            frame.fill_rectangle(
                Point::new(x, 0.0),
                Size::new(1.0, h),
                Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            );
        }
        // Snap the playhead to whole pixels. A 1.5px line at a fractional x
        // rasterises as 1 or 2 px depending on where it lands, and each
        // position update moves it ~0.3px, so it flickered constantly.
        let px = (self.progress * w).round().clamp(0.0, (w - 2.0).max(0.0));
        frame.fill_rectangle(
            Point::new(px, 0.0),
            Size::new(2.0, h),
            Color::from_rgba(1.0, 1.0, 1.0, 0.85),
        );

        // Note: the live emoji reactions are drawn by the standalone
        // ReactionOverlay canvas above — full-window, no pill background, and
        // they fly to mid-screen. That lets them escape the small waveform box.

        // --- comment text overlay (ник: сообщение) ---
        if let Some((f, label)) = &self.active_label {
            let approx_w = (label.chars().count() as f32 * 6.2 + 16.0).min(w - 8.0);
            let mut x = (*f * w) - approx_w / 2.0;
            x = x.clamp(4.0, (w - approx_w - 4.0).max(4.0));
            let y = 2.0;
            // background pill
            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(approx_w, 18.0),
                Color::from_rgba(0.06, 0.06, 0.06, 0.92),
            );
            frame.fill_rectangle(
                Point::new(x + 0.0, y + 18.0 - 1.5),
                Size::new(approx_w, 1.5),
                ORANGE,
            );
            frame.fill_text(canvas::Text {
                content: label.clone(),
                position: Point::new(x + 8.0, y + 3.0),
                color: TEXT,
                size: 11.0.into(),
                font: iced::Font::default(),
                horizontal_alignment: iced::alignment::Horizontal::Left,
                vertical_alignment: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                ..canvas::Text::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &bool,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        // Only over the waveform: rows and columns take their children's
        // strongest interaction, so an unconditional Pointer here put the
        // hand cursor over the whole window (every label looked like a link).
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

/// Live reactions over the whole window: others' reactions floating up from
/// the waveform's playhead, and the burst of your own.
struct ReactionOverlay {
    floating: Vec<FloatingReaction>,
    particles: Vec<Particle>,
}

impl canvas::Program<Message> for ReactionOverlay {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        let now = std::time::Instant::now();
        // Only Segoe UI Emoji has colour emoji glyphs; the canvas doesn't fall
        // back to it on its own.
        let emoji_font = iced::Font::with_name("Segoe UI Emoji");
        let emoji =
            |frame: &mut canvas::Frame, codepoint: &str, at: Point, size: f32, alpha: f32| {
                if let Some(handle) = reaction_image(codepoint) {
                    frame.draw_image(
                        Rectangle::new(
                            Point::new(at.x - size / 2.0, at.y - size / 2.0),
                            Size::new(size, size),
                        ),
                        canvas::Image::new(handle).opacity(alpha),
                    );
                    return;
                }
                let content = WaveReaction {
                    second: 0,
                    codepoint: codepoint.to_string(),
                }
                .emoji();
                frame.fill_text(canvas::Text {
                    content,
                    position: at,
                    color: Color {
                        a: alpha,
                        ..Color::WHITE
                    },
                    size: size.into(),
                    font: emoji_font,
                    horizontal_alignment: iced::alignment::Horizontal::Center,
                    vertical_alignment: iced::alignment::Vertical::Center,
                    shaping: text::Shaping::Advanced,
                    ..canvas::Text::default()
                });
            };

        // The waveform: between the player bar's side blocks, above its
        // centre line.
        let wave_x0 = PB_WAVE_INSET;
        let wave_x1 = (w - PB_WAVE_INSET).max(wave_x0 + 1.0);
        let start_y = h - PB_H + 12.0;
        for r in &self.floating {
            if now < r.spawned_at {
                continue; // waiting its turn in its second
            }
            let age = now.saturating_duration_since(r.spawned_at).as_millis() as f32;
            let t = (age / REACTION_FLOAT_MS).clamp(0.0, 1.0);
            // ease-out rise, fade over the second half
            let ease = 1.0 - (1.0 - t) * (1.0 - t);
            let alpha = if t < 0.5 { 1.0 } else { 1.0 - (t - 0.5) / 0.5 };
            let scale = 0.7 + 0.3 * ease;
            let x = wave_x0 + r.x_frac * (wave_x1 - wave_x0) + r.drift * REACTION_DRIFT * ease;
            let y = start_y - REACTION_RISE * ease;
            emoji(
                &mut frame,
                &r.codepoint,
                Point::new(x, y),
                26.0 * scale,
                alpha,
            );
        }

        const GRAVITY: f32 = 900.0; // px/s², so the burst arcs back down
        for p in &self.particles {
            let age = now.saturating_duration_since(p.spawned_at).as_millis() as f32;
            let t = age / p.life_ms;
            if t >= 1.0 {
                continue;
            }
            let secs = age / 1000.0;
            let at = Point::new(
                p.origin.x + p.vx * secs,
                p.origin.y + p.vy * secs + 0.5 * GRAVITY * secs * secs,
            );
            let alpha = p.alpha
                * if t < 0.85 {
                    1.0
                } else {
                    1.0 - (t - 0.85) / 0.15
                };
            let size = 30.0 * p.scale_at(t);
            if size >= 1.0 {
                emoji(&mut frame, &p.codepoint, at, size, alpha);
            }
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::default() // fully transparent to clicks
    }
}

/// Interactive volume slider canvas supporting drag, mouse wheel (±3%), and middle-click mute.
struct VolumeCanvas {
    volume: f32,
    is_muted: bool,
    color_t: f32,
}

#[derive(Default)]
struct VolumeCanvasState {
    is_dragging: bool,
    is_hovered: bool,
    /// Wheel notches not yet a volume step: a touchpad scrolls in pixels,
    /// a few at a time.
    wheel_acc: f32,
}

impl canvas::Program<Message> for VolumeCanvas {
    type State = VolumeCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.is_dragging = true;
                    let f = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0);
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::PlayerVolume(f)),
                    );
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    // persist once per drag, not per mouse move
                    return (canvas::event::Status::Captured, Some(Message::VolumeCommit));
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                state.is_hovered = cursor.is_over(bounds);
                if state.is_dragging {
                    if let Some(pos) = cursor.position() {
                        let rel_x = pos.x - bounds.x;
                        let f = (rel_x / bounds.width.max(1.0)).clamp(0.0, 1.0);
                        return (
                            canvas::event::Status::Captured,
                            Some(Message::PlayerVolume(f)),
                        );
                    }
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                state.is_hovered = false;
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if cursor.is_over(bounds) {
                    return (canvas::event::Status::Captured, Some(Message::ToggleMute));
                }
                (canvas::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    // 3% per wheel notch; a touchpad's pixels add up to notches
                    state.wheel_acc += match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y,
                        mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                    };
                    let steps = state.wheel_acc.trunc();
                    if steps != 0.0 {
                        state.wheel_acc -= steps;
                        return (
                            canvas::event::Status::Captured,
                            Some(Message::VolumeRelative(0.03 * steps)),
                        );
                    }
                    return (canvas::event::Status::Captured, None);
                }
                (canvas::event::Status::Ignored, None)
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let w = bounds.width;
        let h = bounds.height;
        // whole-pixel rail: a 3.5px bar at a half-pixel centre blurred both edges
        let cy = (h / 2.0).round();

        let rail_h = 4.0;
        let r = rail_h / 2.0;
        let effective_vol = if self.is_muted {
            0.0
        } else {
            self.volume.clamp(0.0, 1.0)
        };

        // Background track (dark rail with rounded ends)
        frame.fill(
            &canvas::Path::circle(Point::new(r, cy), r),
            Color::from_rgb(0.20, 0.20, 0.20),
        );
        frame.fill(
            &canvas::Path::circle(Point::new(w - r, cy), r),
            Color::from_rgb(0.20, 0.20, 0.20),
        );
        if w > rail_h {
            frame.fill_rectangle(
                Point::new(r, cy - r),
                Size::new(w - rail_h, rail_h),
                Color::from_rgb(0.20, 0.20, 0.20),
            );
        }

        // Active track (orange fill with rounded end, fades to grey when muted)
        let fill_w = w * effective_vol;
        if fill_w > 0.0 {
            let orange = if state.is_hovered || state.is_dragging {
                ORANGE
            } else {
                Color::from_rgb(0.95, 0.40, 0.10)
            };
            let grey = Color::from_rgb(0.38, 0.38, 0.38);
            let fill_color = Color {
                r: grey.r + (orange.r - grey.r) * self.color_t,
                g: grey.g + (orange.g - grey.g) * self.color_t,
                b: grey.b + (orange.b - grey.b) * self.color_t,
                a: 1.0,
            };
            frame.fill(&canvas::Path::circle(Point::new(r, cy), r), fill_color);
            if fill_w > r {
                let rect_w = (fill_w - r).min(w - rail_h);
                frame.fill_rectangle(Point::new(r, cy - r), Size::new(rect_w, rail_h), fill_color);
            }
        }

        // Handle (knob)
        // Spotify's knob: a clean 12px white dot (antialiased via MSAA).
        let knob_radius = if state.is_dragging || state.is_hovered {
            6.0
        } else {
            5.0
        };
        let knob_x = fill_w
            .round()
            .clamp(knob_radius, (w - knob_radius).max(knob_radius));
        let knob_color = if self.color_t < 0.2 {
            Color::from_rgb(0.60, 0.60, 0.60)
        } else {
            Color::WHITE
        };
        frame.fill(
            &canvas::Path::circle(Point::new(knob_x, cy), knob_radius),
            knob_color,
        );

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.is_dragging || cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Spotify's 16px save glyphs, drawn from its SVG geometry (16-unit grid) so
/// they stay crisp at any scale: outline circle-plus (not saved), filled
/// check circle (saved; the check is a real hole), and the plain plus of
/// "New playlist".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlyphKind {
    Add,
    Saved,
    Plus,
}

struct SaveGlyph {
    kind: GlyphKind,
    color: Color,
}

impl canvas::Program<Message> for SaveGlyph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.shape(&mut frame, bounds);
        vec![frame.into_geometry()]
    }
}

impl SaveGlyph {
    fn shape(&self, frame: &mut canvas::Frame, bounds: Rectangle) {
        let k = bounds.width.min(bounds.height) / 16.0;
        let p = |x: f32, y: f32| Point::new(x * k, y * k);
        let pen = canvas::Stroke::default()
            .with_color(self.color)
            .with_width(1.5 * k)
            .with_line_cap(canvas::LineCap::Round);
        match self.kind {
            GlyphKind::Add => {
                // ring 1.5 thick (r 6.5..8) and a 1.5 plus with round ends
                frame.stroke(&canvas::Path::circle(p(8.0, 8.0), 7.25 * k), pen);
                frame.stroke(&canvas::Path::line(p(5.0, 8.0), p(11.0, 8.0)), pen);
                frame.stroke(&canvas::Path::line(p(8.0, 5.0), p(8.0, 11.0)), pen);
            }
            GlyphKind::Plus => {
                frame.stroke(&canvas::Path::line(p(1.5, 8.0), p(14.5, 8.0)), pen);
                frame.stroke(&canvas::Path::line(p(8.0, 1.5), p(8.0, 14.5)), pen);
            }
            GlyphKind::Saved => {
                // disc minus the check outline (even-odd), round-capped ends
                let cap = |b: &mut canvas::path::Builder, cx: f32, cy: f32, a0: f32, a1: f32| {
                    for i in 1..=6 {
                        let a = (a0 + (a1 - a0) * i as f32 / 6.0).to_radians();
                        b.line_to(p(cx + 0.75 * a.cos(), cy + 0.75 * a.sin()));
                    }
                };
                let path = canvas::Path::new(|b| {
                    b.circle(p(8.0, 8.0), 8.0 * k);
                    b.move_to(p(11.748, 6.03));
                    cap(b, 11.218, 5.5, 45.0, -135.0);
                    b.line_to(p(6.218, 9.44));
                    b.line_to(p(4.813, 8.034));
                    cap(b, 4.2825, 8.5645, -45.0, -225.0);
                    b.line_to(p(6.218, 11.561));
                    b.close();
                });
                frame.fill(
                    &path,
                    canvas::Fill {
                        style: canvas::Style::Solid(self.color),
                        rule: canvas::fill::Rule::EvenOdd,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_icons_resolve_normalized_services_and_hosts() {
        assert_eq!(
            web_profile_icon(Some("Instagram"), "https://example.com/u").0,
            icons::BRAND_INSTAGRAM
        );
        assert_eq!(
            web_profile_icon(Some("Unknown Service"), "https://www.tiktok.com/@artist").0,
            icons::BRAND_TIKTOK
        );
        assert_eq!(
            web_profile_icon(None, "https://music.apple.com/us/artist/name").0,
            icons::BRAND_APPLE
        );
        assert_eq!(
            web_profile_icon(Some("unknown"), "https://example.org/profile").0,
            icons::EXTERNAL_LINK
        );
        assert_eq!(
            web_profile_icon(Some("unknown"), "https://notinstagram.com/profile").0,
            icons::EXTERNAL_LINK
        );
    }

    #[test]
    fn story_receipt_state_retries_once_then_fails_or_completes() {
        assert_eq!(
            story_receipt_outcome(0, true),
            StoryReceiptOutcome::Complete
        );
        assert_eq!(story_receipt_outcome(0, false), StoryReceiptOutcome::Retry);
        assert_eq!(
            story_receipt_outcome(1, true),
            StoryReceiptOutcome::Complete
        );
        assert_eq!(story_receipt_outcome(1, false), StoryReceiptOutcome::Failed);
        assert_eq!(story_receipt_outcome(2, false), StoryReceiptOutcome::Failed);
    }

    #[test]
    fn offline_store_reads_partial_and_old_files() {
        // a file from before a field existed (or an empty one) still loads
        let empty: OfflineStore = serde_json::from_str("{}").expect("empty store");
        assert!(empty.tracks.is_empty() && empty.playlists.is_empty() && empty.me.is_none());

        let store = OfflineStore {
            tracks: vec![Track {
                id: 7,
                title: "Seven".into(),
                ..Default::default()
            }],
            playlists: vec![PlaylistDetail {
                id: 42,
                id_or_urn: "soundcloud:playlists:42".into(),
                title: "Mix".into(),
                description: None,
                artwork_url: None,
                author: "someone".into(),
                author_avatar: None,
                author_id: None,
                permalink_url: None,
                track_count: 1,
                tracks: vec![Track {
                    id: 7,
                    ..Default::default()
                }],
                is_album: false,
            }],
            my_playlists: vec![Playlist {
                id: 5,
                title: "Mine".into(),
                ..Default::default()
            }],
            liked_playlists: vec![],
            me: Some(Me {
                id: 1,
                username: "me".into(),
                ..Default::default()
            }),
            my_followings: vec![],
        };
        let json = serde_json::to_string(&store).expect("serialize");
        let back: OfflineStore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.tracks[0].id, 7);
        assert_eq!(back.playlists[0].id, 42);
        assert_eq!(back.playlists[0].tracks.len(), 1);
        assert_eq!(back.my_playlists[0].id, 5);
        assert_eq!(back.me.map(|m| m.id), Some(1));
    }

    #[test]
    fn stored_playlist_matches_home_card_id() {
        // offline Home cards open playlists by their plain numeric id
        assert_eq!(parse_trailing_id("42"), 42);
        assert_eq!(parse_trailing_id("soundcloud:playlists:42"), 42);
    }

    #[test]
    fn reaction_codepoints_are_the_apps_emoji() {
        let emoji: Vec<String> = QUICK_REACTIONS
            .iter()
            .map(|(c, _)| {
                WaveReaction {
                    second: 0,
                    codepoint: c.to_string(),
                }
                .emoji()
            })
            .collect();
        assert_eq!(emoji, ["🔥", "👏", "🥹"]);
    }

    #[test]
    fn track_tags_read_like_soundcloud() {
        let t = Track {
            genre: Some("Hip-Hop & Rap".into()),
            tag_list: Some(
                r#"digicore "hip hop" rap soundcloud:source=web-record #glitch "#.into(),
            ),
            ..Default::default()
        };
        assert_eq!(
            track_tags(&t),
            ["Hip-Hop & Rap", "digicore", "hip hop", "rap", "glitch"]
        );
        let none = Track {
            genre: Some(" ".into()),
            ..Default::default()
        };
        assert!(track_tags(&none).is_empty());
    }

    #[test]
    fn burst_particles_pop_in_and_shrink_away() {
        let burst = Particle::burst("1f525", Point::ORIGIN);
        assert_eq!(burst.len(), BURST_PARTICLES);
        for p in &burst {
            assert!(p.vy < 0.0, "particles leave upward");
            assert!(p.life_ms >= 1500.0 && p.life_ms <= 2200.0 * 1.3);
            assert_eq!(p.scale_at(0.0), 0.0);
            assert!((p.scale_at(0.3) - p.scale).abs() < 1e-6);
            assert!(p.scale_at(0.95) < p.scale_at(0.5));
            assert!(p.scale_at(1.0).abs() < 1e-6);
        }
    }
}
