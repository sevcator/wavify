//! Unlocking through YouTube Music: a track SoundCloud can't play in full
//! (Go+ without a subscription streams only a 30 s preview, "SNIP"; or it's
//! blocked in this country) is looked up on YouTube Music and played there.
//! A candidate counts only if it's the same recording: the same length
//! (within 2.5 s), every word of the title and the artist in its name, and
//! none of the words that mark a different version (remix, sped up, ...)
//! unless the original has them too.

use crate::api::Track;
use crate::config::Settings;
use anyhow::Result;

/// Words that mark another version of a song.
const OTHER_VERSION: &[&str] = &[
    "remix",
    "edit",
    "sped",
    "speed",
    "slowed",
    "slow",
    "reverb",
    "nightcore",
    "cover",
    "instrumental",
    "karaoke",
    "acapella",
    "cappella",
    "boosted",
    "8d",
    "flip",
    "bootleg",
    "mashup",
    "live",
    "loop",
    "extended",
    "vip",
    "rework",
    "remake",
    "remastered",
    "demo",
    "version",
    "mix",
    "beat",
    "hour",
    "hours",
    "tiktok",
    "tribute",
    "lofi",
    "piano",
    "orchestral",
    "originally",
    "famous",
];

/// Only the 30 s preview is streamable (Go+ without a subscription).
pub fn is_preview_only(t: &Track) -> bool {
    if t.policy.as_deref() == Some("SNIP") {
        return true;
    }
    let snipped = t
        .media
        .as_ref()
        .map(|m| {
            !m.transcodings.is_empty() && m.transcodings.iter().all(|tr| tr.snipped == Some(true))
        })
        .unwrap_or(false);
    snipped
}

/// Lowercase words, punctuation dropped ("God's Plan!" -> ["god", "s", "plan"]).
fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The title without "(feat. X)" / "[prod. Y]" parts, which uploads write
/// in many ways.
fn core_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ if depth <= 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase();
    let cut = [" feat.", " feat ", " ft.", " ft ", " prod."]
        .iter()
        .filter_map(|m| lower.find(m))
        .min()
        .unwrap_or(lower.len());
    lower[..cut].to_string()
}

/// How far `candidate` (its name, with the uploader or artist) is from
/// the original, in ms of length difference; None when it's not the same
/// recording.
fn same_song(
    artist: &str,
    title: &str,
    orig_ms: u64,
    candidate: &str,
    cand_ms: u64,
) -> Option<u64> {
    let diff = orig_ms.abs_diff(cand_ms);
    if orig_ms == 0 || diff > (orig_ms / 66).max(2_500) {
        return None;
    }
    let cand = words(candidate);
    let has = |w: &str| cand.iter().any(|c| c == w);
    // every word of the title
    let title_words = words(&core_title(title));
    if title_words.is_empty() || !title_words.iter().all(|w| has(w)) {
        return None;
    }
    // the artist (their first real word: "The Weeknd" -> "weeknd")
    let artist_words: Vec<String> = words(artist).into_iter().filter(|w| w != "the").collect();
    if let Some(first) = artist_words.first() {
        if !has(first) {
            return None;
        }
    }
    // nothing that marks another version, unless the original says it too
    let orig = words(title);
    if OTHER_VERSION
        .iter()
        .any(|w| has(w) && !orig.iter().any(|o| o == w))
    {
        return None;
    }
    Some(diff)
}

/// The track's artist and title for looking it up elsewhere: the label's
/// artist name beats an account name ("coldplayofficial").
pub fn artist_and_title(track: &Track) -> (String, String) {
    let (shown_artist, title) = track.display_artist_and_title(true);
    let artist = track
        .publisher_metadata
        .as_ref()
        .and_then(|m| m.artist.clone())
        .filter(|a| !a.trim().is_empty())
        .unwrap_or(shown_artist);
    (artist, title)
}

/// A song on YouTube Music: its video id, length, and a label for the user.
#[derive(Debug, Clone)]
pub struct YtSong {
    pub video_id: String,
    pub duration_ms: u64,
    pub label: String,
}

/// The same recording on YouTube Music, found with its song search (the
/// InnerTube API of music.youtube.com, "Songs" filter). Only the search
/// runs here; playback happens in YouTube Music's own web player.
pub async fn find_on_youtube_music(track: &Track) -> Result<YtSong> {
    const SONGS_FILTER: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
    let (artist, title) = artist_and_title(track);
    let orig_ms = track.full_duration.or(track.duration).unwrap_or(0);
    let http = crate::api::build_http(&Settings::load())?;
    let body = serde_json::json!({
        "context": {"client": {"clientName": "WEB_REMIX", "clientVersion": "1.20260707.12.00", "hl": "en", "gl": "US"}},
        "query": format!("{artist} {title}"),
        "params": SONGS_FILTER,
    });
    let v: serde_json::Value = http
        .post("https://music.youtube.com/youtubei/v1/search?prettyPrint=false")
        .header("Origin", "https://music.youtube.com")
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let mut items = Vec::new();
    collect(&v, "musicResponsiveListItemRenderer", &mut items);
    let mut best: Option<(u64, YtSong)> = None;
    for it in items {
        let Some(video_id) = it
            .pointer("/playlistItemData/videoId")
            .and_then(|x| x.as_str())
        else {
            continue;
        };
        // columns: title | "Artist • Album • 3:19" | plays
        let cols: Vec<String> = it
            .get("flexColumns")
            .and_then(|c| c.as_array())
            .map(|cols| {
                cols.iter()
                    .map(|c| {
                        c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                            .and_then(|r| r.as_array())
                            .map(|runs| {
                                runs.iter()
                                    .filter_map(|r| r.get("text")?.as_str())
                                    .collect::<String>()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (Some(name), Some(meta)) = (cols.first(), cols.get(1)) else {
            continue;
        };
        let ms = meta.rsplit('•').next().map(clock_ms).unwrap_or(0);
        if let Some(diff) = same_song(&artist, &title, orig_ms, &format!("{name} {meta}"), ms) {
            if best.as_ref().map_or(true, |(d, _)| diff < *d) {
                let by = meta.split('•').next().unwrap_or("").trim().to_string();
                best = Some((
                    diff,
                    YtSong {
                        video_id: video_id.to_string(),
                        duration_ms: ms,
                        label: format!("YouTube Music · {by}"),
                    },
                ));
            }
        }
    }
    best.map(|(_, s)| s)
        .ok_or_else(|| anyhow::anyhow!("no matching song on YouTube Music"))
}

/// Every object under `key` anywhere in a JSON tree.
fn collect(v: &serde_json::Value, key: &str, out: &mut Vec<serde_json::Value>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                if k == key {
                    out.push(val.clone());
                } else {
                    collect(val, key, out);
                }
            }
        }
        serde_json::Value::Array(list) => list.iter().for_each(|x| collect(x, key, out)),
        _ => {}
    }
}

/// "3:19" / "1:02:03" -> ms (0 when it isn't a clock).
fn clock_ms(s: &str) -> u64 {
    let mut total = 0u64;
    for part in s.trim().split(':') {
        match part.trim().parse::<u64>() {
            Ok(n) => total = total * 60 + n,
            Err(_) => return 0,
        }
    }
    total * 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_song_takes_reuploads_and_skips_other_versions() {
        let orig = 199_000;
        // a reupload named the other way round, 0.4 s longer
        assert!(same_song(
            "Drake",
            "God's Plan",
            orig,
            "God's Plan - Drake Tom van Wijk",
            199_400
        )
        .is_some());
        // other versions and other lengths
        assert!(same_song(
            "Drake",
            "God's Plan",
            orig,
            "Drake - God's Plan (Slow + Reverb)",
            199_000
        )
        .is_none());
        assert!(same_song(
            "Drake",
            "God's Plan",
            orig,
            "Drake - God's Plan remix",
            199_000
        )
        .is_none());
        assert!(same_song(
            "Drake",
            "God's Plan",
            orig,
            "God's Plan - Drake Tribute",
            199_000
        )
        .is_none());
        assert!(same_song("Drake", "God's Plan", orig, "Drake - God's Plan", 223_000).is_none());
        // the artist must be named
        assert!(same_song(
            "Drake",
            "God's Plan",
            orig,
            "God's Plan - Somebody",
            199_000
        )
        .is_none());
        // "feat." parts don't have to match
        assert!(same_song(
            "Future",
            "Life Is Good (feat. Drake)",
            238_000,
            "Future - Life Is Good",
            238_500
        )
        .is_some());
        // a word the original has is fine ("Remix" of a remix)
        assert!(same_song(
            "X",
            "Song (Y Remix)",
            200_000,
            "X - Song (Y Remix)",
            200_000
        )
        .is_some());
    }

    #[test]
    fn clock_lengths_parse() {
        assert_eq!(clock_ms(" 3:19"), 199_000);
        assert_eq!(clock_ms("1:02:03"), 3_723_000);
        assert_eq!(clock_ms("2.4B plays"), 0);
    }
}
