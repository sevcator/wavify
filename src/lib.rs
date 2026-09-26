pub mod alt_source;
pub mod api;
pub mod auth;
pub mod captcha;
pub mod config;
pub mod console;
pub mod cookie_auth;
pub mod discord_rpc;
pub mod proxy_pool;
pub mod sc_web;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_and_history_paths() {
        let cache_dir = config::audio_cache_dir();
        assert!(cache_dir.to_string_lossy().contains("cache"));
        let cached_path = config::cached_audio_path(987654);
        assert!(cached_path.to_string_lossy().ends_with("987654.mp3"));

        let hist = config::history_path();
        assert!(hist.to_string_lossy().ends_with("history.json"));
    }

    #[test]
    fn settings_migrate_old_artist_preference_key() {
        let settings: config::Settings =
            serde_json::from_str(r#"{"prefer_artist_from_name":true}"#).unwrap();
        assert!(settings.prefer_artist_from_metadata);

        let saved = serde_json::to_value(settings).unwrap();
        assert_eq!(saved["prefer_artist_from_metadata"], true);
        assert!(saved.get("prefer_artist_from_name").is_none());
    }

    #[test]
    fn test_track_serialization_roundtrip() {
        let track = api::Track {
            id: 123456,
            title: "Test Track Title".to_string(),
            duration: Some(180000),
            user: Some(api::UserMini {
                id: 42,
                username: "Artist".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&track).expect("serialize track");
        let deserialized: api::Track = serde_json::from_str(&json).expect("deserialize track");
        assert_eq!(deserialized.id, 123456);
        assert_eq!(deserialized.title, "Test Track Title");
        assert_eq!(deserialized.duration, Some(180000));
        assert_eq!(deserialized.user.as_ref().unwrap().username, "Artist");
    }

    #[test]
    fn test_artist_preference_uses_publisher_metadata_then_uploader() {
        let track = api::Track {
            title: "Track title - not an artist source".into(),
            user: Some(api::UserMini {
                username: "лоло".into(),
                ..Default::default()
            }),
            publisher_metadata: Some(api::PublisherMetadata {
                artist: Some("лоло, #ребенокискусства".into()),
            }),
            ..Default::default()
        };

        assert_eq!(
            track.display_artist_and_title(true),
            (
                "лоло, #ребенокискусства".into(),
                "Track title - not an artist source".into()
            )
        );
        assert_eq!(
            track.display_artist_and_title(false),
            ("лоло".into(), "Track title - not an artist source".into())
        );

        let no_metadata = api::Track {
            publisher_metadata: None,
            ..track
        };
        assert_eq!(
            no_metadata.display_artist_and_title(true),
            ("лоло".into(), "Track title - not an artist source".into())
        );
    }

    #[test]
    fn test_prefer_artist_from_track_name() {
        use api::parse_artist_and_title;

        // Case 1: "angelgrind - кроме тебя" (Artist - Title) with uploader angelgrind
        let (artist1, title1) =
            parse_artist_and_title("angelgrind - кроме тебя", "angelgrind", "angelgrind", true);
        assert_eq!(artist1, "angelgrind");
        assert_eq!(title1, "кроме тебя");

        // Case 2: "кроме тебя - angelgrind" (Title - Artist) with uploader angelgrind
        let (artist2, title2) =
            parse_artist_and_title("кроме тебя - angelgrind", "angelgrind", "angelgrind", true);
        assert_eq!(artist2, "angelgrind");
        assert_eq!(title2, "кроме тебя");

        // Case 3: "кроме тебя - angelgrind" with case-insensitive uploader "ANGELGRIND"
        let (artist3, title3) =
            parse_artist_and_title("кроме тебя - angelgrind", "ANGELGRIND", "angelgrind", true);
        assert_eq!(artist3, "angelgrind");
        assert_eq!(title3, "кроме тебя");

        // Case 4: Third party uploader (e.g. promo channel) with "Artist - Title"
        let (artist4, title4) =
            parse_artist_and_title("Alan Walker - Faded", "Trap City", "trapcity", true);
        assert_eq!(artist4, "Alan Walker");
        assert_eq!(title4, "Faded");

        // Case 5: Hyphen without spaces (e.g. "post-punk anthem") should NOT split
        let (artist5, title5) =
            parse_artist_and_title("post-punk anthem", "TheBand", "theband", true);
        assert_eq!(artist5, "TheBand");
        assert_eq!(title5, "post-punk anthem");

        // Case 6: prefer_from_name = false should always return uploader as artist and original title
        let (artist6, title6) =
            parse_artist_and_title("angelgrind - кроме тебя", "angelgrind", "angelgrind", false);
        assert_eq!(artist6, "angelgrind");
        assert_eq!(title6, "angelgrind - кроме тебя");

        // Case 7: prefer_from_name = false with Title - Artist
        let (artist7, title7) =
            parse_artist_and_title("кроме тебя - angelgrind", "angelgrind", "angelgrind", false);
        assert_eq!(artist7, "angelgrind");
        assert_eq!(title7, "кроме тебя - angelgrind");
    }

    #[test]
    fn test_speed_cycling() {
        let speeds = [1.0f32, 1.15, 1.25, 1.5, 2.0, 0.75];
        let cycle = |current: f32| -> f32 {
            speeds
                .iter()
                .position(|&s| (s - current).abs() < 0.05)
                .map(|idx| speeds[(idx + 1) % speeds.len()])
                .unwrap_or(1.0)
        };

        assert_eq!(cycle(1.0), 1.15);
        assert_eq!(cycle(1.15), 1.25);
        assert_eq!(cycle(1.25), 1.5);
        assert_eq!(cycle(1.5), 2.0);
        assert_eq!(cycle(2.0), 0.75);
        assert_eq!(cycle(0.75), 1.0);
        assert_eq!(cycle(3.5), 1.0); // unknown falls back to 1.0
    }

    #[test]
    fn test_download_filename_sanitization() {
        let artist = "Artist/Slash:Colon";
        let title = "Title?Star*Quote\"Pipe|End";
        let safe_artist = artist.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let safe_title = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let filename = format!("{} - {}.mp3", safe_artist.trim(), safe_title.trim());
        assert_eq!(
            filename,
            "Artist_Slash_Colon - Title_Star_Quote_Pipe_End.mp3"
        );
    }
}
