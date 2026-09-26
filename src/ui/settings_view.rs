//! Settings, laid out like Spotify's preferences (open.spotify.com/
//! preferences): one centred column of sections, each a bold heading over
//! rows, the setting on the left and its control on the right. Every
//! switch, select and slider applies at once (see `set_pref`); only the
//! proxy field waits for its Save.

use super::*;
// named here: through the glob they'd clash with std's column! and row!
use crate::config::{ArtistSort, OpenAtLogin, ShuffleStyle, VolumeLevel};
use crate::dsp::{EQ_BANDS, EQ_RANGE_DB};
use iced::widget::{column, row, stack};

/// A change made in Settings (or by a shortcut); `set_pref` applies and
/// saves it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pref {
    Zoom(f32),
    PrivateSession(bool),
    /// Seconds; 0 turns crossfading off.
    Crossfade(u8),
    NormalizeVolume(bool),
    VolumeLevel(VolumeLevel),
    MonoAudio(bool),
    Equalizer(bool),
    DisableComments(bool),
    DisableReactions(bool),
    DisableWaveBackground(bool),
    /// A band dragged on the equalizer's graph (saved when let go).
    EqBand(usize, f32),
    EqPreset(usize),
    Autoplay(bool),
    ShuffleStyle(ShuffleStyle),
    OpenAtLogin(OpenAtLogin),
    CloseMinimizes(bool),
    AllowSystemTray(bool),
    CompactLibrary(bool),
    StreamQuality(StreamQuality),
    DownloadQuality(DownloadQuality),
    CacheTracks(bool),
    ArtistSort(ArtistSort),
    CheckUpdates(bool),
    DebugMode(bool),
}

/// The zoom levels, Spotify's seven.
pub const ZOOM_STEPS: [f32; 7] = [0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];

/// Equalizer presets: dB for 60 Hz, 150 Hz, 400 Hz, 1 kHz, 2.4 kHz, 15 kHz.
pub const EQ_PRESETS: [(&str, [f32; 6]); 17] = [
    ("Flat", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Acoustic", [4.0, 3.0, 1.0, 1.0, 3.0, 3.5]),
    ("Bass Booster", [6.0, 4.5, 1.5, 0.0, 0.0, 0.0]),
    ("Bass Reducer", [-6.0, -4.5, -1.5, 0.0, 0.0, 0.0]),
    ("Classical", [4.0, 3.0, -1.0, -1.0, 2.0, 4.0]),
    ("Dance", [5.0, 4.0, 0.5, 0.0, 3.0, 2.0]),
    ("Electronic", [4.5, 3.5, 0.0, -1.5, 2.0, 4.5]),
    ("Hip-Hop", [5.0, 4.0, 1.0, -1.0, 1.0, 2.5]),
    ("Jazz", [3.5, 2.0, 1.0, 2.0, -1.0, 3.0]),
    ("Loudness", [5.5, 3.0, 0.0, -1.0, 2.0, 5.0]),
    ("Lounge", [-2.5, -1.0, 1.5, 2.0, 1.0, -0.5]),
    ("Piano", [2.5, 1.5, 0.0, 2.5, 3.0, 2.0]),
    ("Pop", [-1.5, 1.0, 3.5, 3.5, 1.0, -1.5]),
    ("Rock", [4.5, 2.5, -1.5, -1.0, 2.5, 4.5]),
    ("Small Speakers", [5.5, 4.0, 2.0, 0.0, -1.0, -2.0]),
    ("Treble Booster", [0.0, 0.0, 0.0, 1.5, 4.5, 6.0]),
    ("Vocal Booster", [-2.0, -1.0, 2.5, 4.5, 3.0, 0.0]),
];

/// Settings > Audio quality > Streaming quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamQuality {
    Automatic,
    High,
    Low,
}

impl std::fmt::Display for StreamQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StreamQuality::Automatic => "Automatic",
            StreamQuality::High => "High",
            StreamQuality::Low => "Low",
        })
    }
}

/// Settings > Audio quality > Download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadQuality {
    High,
    Low,
}

impl std::fmt::Display for DownloadQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DownloadQuality::High => "High",
            DownloadQuality::Low => "Low",
        })
    }
}

/// The equalizer's preset select: a preset, or "Custom" for gains dragged
/// by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PresetChoice(Option<usize>);

impl std::fmt::Display for PresetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.map_or("Custom", |i| EQ_PRESETS[i].0))
    }
}

/// Wrappers giving the config's choices their labels (Display can't be
/// implemented here for the lib's types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LevelChoice(VolumeLevel);

impl std::fmt::Display for LevelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            VolumeLevel::Loud => "Loud",
            VolumeLevel::Normal => "Normal",
            VolumeLevel::Quiet => "Quiet",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShuffleChoice(ShuffleStyle);

impl std::fmt::Display for ShuffleChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            ShuffleStyle::FewerRepeats => "Fewer repeats",
            ShuffleStyle::Standard => "Standard",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SortChoice(ArtistSort);

impl std::fmt::Display for SortChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            ArtistSort::Subscribers => "Subscribers",
            ArtistSort::Alphabet => "Alphabet",
            ArtistSort::Tracks => "Count of tracks",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoginChoice(OpenAtLogin);

impl std::fmt::Display for LoginChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            OpenAtLogin::No => "No",
            OpenAtLogin::Minimized => "Minimized",
            OpenAtLogin::Yes => "Yes",
        })
    }
}

/// The equalizer's graph: plot height, and the gutters for its labels.
const EQ_GRAPH_H: f32 = 240.0;
const EQ_LEFT: f32 = 64.0;
const EQ_BOTTOM: f32 = 36.0;
const EQ_TOP: f32 = 12.0;
const EQ_RIGHT: f32 = 24.0;

/// Target loudness of a volume level (see dsp).
fn level_rms(level: VolumeLevel) -> f32 {
    match level {
        VolumeLevel::Loud => crate::dsp::LOUD_RMS,
        VolumeLevel::Normal => crate::dsp::NORMAL_RMS,
        VolumeLevel::Quiet => crate::dsp::QUIET_RMS,
    }
}

/// A full-width block in a section (zoom, equalizer, a slider), and the
/// words "Search in Settings" finds it by.
fn settings_block<'a>(
    find: &str,
    view: impl Into<Element<'a, Message>>,
) -> (String, Element<'a, Message>) {
    (find.to_string(), view.into())
}

/// A keyboard key as Spotify draws one in running text.
fn key_chip(label: &str) -> Element<'static, Message> {
    container(
        text(label.to_string())
            .size(13)
            .font(UI_BOLD)
            .wrapping(text::Wrapping::None)
            .style(|_| bright()),
    )
    .padding(Padding::from([2, 7]))
    .style(|_| container::Style {
        border: Border {
            radius: border::Radius::from(4.0),
            width: 1.0,
            color: TEXT_MUTED,
        },
        ..container::Style::default()
    })
    .into()
}

impl App {
    /// Downloads' quality for the API: "highest" unless set to Low.
    pub(super) fn download_quality(&self) -> String {
        self.settings
            .download_quality
            .clone()
            .unwrap_or_else(|| "highest".into())
    }

    /// Hand the audio settings to the player: its stage (see dsp), and
    /// whether it keeps what it streams.
    pub(super) fn apply_audio_prefs(&self) {
        let Some(p) = &self.player else { return };
        let s = &self.settings;
        p.cache_audio
            .store(s.cache_tracks, std::sync::atomic::Ordering::Relaxed);
        p.dsp.set_eq(s.equalizer, s.eq_gains);
        p.dsp.set_mono(s.mono_audio);
        p.dsp
            .set_normalize(s.normalize_volume, level_rms(s.volume_level));
    }

    /// Ctrl+- / Ctrl++ (a zoom step), Ctrl+0 (back to 100%).
    pub(super) fn zoom_step(&mut self, step: i8) -> Task<Message> {
        let cur = ZOOM_STEPS
            .iter()
            .position(|z| (z - self.settings.zoom).abs() < 0.05)
            .unwrap_or(3) as i32;
        let next = match step {
            0 => 3,
            s => (cur + i32::from(s)).clamp(0, ZOOM_STEPS.len() as i32 - 1),
        };
        self.set_pref(Pref::Zoom(ZOOM_STEPS[next as usize]))
    }

    /// Apply a settings change and save it (a dragged equalizer band is
    /// saved when it's let go, by SavePrefs).
    pub(super) fn set_pref(&mut self, pref: Pref) -> Task<Message> {
        let mut save = true;
        let mut task = Task::none();
        match pref {
            Pref::Zoom(z) => {
                let z = z.clamp(0.7, 1.3);
                let old = self.settings.zoom.clamp(0.7, 1.3);
                self.settings.zoom = z;
                // Sizes come in zoomed units, and a new zoom sends no resize:
                // the window keeps its pixels, which now hold more or fewer
                // of them.
                self.window_size = Size::new(
                    self.window_size.width * old / z,
                    self.window_size.height * old / z,
                );
                task = self.bake_wave_visual();
            }
            Pref::PrivateSession(on) => {
                self.settings.private_until_ms = on.then_some(u64::MAX);
                self.update_discord_rpc();
                self.show_toast(
                    if on {
                        "Private session enabled"
                    } else {
                        "Private session ended"
                    },
                    ToastKind::Info,
                );
            }
            Pref::Crossfade(secs) => self.settings.crossfade_secs = secs.min(12),
            Pref::NormalizeVolume(on) => {
                self.settings.normalize_volume = on;
                self.apply_audio_prefs();
            }
            Pref::VolumeLevel(level) => {
                self.settings.volume_level = level;
                self.apply_audio_prefs();
            }
            Pref::MonoAudio(on) => {
                self.settings.mono_audio = on;
                self.apply_audio_prefs();
            }
            Pref::Equalizer(on) => {
                self.settings.equalizer = on;
                self.apply_audio_prefs();
            }
            Pref::DisableComments(on) => {
                self.settings.disable_comments = on;
                if on {
                    self.wave_comments.clear();
                    self.comments_next = None;
                    self.comments_loading = false;
                    self.wave_context_frac = None;
                    self.wave_context_track = None;
                } else {
                    let mut reloads = Vec::new();
                    if let Some(track_id) = self.playing_id.filter(|id| {
                        !self.settings.offline_mode
                            || crate::config::cached_comments_path(*id).exists()
                    }) {
                        self.comments_loading = true;
                        reloads.push(Task::perform(
                            fetch_comments(track_id, self.settings.offline_mode),
                            |r| Message::CommentsLoaded(r.map_err(|e| e.to_string())),
                        ));
                    }
                    if let Some(track_id) = self.track_page.as_ref().map(|page| page.track.id) {
                        reloads.push(Task::perform(
                            fetch_inspector_comments(track_id),
                            |r| match r {
                                Ok(comments) => Message::TrackPageCommentsLoaded(Ok(comments)),
                                Err(e) => Message::TrackPageCommentsLoaded(Err(e.to_string())),
                            },
                        ));
                    }
                    if let Some(track_id) = self.inspector_track.as_ref().map(|track| track.id) {
                        self.inspector_tasks_pending += 1;
                        reloads.push(Task::perform(
                            fetch_inspector_comments(track_id),
                            move |r| {
                                Message::InspectorCommentsLoaded(
                                    track_id,
                                    r.map(|(_, comments)| comments).map_err(|e| e.to_string()),
                                )
                            },
                        ));
                    }
                    task = Task::batch(reloads);
                }
            }
            Pref::DisableReactions(on) => {
                self.settings.disable_reactions = on;
                if on {
                    self.reactions.clear();
                    self.reactions_fetched.clear();
                    self.floating.clear();
                    self.particles.clear();
                    self.wave_context_frac = None;
                    self.wave_context_track = None;
                }
            }
            Pref::DisableWaveBackground(on) => {
                self.settings.disable_wave_background = on;
                if on {
                    self.wave_visual = None;
                    self.wave_visual_src = None;
                    self.wave_visual_baking = false;
                } else if let Some(track) =
                    self.playing_id.and_then(|id| self.find_track_anywhere(id))
                {
                    task = self.load_wave_visual(&track);
                }
            }
            Pref::EqBand(band, db) => {
                if let Some(g) = self.settings.eq_gains.get_mut(band) {
                    *g = db.clamp(-EQ_RANGE_DB, EQ_RANGE_DB);
                }
                self.apply_audio_prefs();
                save = false;
            }
            Pref::EqPreset(i) => {
                if let Some((_, gains)) = EQ_PRESETS.get(i) {
                    self.settings.eq_gains = *gains;
                    self.apply_audio_prefs();
                }
            }
            Pref::Autoplay(on) => self.settings.autoplay = on,
            Pref::ShuffleStyle(style) => self.settings.shuffle_style = style,
            Pref::OpenAtLogin(mode) => match crate::config::set_open_at_login(mode) {
                Ok(()) => self.settings.open_at_login = mode,
                Err(e) => {
                    crate::log!("open at login: {e}");
                    self.show_toast(format!("Couldn't change startup: {e}"), ToastKind::Error);
                    save = false;
                }
            },
            Pref::CloseMinimizes(on) => self.settings.close_minimizes = on,
            Pref::AllowSystemTray(on) => {
                #[cfg(windows)]
                {
                    if on {
                        match crate::system_tray::TrayController::start() {
                            Ok(tray) => {
                                self.system_tray = Some(tray);
                                self.settings.allow_system_tray = true;
                            }
                            Err(e) => {
                                self.show_toast(
                                    format!("Couldn't enable system tray: {e}"),
                                    ToastKind::Error,
                                );
                                save = false;
                            }
                        }
                    } else {
                        self.system_tray = None;
                        self.settings.allow_system_tray = false;
                    }
                }
                #[cfg(not(windows))]
                {
                    self.settings.allow_system_tray = false;
                    save = false;
                }
            }
            Pref::CompactLibrary(on) => self.settings.compact_library = on,
            Pref::StreamQuality(q) => {
                self.settings.audio_quality = match q {
                    StreamQuality::Automatic => None,
                    StreamQuality::High => Some("highest".into()),
                    StreamQuality::Low => Some("lowest".into()),
                };
            }
            Pref::CacheTracks(on) => {
                self.settings.cache_tracks = on;
                self.apply_audio_prefs();
                if !on {
                    // off means no music kept: what was cached goes now
                    // (downloads stay)
                    self.settings.save();
                    return self.drop_streamed_audio();
                }
            }
            Pref::ArtistSort(sort) => {
                self.settings.artist_sort = sort;
                sort_followings(&mut self.my_followings, sort);
            }
            Pref::CheckUpdates(on) => self.settings.check_updates = on,
            Pref::DebugMode(on) => {
                self.settings.debug_mode = on;
                self.show_toast(
                    if on {
                        "Debug mode will start on the next launch"
                    } else {
                        "Debug mode disabled for the next launch"
                    },
                    ToastKind::Info,
                );
            }
            Pref::DownloadQuality(q) => {
                self.settings.download_quality = match q {
                    DownloadQuality::High => None,
                    DownloadQuality::Low => Some("lowest".into()),
                };
            }
        }
        if save {
            self.settings.save();
        }
        task
    }

    pub(super) fn view_settings(&self) -> Element<'_, Message> {
        let query = self.settings_search.trim().to_lowercase();
        let s = &self.settings;
        let mut sections: Vec<Element<'_, Message>> = Vec::new();

        // --- Account: only signed out, the ways in (signed in, the account
        // menu has Log out)
        if !self.state.authenticated {
            let mut account: Vec<(String, Element<'_, Message>)> = Vec::new();
            if let Some(e) = &self.login_error {
                account.push((
                    format!("account sign in error {e}"),
                    container(text(e.clone()).size(14).style(|_| t_color(HEART)))
                        .padding(Padding::from([8, 12]))
                        .width(Length::Fill)
                        .style(|_| panel(BG_CARD, 4.0))
                        .into(),
                ));
            }
            account.push(settings_row(
                "account sign in with soundcloud log in".into(),
                settings_label(
                    "Sign in with SoundCloud",
                    Some("Opens SoundCloud's sign-in window. Everything works with this sign-in."),
                ),
                settings_primary_btn("Sign in", Message::LoginStart),
            ));
            account.push(settings_row(
                "account browser session cookies sign in".into(),
                settings_label(
                    "Use your browser's session",
                    Some(
                        "Sign in on soundcloud.com, then copy the Cookie header from DevTools > \
                         Network.",
                    ),
                ),
                settings_outline_btn("Open SoundCloud", Some(Message::CookieOpenBrowser)),
            ));
            let pasted = if self.cookie_input.is_empty() {
                "Paste it from the clipboard, or into the field".to_string()
            } else {
                format!("{} characters ready", self.cookie_input.chars().count())
            };
            account.push(settings_row(
                "account cookie header oauth_token paste clipboard authorize".into(),
                settings_label("Cookie header or oauth_token", Some(&pasted)),
                column![
                    settings_input(
                        "Cookie or oauth_token",
                        &self.cookie_input,
                        Message::CookieInput,
                        Some(Message::CookieAuthorize),
                    ),
                    row![
                        settings_outline_btn("Paste", Some(Message::PasteClipboard)),
                        settings_outline_btn("Authorize", Some(Message::CookieAuthorize)),
                    ]
                    .spacing(8),
                ]
                .spacing(8)
                .align_x(iced::Alignment::End),
            ));
            sections.extend(settings_section("Account", account, &query));
        }

        // --- Audio quality
        let stream = match s.audio_quality.as_deref() {
            Some("highest") => StreamQuality::High,
            Some("lowest") => StreamQuality::Low,
            _ => StreamQuality::Automatic,
        };
        let download = if s.download_quality.as_deref() == Some("lowest") {
            DownloadQuality::Low
        } else {
            DownloadQuality::High
        };
        sections.extend(settings_section(
            "Audio quality",
            vec![
                settings_row(
                    "streaming quality automatic high low aac mp3 bitrate".into(),
                    settings_label(
                        "Streaming quality",
                        Some("High prefers 256k AAC and 320k MP3, Low 64k and 128k streams."),
                    ),
                    settings_select(
                        vec![
                            StreamQuality::Automatic,
                            StreamQuality::High,
                            StreamQuality::Low,
                        ],
                        stream,
                        |q| Message::SetPref(Pref::StreamQuality(q)),
                    ),
                ),
                settings_row(
                    "download quality high low offline".into(),
                    settings_label("Download", None),
                    settings_select(
                        vec![DownloadQuality::High, DownloadQuality::Low],
                        download,
                        |q| Message::SetPref(Pref::DownloadQuality(q)),
                    ),
                ),
            ],
            &query,
        ));

        // --- Your Library
        sections.extend(settings_section(
            "Your Library",
            vec![
                settings_row(
                    "your library compact layout sidebar".into(),
                    settings_label("Use compact library layout", None),
                    settings_switch(
                        s.compact_library,
                        Message::SetPref(Pref::CompactLibrary(!s.compact_library)),
                    ),
                ),
                settings_row(
                    "your library sort artists subscribers followers alphabet name tracks".into(),
                    settings_label("Sort artists in Your Library", None),
                    settings_select(
                        vec![
                            SortChoice(ArtistSort::Subscribers),
                            SortChoice(ArtistSort::Alphabet),
                            SortChoice(ArtistSort::Tracks),
                        ],
                        SortChoice(s.artist_sort),
                        |c| Message::SetPref(Pref::ArtistSort(c.0)),
                    ),
                ),
            ],
            &query,
        ));

        // --- Display
        sections.extend(settings_section(
            "Display",
            vec![
                settings_block(
                    "display zoom level dense default spacious size scale",
                    self.zoom_panel(),
                ),
                settings_row(
                    "display prefer artists from metadata uploader".into(),
                    settings_label(
                        "Prefer Artists from metadata",
                        Some(
                            "Use the artist credited in SoundCloud track metadata instead of \
                             the uploader's display name. Falls back to the uploader when missing.",
                        ),
                    ),
                    settings_switch(
                        s.prefer_artist_from_metadata,
                        Message::SettingsPreferArtistFromMetadata(!s.prefer_artist_from_metadata),
                    ),
                ),
                settings_row(
                    "display hide scrollbars scroll".into(),
                    settings_label("Hide scrollbars", None),
                    settings_switch(
                        s.hide_scrollbars,
                        Message::SettingsHideScrollbarsToggled(!s.hide_scrollbars),
                    ),
                ),
            ],
            &query,
        ));

        // --- Listening activity and insights
        let private = s.private_session();
        let private_desc =
            "Hides what you play from Discord and keeps it out of your listening history.";
        let discord_desc = if !s.discord_rpc {
            "Your Discord profile shows the track you're playing."
        } else if crate::discord_rpc::is_connected() {
            "Your Discord profile shows the track you're playing. Connected."
        } else {
            "Your Discord profile shows the track you're playing. Connecting to Discord…"
        };
        sections.extend(settings_section(
            "Listening activity and insights",
            vec![
                settings_row(
                    format!("listening activity private session hide {private_desc}"),
                    settings_label("Private session", Some(private_desc)),
                    settings_switch(private, Message::SetPref(Pref::PrivateSession(!private))),
                ),
                settings_row(
                    "listening activity discord rich presence status profile".into(),
                    settings_label("Listening activity", Some(discord_desc)),
                    settings_switch(
                        s.discord_rpc,
                        Message::SettingsDiscordRpcToggled(!s.discord_rpc),
                    ),
                ),
            ],
            &query,
        ));

        // --- Playback
        let crossfade = s.crossfade_secs;
        let speeds = self.track_speeds.len();
        let speeds_desc = if speeds == 0 {
            "A speed you set while a track plays is kept for that track and shown next to \
             its title."
                .to_string()
        } else {
            format!(
                "{speeds} {} at a speed of {} own, shown next to the title.",
                if speeds == 1 {
                    "track plays"
                } else {
                    "tracks play"
                },
                if speeds == 1 { "its" } else { "their" },
            )
        };
        let mut playback = vec![settings_row(
            "playback crossfade songs transition fade".into(),
            settings_label("Crossfade songs", None),
            settings_switch(
                crossfade > 0,
                Message::SetPref(Pref::Crossfade(if crossfade > 0 { 0 } else { 5 })),
            ),
        )];
        if crossfade > 0 {
            playback.push(settings_block(
                "playback crossfade seconds",
                row![
                    text("1 s").size(12).style(|_| dim()),
                    iced::widget::slider(1..=12u8, crossfade, |v| {
                        Message::SetPref(Pref::Crossfade(v))
                    })
                    .on_release(Message::SavePrefs)
                    .width(Length::Fill)
                    .style(|_, status| {
                        let hot = !matches!(status, iced::widget::slider::Status::Active);
                        iced::widget::slider::Style {
                            rail: iced::widget::slider::Rail {
                                backgrounds: (
                                    Background::Color(if hot { ORANGE } else { TEXT }),
                                    Background::Color(Color::from_rgb(0.33, 0.33, 0.33)),
                                ),
                                width: 4.0,
                                border: round(2.0),
                            },
                            handle: iced::widget::slider::Handle {
                                shape: iced::widget::slider::HandleShape::Circle { radius: 6.0 },
                                background: Background::Color(TEXT),
                                border_width: 0.0,
                                border_color: Color::TRANSPARENT,
                            },
                        }
                    }),
                    text("12 s").size(12).style(|_| dim()),
                    container(
                        text(format!("{crossfade} s"))
                            .size(14)
                            .font(UI_BOLD)
                            .style(|_| bright()),
                    )
                    .width(Length::Fixed(40.0))
                    .align_x(iced::alignment::Horizontal::Right),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            ));
        }
        playback.push(settings_row(
            "playback normalize volume level same loudness".into(),
            settings_label(
                "Normalize volume - Set the same volume level for all tracks",
                None,
            ),
            settings_switch(
                s.normalize_volume,
                Message::SetPref(Pref::NormalizeVolume(!s.normalize_volume)),
            ),
        ));
        playback.push(settings_row(
            "playback volume level loud normal quiet environment".into(),
            settings_label(
                "Volume level - Adjust the volume for your environment. Loud may diminish \
                 audio quality. Applies while Normalize volume is on.",
                None,
            ),
            settings_select(
                vec![
                    LevelChoice(VolumeLevel::Loud),
                    LevelChoice(VolumeLevel::Normal),
                    LevelChoice(VolumeLevel::Quiet),
                ],
                LevelChoice(s.volume_level),
                |c| Message::SetPref(Pref::VolumeLevel(c.0)),
            ),
        ));
        playback.push(settings_row(
            "playback mono audio left right speakers".into(),
            settings_label(
                "Mono audio - Makes the left and right speakers play the same audio",
                None,
            ),
            settings_switch(
                s.mono_audio,
                Message::SetPref(Pref::MonoAudio(!s.mono_audio)),
            ),
        ));
        playback.push(settings_row(
            "playback comments disable hide".into(),
            settings_label(
                "Disable comments",
                Some("Hide comments and comment controls."),
            ),
            settings_switch(
                s.disable_comments,
                Message::SetPref(Pref::DisableComments(!s.disable_comments)),
            ),
        ));
        playback.push(settings_row(
            "playback reactions disable hide".into(),
            settings_label(
                "Disable reactions",
                Some("Hide reactions and reaction controls."),
            ),
            settings_switch(
                s.disable_reactions,
                Message::SetPref(Pref::DisableReactions(!s.disable_reactions)),
            ),
        ));
        playback.push(settings_row(
            "playback equalizer eq bass treble presets".into(),
            settings_label("Equalizer", None),
            settings_switch(s.equalizer, Message::SetPref(Pref::Equalizer(!s.equalizer))),
        ));
        playback.push(settings_block(
            "playback equalizer eq bass treble presets graph",
            self.eq_panel(),
        ));
        playback.push(settings_row(
            "playback waveform background visual disable".into(),
            settings_label(
                "Disable background for waves",
                Some("Hide track artwork behind the player waveform."),
            ),
            settings_switch(
                s.disable_wave_background,
                Message::SetPref(Pref::DisableWaveBackground(!s.disable_wave_background)),
            ),
        ));
        playback.push(settings_row(
            format!("playback track speeds playback speed saved reset {speeds_desc}"),
            settings_label("Track speeds", Some(&speeds_desc)),
            settings_outline_btn("Reset", (speeds > 0).then_some(Message::ClearTrackSpeeds)),
        ));
        playback.push(settings_row(
            "playback offline mode downloaded disk".into(),
            settings_label(
                "Offline mode",
                Some("Play only what's downloaded, without a network."),
            ),
            settings_switch(
                s.offline_mode,
                Message::SettingsOfflineModeToggled(!s.offline_mode),
            ),
        ));
        playback.push(settings_row(
            "playback autoplay nonstop similar related".into(),
            settings_label(
                "Autoplay",
                Some("Enjoy nonstop listening. When your queue ends, similar tracks play."),
            ),
            settings_switch(s.autoplay, Message::SetPref(Pref::Autoplay(!s.autoplay))),
        ));
        sections.extend(settings_section("Playback", playback, &query));

        // --- Shuffle
        sections.extend(settings_section(
            "Shuffle",
            vec![settings_row(
                "shuffle style fewer repeats standard random".into(),
                settings_label(
                    "Pick your preferred shuffle style.",
                    Some("Fewer repeats keeps tracks by the same artist apart."),
                ),
                settings_select(
                    vec![
                        ShuffleChoice(ShuffleStyle::FewerRepeats),
                        ShuffleChoice(ShuffleStyle::Standard),
                    ],
                    ShuffleChoice(s.shuffle_style),
                    |c| Message::SetPref(Pref::ShuffleStyle(c.0)),
                ),
            )],
            &query,
        ));

        // --- Unlock
        let google_desc = if self.yt_signed_in {
            "Signed in to YouTube Music with your Google account."
        } else {
            "Not signed in. Sign in if YouTube asks to confirm you're not a bot. Ads play \
             muted and are skipped as soon as YouTube allows; with Premium there are none."
        };
        let mut unlock = vec![settings_row(
            "unlock through youtube music go+ preview blocked region".into(),
            settings_label(
                "Through YouTube Music",
                Some(
                    "Go+ tracks (a 30 s preview here) and tracks blocked in your region play \
                     in full in YouTube Music's own web player. These can't be downloaded.",
                ),
            ),
            settings_switch(
                s.youtube_music,
                Message::SettingsYoutubeToggled(!s.youtube_music),
            ),
        )];
        if s.youtube_music {
            unlock.push(settings_row(
                format!("unlock youtube google account sign in out {google_desc}"),
                settings_label("Google account", Some(google_desc)),
                if self.yt_signed_in {
                    settings_outline_btn("Sign out", Some(Message::YoutubeSignOut))
                } else {
                    settings_outline_btn("Sign in with Google", Some(Message::YoutubeSignIn))
                },
            ));
        }
        unlock.push(settings_row(
            "unlock through proxy region blocked unavailable country".into(),
            settings_label(
                "Through Proxy",
                Some(
                    "Tracks blocked in your region play through free public proxies from \
                     other countries, found automatically. Your sign-in never goes through \
                     them.",
                ),
            ),
            settings_switch(
                s.bypass_unavailable,
                Message::SettingsBypassToggled(!s.bypass_unavailable),
            ),
        ));
        unlock.push(settings_row(
            "unlock your own proxy socks5 http https network user password".into(),
            settings_label(
                "Your proxy",
                Some(
                    "Everything Wavify loads goes through it: protocol://host:port or \
                     protocol://user:pass@host:port (http, https, socks5).",
                ),
            ),
            settings_input(
                "socks5://127.0.0.1:9050",
                &self.proxy_draft,
                Message::SettingsProxyChanged,
                Some(Message::SettingsSave),
            ),
        ));
        sections.extend(settings_section("Unlock", unlock, &query));

        // --- Storage
        let (downloads, cache) = match self.storage {
            Some((cache, downloads)) => (fmt_bytes(downloads), fmt_bytes(cache)),
            None => ("…".to_string(), "…".to_string()),
        };
        let n_downloads = self.offline_store.tracks.len();
        let downloads_desc = format!(
            "{n_downloads} {} you downloaded for offline listening",
            if n_downloads == 1 { "track" } else { "tracks" }
        );
        let remove_btn: Element<'_, Message> = if self.confirm_remove_downloads {
            settings_primary_btn("Remove them all", Message::RemoveAllDownloads)
        } else {
            settings_outline_btn(
                "Remove all downloads",
                (n_downloads > 0).then_some(Message::RemoveAllDownloads),
            )
        };
        let location = crate::config::cache_dir().display().to_string();
        sections.extend(settings_section(
            "Storage",
            vec![
                settings_row(
                    format!("storage downloads offline remove {downloads_desc}"),
                    settings_label(&format!("Downloads: {downloads}"), Some(&downloads_desc)),
                    remove_btn,
                ),
                settings_row(
                    "storage cache tracks locally keep offline stream".into(),
                    settings_label(
                        "Cache tracks locally",
                        Some(
                            "Tracks you play are kept on this computer with their cover, \
                             waveform banner, waveform, comments and details, so they start \
                             at once next time. Off: every track streams from SoundCloud and \
                             no music is kept (downloads stay).",
                        ),
                    ),
                    settings_switch(
                        s.cache_tracks,
                        Message::SetPref(Pref::CacheTracks(!s.cache_tracks)),
                    ),
                ),
                settings_row(
                    "storage cache clear temporary files".into(),
                    settings_label(
                        &format!("Cache: {cache}"),
                        Some(
                            "Tracks played before, their waveforms, comments and cover art, \
                             kept so they start at once",
                        ),
                    ),
                    settings_outline_btn(
                        "Clear cache",
                        self.storage
                            .is_some_and(|(cache, _)| cache > 0)
                            .then_some(Message::ClearCache),
                    ),
                ),
                settings_row(
                    format!("storage offline location folder {location}"),
                    settings_label("Offline storage location", Some(&location)),
                    settings_outline_btn("Open folder", Some(Message::OpenStorageFolder)),
                ),
            ],
            &query,
        ));

        // --- Startup and window behaviour
        sections.extend(settings_section(
            "Updates",
            vec![
                settings_row(
                    "updates check automatically when Wavify starts".into(),
                    settings_label(
                        "Check updates",
                        Some("Check for new Wavify releases at startup"),
                    ),
                    settings_switch(
                        s.check_updates,
                        Message::SetPref(Pref::CheckUpdates(!s.check_updates)),
                    ),
                ),
                settings_row(
                    "updates check now current version".into(),
                    settings_label("Check for updates now", None),
                    settings_outline_btn(
                        if self.update_checking {
                            "Checking…"
                        } else {
                            "Check updates now"
                        },
                        (!self.update_checking).then_some(Message::CheckUpdates),
                    ),
                ),
            ],
            &query,
        ));

        let mut window_options = vec![
            settings_row(
                "startup open automatically log into computer login windows".into(),
                settings_label(
                    "Open Wavify automatically after you log into the computer",
                    None,
                ),
                settings_select(
                    vec![
                        LoginChoice(OpenAtLogin::No),
                        LoginChoice(OpenAtLogin::Minimized),
                        LoginChoice(OpenAtLogin::Yes),
                    ],
                    LoginChoice(s.open_at_login),
                    |c| Message::SetPref(Pref::OpenAtLogin(c.0)),
                ),
            ),
            settings_row(
                "window close button minimize".into(),
                settings_label("Close button should minimize the Wavify window", None),
                settings_switch(
                    s.close_minimizes,
                    Message::SetPref(Pref::CloseMinimizes(!s.close_minimizes)),
                ),
            ),
            settings_row(
                "debug mode console server requests responses payloads diagnostics logs".into(),
                settings_label(
                    "Debug mode",
                    Some("On the next launch, show a console and save detailed server requests, responses, links, and payloads. Credentials are redacted."),
                ),
                settings_switch(
                    s.debug_mode,
                    Message::SetPref(Pref::DebugMode(!s.debug_mode)),
                ),
            ),
        ];
        #[cfg(windows)]
        window_options.push(settings_row(
            "window close hide to system tray background keep running".into(),
            settings_label(
                "Allow Wavify in the system tray",
                Some("Closing hides the window but keeps Wavify running. Open it or quit from the tray icon."),
            ),
            settings_switch(
                s.allow_system_tray,
                Message::SetPref(Pref::AllowSystemTray(!s.allow_system_tray)),
            ),
        ));
        sections.extend(settings_section(
            "Startup and window behaviour",
            window_options,
            &query,
        ));

        // Header: the title, and "Search in Settings" as Spotify's opened
        // search pill (tinted, 32px, round).
        let search = text_input("Search in Settings", &self.settings_search)
            .on_input(Message::SettingsSearch)
            .icon(text_input::Icon {
                font: FA_SOLID,
                code_point: '\u{f002}',
                size: Some(iced::Pixels(13.0)),
                spacing: 10.0,
                side: text_input::Side::Left,
            })
            .size(14)
            .line_height(text::LineHeight::Absolute(iced::Pixels(18.0)))
            .padding(Padding {
                top: 7.0,
                right: 14.0,
                bottom: 7.0,
                left: 14.0,
            })
            .width(Length::Fixed(220.0))
            .style(|_, status| text_input::Style {
                background: Background::Color(match status {
                    text_input::Status::Hovered | text_input::Status::Focused => BG_TINT_HI,
                    _ => BG_TINT,
                }),
                border: Border {
                    radius: border::Radius::from(SET_CTRL_H / 2.0),
                    width: if matches!(status, text_input::Status::Focused) {
                        1.0
                    } else {
                        0.0
                    },
                    color: TEXT_MUTED,
                },
                icon: Color::from_rgba(1.0, 1.0, 1.0, 0.7),
                placeholder: Color::from_rgba(1.0, 1.0, 1.0, 0.7),
                value: TEXT,
                selection: Color::from_rgba(1.0, 0.33, 0.0, 0.3),
            });
        let header = container(
            row![
                text("Settings")
                    .size(32)
                    .font(UI_BOLD)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright()),
                horizontal_space(),
                search,
            ]
            .spacing(16)
            .align_y(iced::Alignment::Center),
        )
        .padding(pad4(0.0, 0.0, 16.0, 0.0));

        let mut page = column![header].spacing(32);
        if sections.is_empty() {
            page = page.push(
                column![
                    text(format!(
                        "No settings match \u{201c}{}\u{201d}",
                        self.settings_search.trim()
                    ))
                    .size(16)
                    .font(UI_BOLD)
                    .style(|_| bright()),
                    text("Check the spelling, or try another word.")
                        .size(14)
                        .style(|_| dim()),
                ]
                .spacing(8),
            );
        }
        for section in sections {
            page = page.push(section);
        }

        let page = container(
            container(page)
                .width(Length::Fill)
                .max_width(900.0)
                .padding(32),
        )
        .center_x(Length::Fill);
        self.v_scrollable(page).into()
    }

    /// Display > Zoom level: Spotify's block, three pictures of the layout
    /// over seven stops from 70% to 130%, and Reset.
    fn zoom_panel(&self) -> Element<'_, Message> {
        const DOT: f32 = 18.0;
        // a stop: its dot, 10px, and its 14px label (18.2 tall)
        const STOPS_H: f32 = DOT + 10.0 + 20.0;
        let zoom = self.settings.zoom;
        let at = |z: f32| (z - zoom).abs() < 0.05;

        // a small picture of the page: header lines over a grid of cards
        let picture = |label: &'static str, z: f32, lines: usize, cols: usize, rows: usize| {
            let on = at(z);
            let fill = if on {
                ORANGE
            } else {
                Color::from_rgb(0.36, 0.36, 0.36)
            };
            let block = move |w: f32, h: f32| -> Element<'static, Message> {
                container(iced::widget::Space::new(Length::Fixed(w), Length::Fixed(h)))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(fill)),
                        border: round(1.0),
                        ..container::Style::default()
                    })
                    .into()
            };
            const INNER: f32 = 72.0;
            let gap = 3.0;
            let mut pic = column![].spacing(gap);
            for _ in 0..lines {
                pic = pic.push(row![block(34.5, 3.0), block(34.5, 3.0)].spacing(gap));
            }
            let card_w = (INNER - gap * (cols as f32 - 1.0)) / cols as f32;
            let card_h =
                ((50.0 - lines as f32 * (3.0 + gap)) - gap * (rows as f32 - 1.0)) / rows as f32;
            for _ in 0..rows {
                let mut r = row![].spacing(gap);
                for _ in 0..cols {
                    r = r.push(block(card_w, card_h));
                }
                pic = pic.push(r);
            }
            column![
                button(
                    container(pic)
                        .padding(6)
                        .width(Length::Fixed(INNER + 12.0))
                        .height(Length::Fixed(62.0))
                        .clip(true),
                )
                .on_press(Message::SetPref(Pref::Zoom(z)))
                .padding(0)
                .style(move |_, status| button::Style {
                    background: None,
                    border: Border {
                        radius: border::Radius::from(2.0),
                        width: 1.0,
                        color: if on || matches!(status, button::Status::Hovered) {
                            TEXT
                        } else {
                            TEXT_MUTED
                        },
                    },
                    ..button::Style::default()
                }),
                text(label)
                    .size(14)
                    .font(UI_BOLD)
                    .wrapping(text::Wrapping::None)
                    .style(|_| bright()),
            ]
            .spacing(14)
            .align_x(iced::Alignment::Center)
        };
        let slot = |el: Option<Element<'static, Message>>| -> Element<'static, Message> {
            container(el.unwrap_or_else(|| horizontal_space().into()))
                .width(Length::FillPortion(1))
                .center_x(Length::FillPortion(1))
                .into()
        };
        let mut pictures = row![].align_y(iced::Alignment::End);
        for (i, _) in ZOOM_STEPS.iter().enumerate() {
            pictures = pictures.push(slot(match i {
                0 => Some(picture("Dense", 0.7, 2, 5, 3).into()),
                3 => Some(picture("Default", 1.0, 2, 4, 2).into()),
                6 => Some(picture("Spacious", 1.3, 3, 3, 1).into()),
                _ => None,
            }));
        }

        // the stops on a thin line through their middles
        let mut stops = row![];
        for &z in &ZOOM_STEPS {
            let on = at(z);
            let dot = button(iced::widget::Space::new(
                Length::Fixed(DOT),
                Length::Fixed(DOT),
            ))
            .on_press(Message::SetPref(Pref::Zoom(z)))
            .padding(0)
            .style(move |_, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(BG_CARD)),
                    border: Border {
                        radius: border::Radius::from(DOT / 2.0),
                        width: if on { 4.5 } else { 1.5 },
                        color: if on {
                            ORANGE
                        } else if hot {
                            TEXT
                        } else {
                            TEXT_MUTED
                        },
                    },
                    ..button::Style::default()
                }
            });
            stops = stops.push(
                container(
                    column![
                        dot,
                        text(format!("{:.0}%", z * 100.0))
                            .size(14)
                            .wrapping(text::Wrapping::None)
                            .style(move |_| if on { bright() } else { dim() }),
                    ]
                    .spacing(10)
                    .align_x(iced::Alignment::Center),
                )
                .width(Length::FillPortion(1))
                .center_x(Length::FillPortion(1)),
            );
        }
        let rail = column![
            iced::widget::Space::new(Length::Fill, Length::Fixed(DOT / 2.0 - 0.5)),
            row![
                iced::widget::Space::new(Length::FillPortion(1), Length::Fixed(1.0)),
                container(iced::widget::Space::new(Length::Fill, Length::Fixed(1.0)))
                    .width(Length::FillPortion(12))
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                        ..container::Style::default()
                    }),
                iced::widget::Space::new(Length::FillPortion(1), Length::Fixed(1.0)),
            ],
        ]
        // a stack takes its size from its first layer: the rail is as tall
        // as the stops over it, or they'd be squeezed into its 1px line
        .height(Length::Fixed(STOPS_H));
        let stops = stops.height(Length::Fixed(STOPS_H));

        let intro = column![
            text(
                "Adjusting the zoom level can help you make the most of Wavify and adapt it \
                 to your preferences or needs."
            )
            .size(14)
            .style(|_| dim()),
            row![
                text("You can also change the zoom level by pressing")
                    .size(14)
                    .style(|_| dim()),
                key_chip("Ctrl"),
                key_chip("-"),
                text("or").size(14).style(|_| dim()),
                key_chip("Ctrl"),
                key_chip("+"),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(8);

        let stops_block = container(
            column![
                pictures,
                stack![rail, stops],
                row![
                    horizontal_space(),
                    settings_outline_btn(
                        "Reset",
                        (!at(1.0)).then_some(Message::SetPref(Pref::Zoom(1.0)))
                    ),
                ],
            ]
            .spacing(24),
        )
        .padding(pad4(28.0, 24.0, 20.0, 24.0))
        .width(Length::Fill)
        .style(|_| panel(BG_CARD, 8.0));

        column![
            text("Zoom level").size(14).style(|_| bright()),
            intro,
            stops_block,
        ]
        .spacing(12)
        .into()
    }

    /// Playback > Equalizer: presets, the six bands on a graph to drag, and
    /// Reset. Off, it's shown dimmed and still.
    fn eq_panel(&self) -> Element<'_, Message> {
        let s = &self.settings;
        let preset = EQ_PRESETS
            .iter()
            .position(|(_, g)| g.iter().zip(&s.eq_gains).all(|(a, b)| (a - b).abs() < 0.01));
        let choices: Vec<PresetChoice> = (0..EQ_PRESETS.len())
            .map(|i| PresetChoice(Some(i)))
            .collect();
        let presets = row![
            text("Presets").size(14).style(|_| dim()),
            settings_select(choices, PresetChoice(preset), |c| match c.0 {
                Some(i) => Message::SetPref(Pref::EqPreset(i)),
                None => Message::Noop,
            }),
        ]
        .spacing(16)
        .align_y(iced::Alignment::Center);
        let graph = canvas(EqGraph {
            gains: s.eq_gains,
            enabled: s.equalizer,
        })
        .width(Length::Fill)
        .height(Length::Fixed(EQ_GRAPH_H + EQ_TOP + EQ_BOTTOM));
        let flat = preset == Some(0);
        container(
            column![
                presets,
                graph,
                row![
                    horizontal_space(),
                    settings_outline_btn(
                        "Reset",
                        (!flat).then_some(Message::SetPref(Pref::EqPreset(0))),
                    ),
                ],
            ]
            .spacing(16),
        )
        .padding(pad4(24.0, 24.0, 20.0, 24.0))
        .width(Length::Fill)
        .style(|_| panel(BG_CARD, 8.0))
        .into()
    }
}

/// Spotify's select: 32px on a light tint, #B3B3B3 text brightening on hover.
fn settings_select<T>(
    options: Vec<T>,
    selected: T,
    on_select: impl Fn(T) -> Message + 'static,
) -> Element<'static, Message>
where
    T: std::fmt::Display + PartialEq + Clone + 'static,
{
    iced::widget::pick_list(options, Some(selected), on_select)
        .width(Length::Fixed(180.0))
        .text_size(14)
        .text_line_height(text::LineHeight::Absolute(iced::Pixels(20.0)))
        .padding(pad4(6.0, 12.0, 6.0, 12.0))
        .handle(iced::widget::pick_list::Handle::Static(
            iced::widget::pick_list::Icon {
                font: FA_SOLID,
                code_point: '\u{f078}',
                size: Some(iced::Pixels(10.0)),
                line_height: text::LineHeight::Absolute(iced::Pixels(20.0)),
                shaping: text::Shaping::Basic,
            },
        ))
        .style(|_, status| {
            let hot = matches!(
                status,
                iced::widget::pick_list::Status::Hovered | iced::widget::pick_list::Status::Opened
            );
            iced::widget::pick_list::Style {
                text_color: if hot { TEXT } else { TEXT_DIM },
                placeholder_color: TEXT_MUTED,
                handle_color: if hot { TEXT } else { TEXT_DIM },
                background: Background::Color(if hot { BG_HOVER } else { BG_TINT }),
                border: round(4.0),
            }
        })
        .menu_style(|_| iced::overlay::menu::Style {
            background: Background::Color(BG_HOVER),
            border: round(4.0),
            text_color: TEXT_DIM,
            selected_text_color: TEXT,
            selected_background: Background::Color(BG_TINT_HI),
        })
        .into()
}

/// The equalizer's graph: each band a point at its gain, joined by lines
/// over a filled area, the points dragged up and down.
struct EqGraph {
    gains: [f32; 6],
    enabled: bool,
}

#[derive(Default)]
struct EqDrag {
    band: Option<usize>,
}

impl EqGraph {
    /// The plot area inside the canvas (the rest holds the labels).
    fn plot(size: Size) -> Rectangle {
        Rectangle {
            x: EQ_LEFT,
            y: EQ_TOP,
            width: (size.width - EQ_LEFT - EQ_RIGHT).max(1.0),
            height: EQ_GRAPH_H,
        }
    }

    fn band_x(plot: Rectangle, i: usize) -> f32 {
        // the bands sit inside the plot, a margin from its edges
        let margin = 24.0;
        plot.x + margin + (plot.width - 2.0 * margin) * i as f32 / (EQ_BANDS.len() - 1) as f32
    }

    fn gain_y(plot: Rectangle, db: f32) -> f32 {
        plot.y + plot.height * (1.0 - (db + EQ_RANGE_DB) / (2.0 * EQ_RANGE_DB))
    }

    fn y_gain(plot: Rectangle, y: f32) -> f32 {
        let t = 1.0 - (y - plot.y) / plot.height;
        (t * 2.0 * EQ_RANGE_DB - EQ_RANGE_DB).clamp(-EQ_RANGE_DB, EQ_RANGE_DB)
    }
}

impl canvas::Program<Message> for EqGraph {
    type State = EqDrag;

    fn update(
        &self,
        state: &mut EqDrag,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        use canvas::event::Status;
        if !self.enabled {
            state.band = None;
            return (Status::Ignored, None);
        }
        let plot = Self::plot(bounds.size());
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_in(bounds) else {
                    return (Status::Ignored, None);
                };
                if p.y < plot.y - 12.0 || p.y > plot.y + plot.height + 12.0 {
                    return (Status::Ignored, None);
                }
                // the band nearest the pointer, its point jumping there
                let band = (0..EQ_BANDS.len())
                    .min_by(|&a, &b| {
                        (Self::band_x(plot, a) - p.x)
                            .abs()
                            .total_cmp(&(Self::band_x(plot, b) - p.x).abs())
                    })
                    .unwrap_or(0);
                state.band = Some(band);
                (
                    Status::Captured,
                    Some(Message::SetPref(Pref::EqBand(
                        band,
                        Self::y_gain(plot, p.y),
                    ))),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => match state.band {
                Some(band) => (
                    Status::Captured,
                    Some(Message::SetPref(Pref::EqBand(
                        band,
                        Self::y_gain(plot, position.y - bounds.y),
                    ))),
                ),
                None => (Status::Ignored, None),
            },
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.band.is_some() =>
            {
                state.band = None;
                (Status::Captured, Some(Message::SavePrefs))
            }
            _ => (Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &EqDrag,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let plot = Self::plot(bounds.size());
        let alpha = if self.enabled { 1.0 } else { 0.4 };
        let grid = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
        let label =
            |content: String, position: Point, align: iced::alignment::Horizontal| canvas::Text {
                content,
                position,
                color: Color {
                    a: alpha,
                    ..TEXT_DIM
                },
                size: iced::Pixels(13.0),
                font: iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..iced::Font::with_name("Segoe UI")
                },
                horizontal_alignment: align,
                vertical_alignment: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            };

        // the grid: a line per band, and 0 dB
        for i in 0..EQ_BANDS.len() {
            let x = Self::band_x(plot, i);
            frame.fill_rectangle(Point::new(x, plot.y), Size::new(1.0, plot.height), grid);
        }
        let zero = Self::gain_y(plot, 0.0);
        frame.fill_rectangle(Point::new(plot.x, zero), Size::new(plot.width, 1.0), grid);

        // labels: the range on the left, the bands under the plot
        frame.fill_text(label(
            format!("+{EQ_RANGE_DB:.0}dB"),
            Point::new(plot.x - 12.0, plot.y),
            iced::alignment::Horizontal::Right,
        ));
        frame.fill_text(label(
            format!("-{EQ_RANGE_DB:.0}dB"),
            Point::new(plot.x - 12.0, plot.y + plot.height),
            iced::alignment::Horizontal::Right,
        ));
        for (i, hz) in EQ_BANDS.iter().enumerate() {
            let name = if *hz >= 1000.0 {
                let k = hz / 1000.0;
                if k.fract() == 0.0 {
                    format!("{k:.0}KHz")
                } else {
                    format!("{k:.1}KHz")
                }
            } else {
                format!("{hz:.0}Hz")
            };
            frame.fill_text(label(
                name,
                Point::new(
                    Self::band_x(plot, i),
                    plot.y + plot.height + EQ_BOTTOM / 2.0 + 4.0,
                ),
                iced::alignment::Horizontal::Center,
            ));
        }

        // the curve over its area, from each band's point
        let points: Vec<Point> = (0..EQ_BANDS.len())
            .map(|i| Point::new(Self::band_x(plot, i), Self::gain_y(plot, self.gains[i])))
            .collect();
        let bottom = plot.y + plot.height;
        let area = canvas::Path::new(|b| {
            b.move_to(Point::new(points[0].x, bottom));
            for p in &points {
                b.line_to(*p);
            }
            b.line_to(Point::new(points[points.len() - 1].x, bottom));
            b.close();
        });
        frame.fill(
            &area,
            canvas::Fill {
                style: canvas::Style::Gradient(canvas::Gradient::Linear(
                    canvas::gradient::Linear::new(Point::new(0.0, plot.y), Point::new(0.0, bottom))
                        .add_stop(
                            0.0,
                            Color {
                                a: 0.75 * alpha,
                                ..ORANGE
                            },
                        )
                        .add_stop(
                            1.0,
                            Color {
                                a: 0.05 * alpha,
                                ..ORANGE
                            },
                        ),
                )),
                ..canvas::Fill::default()
            },
        );
        let line = canvas::Path::new(|b| {
            b.move_to(points[0]);
            for p in &points[1..] {
                b.line_to(*p);
            }
        });
        frame.stroke(
            &line,
            canvas::Stroke::default()
                .with_width(2.0)
                .with_color(Color { a: alpha, ..ORANGE }),
        );
        for (i, p) in points.iter().enumerate() {
            let grabbed = state.band == Some(i);
            let r = if grabbed { 7.0 } else { 5.5 };
            frame.fill(
                &canvas::Path::circle(*p, r),
                Color {
                    a: alpha,
                    ..Color::WHITE
                },
            );
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &EqDrag,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if !self.enabled {
            return mouse::Interaction::default();
        }
        if state.band.is_some() {
            return mouse::Interaction::Grabbing;
        }
        let plot = Self::plot(bounds.size());
        match cursor.position_in(bounds) {
            Some(p) if p.y >= plot.y - 12.0 && p.y <= plot.y + plot.height + 12.0 => {
                mouse::Interaction::Grab
            }
            _ => mouse::Interaction::default(),
        }
    }
}
