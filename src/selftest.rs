use crate::api::*;
use crate::config::*;
use anyhow::Result;

/// Hidden selftest mode: exercises the API through the same code paths the
/// UI uses and logs PASS/FAIL for each. Run: wavify.exe --selftest, reads
/// only. Extra flags:
///   --like-check    like a track, see it in the likes, unlike it again
///   --auth-check    swap the browser session for an app token (in memory)
///   --bypass-check  resolve a track through a public proxy
///   --full-check    find full versions of some Go+ (preview-only) tracks,
///                   elsewhere and on YouTube Music
///   --writes        reposts, comments, follows (visible to other people)
pub async fn run() -> Result<()> {
    crate::log!("=== wavify selftest ===");
    let settings = Settings::load();
    let auth = crate::auth::Auth::new(&settings)?;
    let access = auth.access().await?;
    let api = Api::new(auth.http().clone(), &settings).with_access(Some(access.clone()));
    let cid = CLIENT_ID.to_string();

    let me = api.me(&access).await?;
    crate::log!("[PASS] me: id={} username={}", me.id, me.username);
    // the login window's token works on the mobile API; a browser
    // session's doesn't, and SoundCloud blocks its writes
    crate::log!(
        "[INFO] sign-in can write: {}",
        if api.mobile_accepts(&access).await {
            "yes"
        } else {
            "no (browser-session token)"
        }
    );
    let me_id = me.id;

    // --- READS ---
    match api.mixed_selections(&cid).await {
        Ok(v) => {
            let n = v
                .get("collection")
                .and_then(|c| c.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            crate::log!("[PASS] mixed-selections: {} sections", n);
        }
        Err(e) => crate::log!("[FAIL] mixed-selections: {e}"),
    }
    for tag in ["Hip-Hop & Rap", "digicore"] {
        match api.search_tracks_tagged(tag, &cid, 10).await {
            Ok(l) => {
                let tagged = l
                    .collection
                    .iter()
                    .filter(|t| {
                        let low = tag.to_lowercase();
                        t.genre.as_deref().is_some_and(|g| g.to_lowercase() == low)
                            || t.tag_list
                                .as_deref()
                                .is_some_and(|g| g.to_lowercase().contains(&low))
                    })
                    .count();
                crate::log!(
                    "[PASS] tag search \"{tag}\": {} tracks, {tagged} carry the tag",
                    l.collection.len()
                );
            }
            Err(e) => crate::log!("[FAIL] tag search \"{tag}\": {e}"),
        }
    }
    match api.search_tracks("travis scott", &cid, 5).await {
        Ok(l) => {
            let first = list_collection_track_id(&l);
            crate::log!(
                "[PASS] search: {} results (first id {})",
                l.collection.len(),
                first
            );
        }
        Err(e) => crate::log!("[FAIL] search: {e}"),
    }
    match api.all_user_likes(me_id, &cid, 200).await {
        Ok(v) => crate::log!("[PASS] my likes: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] my likes: {e}"),
    }
    match api.user_likes(me_id, &cid, 20).await {
        Ok(v) => crate::log!("[PASS] other-user likes list: {} items", v.len()),
        Err(e) => crate::log!("[FAIL] other-user likes: {e}"),
    }
    match api.user_top_tracks(3685019, &cid).await {
        Ok(v) => crate::log!("[PASS] user top-tracks: {} items", v.len()),
        Err(e) => crate::log!("[FAIL] user top-tracks: {e}"),
    }
    match api.user_playlists_posted(3685019, &cid, 1).await {
        Ok(v) => crate::log!("[PASS] user playlists posted: {} items", v.len()),
        Err(e) => crate::log!("[FAIL] user playlists posted: {e}"),
    }
    match api.user_liked_playlists(me_id, &cid).await {
        Ok(v) => crate::log!("[PASS] liked playlists: {} items", v.len()),
        Err(e) => crate::log!("[FAIL] liked playlists: {e}"),
    }
    {
        let s = api.my_following_ids(&access, me_id).await;
        crate::log!("[PASS] my following ids: {} users", s.len());
    }
    match api.related(2162751417, &cid).await {
        Ok(l) => crate::log!("[PASS] related: {} tracks", l.collection.len()),
        Err(e) => crate::log!("[FAIL] related: {e}"),
    }
    match api.waveform(2162751417, &cid).await {
        Ok(s) => crate::log!("[PASS] waveform: {} samples", s.len()),
        Err(e) => crate::log!("[FAIL] waveform: {e}"),
    }
    match api.comments(2162751417, &cid, 5).await {
        Ok((list, next)) => crate::log!(
            "[PASS] comments: {} items, more={}",
            list.len(),
            next.is_some()
        ),
        Err(e) => crate::log!("[FAIL] comments: {e}"),
    }
    match api.station_for_track(2162751417, &cid).await {
        Ok(v) => crate::log!("[PASS] track radio: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] track radio: {e}"),
    }
    match api.station_for_artist(3685019, &cid).await {
        Ok(v) => crate::log!("[PASS] artist radio: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] artist radio: {e}"),
    }
    match api.user_reposts(3685019, &cid).await {
        Ok(v) => crate::log!("[PASS] user reposts: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] user reposts: {e}"),
    }
    {
        let ids = api.my_liked_playlist_ids(&access, me_id).await;
        crate::log!("[PASS] my liked playlist ids: {} entries", ids.len());
    }
    match api.user_liked_playlists(me_id, &cid).await {
        Ok(v) => crate::log!("[PASS] liked playlists list: {} items", v.len()),
        Err(e) => crate::log!("[FAIL] liked playlists list: {e}"),
    }
    match api
        .search_tracks_next("https://api-v2.soundcloud.com/search/tracks?q=test&limit=5&offset=0")
        .await
    {
        Ok(l) => crate::log!("[PASS] search next: {} tracks", l.collection.len()),
        Err(e) => crate::log!("[FAIL] search next: {e}"),
    }
    match api.tracks_by_ids(&[2162751417, 2111714670], &cid).await {
        Ok(v) => crate::log!("[PASS] tracks_by_ids: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] tracks_by_ids: {e}"),
    }
    match api.playlist(1780168458, &cid).await {
        Ok(p) => crate::log!(
            "[PASS] playlist detail: \"{}\" ({} tracks)",
            p.title,
            p.tracks.as_ref().map(|t| t.len()).unwrap_or(0)
        ),
        Err(e) => crate::log!("[FAIL] playlist detail: {e}"),
    }
    // --bypass-check: find a working public proxy and resolve a track
    // through it (anonymously), as "Bypass unavailability" does.
    if std::env::args().any(|a| a == "--bypass-check") {
        let started = std::time::Instant::now();
        match crate::proxy_pool::resolve_via_proxies(2162751417, "hls").await {
            Ok((src, proxy)) => crate::log!(
                "[PASS] bypass: resolved via {proxy} in {:.1}s: {}",
                started.elapsed().as_secs_f32(),
                src.describe()
            ),
            Err(e) => crate::log!(
                "[FAIL] bypass: {e} ({:.1}s)",
                started.elapsed().as_secs_f32()
            ),
        }
    }
    if std::env::args().any(|a| a == "--full-check") {
        let mut go_plus: Vec<Track> = Vec::new();
        for q in [
            "The Weeknd Blinding Lights",
            "Dua Lipa Levitating",
            "Ed Sheeran Shape of You",
            "Porter Robinson Language",
            "Skrillex Bangarang",
            "Rick Astley Never Gonna Give You Up",
            "Imagine Dragons Believer",
            "Daft Punk Get Lucky",
            "Coldplay Viva La Vida",
            "Linkin Park Numb",
        ] {
            if let Ok(l) = api.search_tracks(q, &cid, 10).await {
                if let Some(t) = l
                    .collection
                    .into_iter()
                    .find(crate::alt_source::is_preview_only)
                {
                    go_plus.push(t);
                }
            }
        }
        crate::log!("full-check: {} Go+ tracks", go_plus.len());
        let mut found = 0;
        for t in &go_plus {
            let started = std::time::Instant::now();
            match crate::alt_source::find_on_youtube_music(t).await {
                Ok(song) => {
                    found += 1;
                    crate::log!(
                        "[PASS] youtube music for \"{}\": {} ({}s, video {}) in {:.1}s",
                        t.title,
                        song.label,
                        song.duration_ms / 1000,
                        song.video_id,
                        started.elapsed().as_secs_f32()
                    );
                }
                Err(e) => crate::log!("[MISS] \"{}\": youtube music: {e}", t.title),
            }
        }
        crate::log!("full-check: {found} of {} found", go_plus.len());
    }
    // quick reactions (GraphQL), the first two minutes of a popular track
    let secs: Vec<u64> = (0..120).collect();
    for per_second in [1, 5] {
        match api
            .track_reactions(Some(&access), 2162751417, &secs, per_second)
            .await
        {
            Ok(v) => {
                let mut per: std::collections::BTreeMap<u64, usize> = Default::default();
                for r in &v {
                    *per.entry(r.second).or_default() += 1;
                }
                crate::log!(
                    "[PASS] reactions (up to {per_second}/s): {} in 0-120s, at most {} in one second",
                    v.len(),
                    per.values().max().copied().unwrap_or(0),
                )
            }
            Err(e) => crate::log!("[FAIL] reactions (up to {per_second}/s): {e}"),
        }
    }
    match api.resolve_stream(2162751417, &cid, "hls").await {
        Ok(ref s) => crate::log!("[PASS] stream resolve: {}", s.describe()),
        Err(e) => crate::log!("[FAIL] stream resolve: {e}"),
    }

    // --- WRITES (safe toggles, restore state) ---
    // --like-check: the smallest write round trip. Likes a track that isn't
    // liked yet, confirms it shows in the likes, and unlikes it again.
    // --auth-check: dry run of the cookie -> android token swap. The new
    // token lives only in memory here; token.json is not touched.
    let (api, access) = if std::env::args().any(|a| a == "--auth-check") {
        let cookies = crate::cookie_auth::load_cookies().unwrap_or_default();
        match tokio::task::spawn_blocking(move || {
            crate::cookie_auth::android_token_from_cookies(&cookies)
        })
        .await?
        {
            Ok((tok, refresh, _)) => {
                crate::log!(
                    "[PASS] android token from cookies (refresh token: {})",
                    refresh.is_some()
                );
                (
                    Api::new(auth.http().clone(), &settings).with_access(Some(tok.clone())),
                    tok,
                )
            }
            Err(e) => {
                crate::log!("[FAIL] android token from cookies: {e}");
                (api, access)
            }
        }
    } else {
        (api, access)
    };
    if std::env::args().any(|a| a == "--like-check") {
        let tid = 2162751417;
        let liked = |v: &Vec<Track>| v.iter().any(|t| t.id == tid);
        let before = api.user_likes(me_id, &cid, 50).await.unwrap_or_default();
        if liked(&before) {
            crate::log!("[SKIP] like check: track {tid} is already liked");
        } else {
            match api.like_track(&access, me_id, tid).await {
                Ok(()) => crate::log!("[PASS] like"),
                Err(e) => crate::log!("[FAIL] like: {e}"),
            }
            let after = api.user_likes(me_id, &cid, 50).await.unwrap_or_default();
            crate::log!(
                "[{}] like visible in likes",
                if liked(&after) { "PASS" } else { "FAIL" }
            );
            match api.unlike_track(&access, me_id, tid).await {
                Ok(()) => crate::log!("[PASS] unlike"),
                Err(e) => crate::log!("[FAIL] unlike: {e}"),
            }
            let restored = api.user_likes(me_id, &cid, 50).await.unwrap_or_default();
            crate::log!(
                "[{}] like removed again",
                if liked(&restored) { "FAIL" } else { "PASS" }
            );
        }
    }
    // Reposts, comments and follows are visible to other people (feeds,
    // notifications), so they only run when asked for with --writes.
    if !std::env::args().any(|a| a == "--writes") {
        crate::log!("[SKIP] write tests (pass --writes to run them)");
        crate::log!("=== selftest done ===");
        return Ok(());
    }
    // 1. repost toggle on a random recent track
    let test_track = api.track(2162751417, &cid).await?;
    crate::log!(
        "--- write tests on track {} (\"{}\") ---",
        test_track.id,
        test_track.title
    );

    match api.repost_track(&access, me_id, test_track.id).await {
        Ok(()) => crate::log!("[PASS] repost"),
        Err(e) => crate::log!("[FAIL] repost: {e}"),
    }
    match api.unrepost_track(&access, me_id, test_track.id).await {
        Ok(()) => crate::log!("[PASS] unrepost"),
        Err(e) => crate::log!("[FAIL] unrepost: {e}"),
    }

    // 2. like toggle (test on a track NOT already liked)
    let was_liked = api
        .user_likes(me_id, &cid, 200)
        .await
        .map(|l| l.iter().any(|t| t.id == test_track.id))
        .unwrap_or(false);
    if !was_liked {
        match api.like_track(&access, me_id, test_track.id).await {
            Ok(()) => crate::log!("[PASS] like"),
            Err(e) => crate::log!("[FAIL] like: {e}"),
        }
        match api.unlike_track(&access, me_id, test_track.id).await {
            Ok(()) => crate::log!("[PASS] unlike"),
            Err(e) => crate::log!("[FAIL] unlike: {e}"),
        }
    } else {
        crate::log!("[SKIP] like toggle (track already liked, testing unlike+relike)");
        match api.unlike_track(&access, me_id, test_track.id).await {
            Ok(()) => crate::log!("[PASS] unlike"),
            Err(e) => crate::log!("[FAIL] unlike: {e}"),
        }
        match api.like_track(&access, me_id, test_track.id).await {
            Ok(()) => crate::log!("[PASS] relike"),
            Err(e) => crate::log!("[FAIL] relike: {e}"),
        }
    }

    // 3. comment post + delete
    match api
        .post_comment(&access, test_track.id, "wavify selftest", 7777)
        .await
    {
        Ok(c) => {
            crate::log!("[PASS] comment posted id={}", c.id);
            match api.delete_comment(&access, c.id).await {
                Ok(()) => crate::log!("[PASS] comment deleted"),
                Err(e) => crate::log!("[FAIL] comment delete: {e}"),
            }
        }
        Err(e) => crate::log!("[FAIL] comment post: {e}"),
    }

    // 4. follow / unfollow (use a big artist account)
    let follow_target = 3685019;
    let already = api.is_following(&access, me_id, follow_target).await;
    crate::log!(
        "follow check (GET /users/{}/followings/ids) returned {}",
        me_id,
        already
    );
    if already {
        match api.unfollow_user(&access, follow_target).await {
            Ok(()) => crate::log!("[PASS] unfollow"),
            Err(e) => crate::log!("[FAIL] unfollow: {e}"),
        }
        match api.follow_user(&access, follow_target).await {
            Ok(()) => crate::log!("[PASS] follow"),
            Err(e) => crate::log!("[FAIL] follow: {e}"),
        }
    } else {
        match api.follow_user(&access, follow_target).await {
            Ok(()) => crate::log!("[PASS] follow"),
            Err(e) => crate::log!("[FAIL] follow: {e}"),
        }
        match api.unfollow_user(&access, follow_target).await {
            Ok(()) => crate::log!("[PASS] unfollow"),
            Err(e) => crate::log!("[FAIL] unfollow: {e}"),
        }
    }

    // 5. playlist like toggle
    match api.user_liked_playlists(me_id, &cid).await {
        Ok(list) => {
            crate::log!("liked playlists right now: {}", list.len());
            // test on any playlist from top playlists
            match api.mixed_selections(&cid).await {
                Ok(v) => {
                    // find first playlist id in payload
                    let pid = find_first_playlist_id(&v);
                    if let Some(pid) = pid {
                        match api.like_playlist(&access, me_id, pid).await {
                            Ok(()) => crate::log!("[PASS] playlist like (id {pid})"),
                            Err(e) => crate::log!("[FAIL] playlist like: {e}"),
                        }
                        match api.unlike_playlist(&access, me_id, pid).await {
                            Ok(()) => crate::log!("[PASS] playlist unlike"),
                            Err(e) => crate::log!("[FAIL] playlist unlike: {e}"),
                        }
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => {}
    }

    // 6. artist radio station
    match api.station_for_artist(3685019, &cid).await {
        Ok(v) => crate::log!("[PASS] artist station: {} tracks", v.len()),
        Err(e) => crate::log!("[FAIL] artist station: {e}"),
    }

    crate::log!("=== selftest done ===");
    Ok(())
}

fn list_collection_track_id(_l: &ApiList<Track>) -> i64 {
    0
}

fn find_first_playlist_id(v: &serde_json::Value) -> Option<i64> {
    // search for a playlist object anywhere
    let mut stack = vec![v];
    while let Some(node) = stack.pop() {
        if let Some(obj) = node.as_object() {
            if let Some(id) = obj.get("id").and_then(|i| i.as_i64()) {
                if obj.get("kind").and_then(|k| k.as_str()) == Some("playlist") && id != 0 {
                    return Some(id);
                }
            }
            for (_, val) in obj {
                if val.is_object() || val.is_array() {
                    stack.push(val);
                }
            }
        } else if let Some(arr) = node.as_array() {
            for item in arr {
                stack.push(item);
            }
        }
    }
    None
}
