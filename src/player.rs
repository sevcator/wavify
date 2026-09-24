use anyhow::Result;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub enum PlayerCommand {
    /// `proxy`: fetch the audio through this proxy (a track resolved via
    /// "bypass unavailability" must come through the same one).
    PlaySingle {
        track_id: Option<i64>,
        url: String,
        proxy: Option<String>,
    },
    PlayChunks {
        track_id: Option<i64>,
        init: Option<String>,
        chunks: Vec<String>,
        proxy: Option<String>,
    },
    PlayCached {
        track_id: i64,
        path: std::path::PathBuf,
    },
    Stop,
    SetVolume(f32),
    SetSpeed(f32),
    Pause,
    Resume,
    SeekMs(u64),
    /// Crossfade: the playing track fades out over this many ms under the
    /// next one, which fades in (instead of a Stop before it).
    FadeOut(u64),
}

impl PlayerHandle {
    /// Send a command; while the track comes from YouTube Music it's
    /// mirrored to that player too (see yt_music::forward).
    pub fn send(&self, cmd: PlayerCommand) {
        crate::yt_music::forward(&cmd);
        let _ = self.tx.send(cmd);
    }
}

#[derive(Clone)]
pub struct PlayerHandle {
    pub tx: mpsc::UnboundedSender<PlayerCommand>,
    /// Current playback position, milliseconds (updated by watcher thread)
    pub pos_ms: Arc<AtomicU64>,
    /// Set to true when the current track finished playing naturally
    pub ended: Arc<AtomicBool>,
    /// Equalizer, normalization and mono, applied while tracks play.
    pub dsp: Arc<crate::dsp::DspParams>,
    /// Settings > Storage > Cache tracks locally: a streamed track is kept
    /// on disk once complete, and a kept one plays from there. Off, every
    /// track streams.
    pub cache_audio: Arc<AtomicBool>,
}

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// A growing in-memory pipe: the loader thread downloads chunks and appends,
/// the decoder reads from the front. Playback starts as soon as the first
/// chunk arrives, and the rest streams in the background.
struct PipeSource {
    buf: Arc<Mutex<Vec<u8>>>,
    pos: usize,
    /// Set when the loader finished (no more data will arrive).
    done: Arc<AtomicBool>,
    /// The player's generation counter, and its value for this pipe's track.
    generation: Arc<AtomicU64>,
    my_gen: u64,
}

impl PipeSource {
    fn available(&self) -> usize {
        self.buf.lock().map(|b| b.len()).unwrap_or(0)
    }

    /// A newer Play or Stop has taken over, so nothing more is coming for
    /// this pipe, whatever its loader is still doing.
    fn is_stale(&self) -> bool {
        self.generation.load(Ordering::Relaxed) != self.my_gen
    }
}

impl Read for PipeSource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let avail = self.available();
            if self.pos < avail {
                let buf = self.buf.lock().unwrap();
                let n = (avail - self.pos).min(out.len());
                out[..n].copy_from_slice(&buf[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            // A stale pipe ends where its data does (a track fading out
            // under the next one plays what it has). This runs inside the
            // audio callback, so a pipe left waiting for data that never
            // comes (its loader gave up) would silence all output for good.
            if self.done.load(Ordering::Relaxed) || self.is_stale() {
                return Ok(0);
            }
            // wait for more data
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

impl Seek for PipeSource {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        let target = match p {
            SeekFrom::Start(o) => o as i64,
            SeekFrom::End(o) => self.available() as i64 + o,
            SeekFrom::Current(o) => self.pos as i64 + o,
        };
        // A seek while the download runs works like one in a file whose tail
        // is still being written. Going back is into data already here: MP3's
        // seek rewinds to the first frame, then reads forward to the target
        // (refusing that made every backward seek fail until the whole track
        // was in). Going past the data waits for it, as a read does (symphonia
        // skips big blocks by seeking and doesn't check where it landed). The
        // end isn't known yet: that keeps the position, and says so.
        if !self.done.load(Ordering::Relaxed) {
            if matches!(p, SeekFrom::End(_)) {
                return Ok(self.pos as u64);
            }
            while target > self.available() as i64
                && !self.done.load(Ordering::Relaxed)
                && !self.is_stale()
            {
                std::thread::sleep(Duration::from_millis(15));
            }
        }
        self.pos = target.clamp(0, self.available() as i64) as usize;
        Ok(self.pos as u64)
    }
}

/// The audio client: through `proxy` when given (a bypass), else through
/// the proxy from Settings, if any.
fn blocking_client(proxy: Option<&str>) -> reqwest::blocking::Client {
    let settings = crate::config::Settings::load();
    let mut b = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10));
    let proxy = proxy
        .or(settings.proxy.as_deref())
        .filter(|p| !p.is_empty());
    if let Some(p) = proxy.and_then(|p| reqwest::Proxy::all(p).ok()) {
        b = b.proxy(p);
    }
    b.build().unwrap_or_default()
}

/// Spawn a thread that downloads all sources sequentially into the pipe.
fn start_loader(
    keep: Arc<AtomicBool>,
    client: reqwest::blocking::Client,
    sources: Vec<String>,
    buf: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
    track_id: Option<i64>,
    generation: Arc<AtomicU64>,
    expected_gen: u64,
) {
    std::thread::spawn(move || {
        let total = sources.len();
        // Cleared when a part is lost for good: playback goes on without it,
        // but such a file must not become the track's permanent disk cache.
        let mut all_ok = true;
        for (i, url) in sources.iter().enumerate() {
            if done.load(Ordering::Relaxed) || generation.load(Ordering::Relaxed) != expected_gen {
                done.store(true, Ordering::Relaxed); // no reader may wait on us
                return; // playback switched to another track
            }
            let mut downloaded = false;
            for attempt in 0..3 {
                if done.load(Ordering::Relaxed)
                    || generation.load(Ordering::Relaxed) != expected_gen
                {
                    done.store(true, Ordering::Relaxed);
                    return;
                }
                match client.get(url).send() {
                    // An error page (403 on an expired URL, 5xx) is not audio:
                    // retry instead of appending it to the stream.
                    Ok(resp) if !resp.status().is_success() => {
                        crate::log!(
                            "player: chunk {}/{} fetch attempt {} FAILED: HTTP {}",
                            i + 1,
                            total,
                            attempt + 1,
                            resp.status()
                        );
                    }
                    Ok(mut resp) => {
                        let mut chunk_data = Vec::new();
                        use std::io::Read as _;
                        if resp.read_to_end(&mut chunk_data).is_ok() {
                            if let Ok(mut b) = buf.lock() {
                                b.extend_from_slice(&chunk_data);
                            }
                            downloaded = true;
                            crate::log!(
                                "player: chunk {}/{} downloaded ({} bytes, buffer {})",
                                i + 1,
                                total,
                                chunk_data.len(),
                                buf.lock().map(|b| b.len()).unwrap_or(0)
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        crate::log!(
                            "player: chunk {}/{} fetch attempt {} FAILED: {e}",
                            i + 1,
                            total,
                            attempt + 1
                        );
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !downloaded {
                all_ok = false;
                crate::log!(
                    "player: chunk {}/{} completely failed after 3 attempts",
                    i + 1,
                    total
                );
            }
        }
        done.store(true, Ordering::Relaxed);
        crate::log!("player: all chunks loaded");

        // Save complete audio to disk cache for instantaneous future replays
        if let Some(tid) = track_id.filter(|_| keep.load(Ordering::Relaxed)) {
            if all_ok && generation.load(Ordering::Relaxed) == expected_gen {
                let cache_path = crate::config::cached_audio_path(tid);
                if let Ok(b) = buf.lock() {
                    if b.len() > 10000 && !cache_path.exists() {
                        let tmp = cache_path.with_extension("tmp");
                        if std::fs::write(&tmp, &*b).is_ok() {
                            let _ = std::fs::rename(tmp, &cache_path);
                            crate::log!(
                                "player: cached {} bytes to {}",
                                b.len(),
                                cache_path.display()
                            );
                        }
                    }
                }
            }
        }
    });
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PlaybackState {
    Idle,
    Loading(u64),
    Playing(u64),
    Paused(u64),
}

/// Wraps a decoder with a track-time clock: the position is read on the
/// decoder, before the sink's speed stage, so it stays in track time at any
/// speed. (The sink's own get_pos counts played-out time, i.e. track time
/// divided by the speed: at 0.5x the playhead would crawl, at 2x it would stop
/// halfway.) Only the current generation writes the shared position, so a
/// stopped track's last callback can't overwrite the new one's.
///
/// It also runs the track's pending seek (SeekMs), here on the audio thread
/// and in track time. Sink::try_seek made the command loop wait until the
/// seek was done: on a stream, until the download reached the target, and
/// forever once the output device was gone.
fn with_track_clock<S>(
    src: S,
    pos_ms: &Arc<AtomicU64>,
    generation: &Arc<AtomicU64>,
    my_gen: u64,
    seek: &SeekSlot,
) -> impl Source<Item = S::Item> + Send + 'static
where
    S: Source + Send + 'static,
    S::Item: rodio::Sample + Send,
{
    let pos_ms = pos_ms.clone();
    let generation = generation.clone();
    let seek = seek.clone();
    // 5ms, like the sink's own controls: a seek made while paused is applied
    // as soon as playback resumes.
    src.track_position()
        .periodic_access(Duration::from_millis(5), move |tp| {
            // take() and unlock before seeking, which may wait for data
            let target = seek.try_lock().ok().and_then(|mut s| s.take());
            if let Some(to) = target {
                if let Err(e) = tp.try_seek(to) {
                    crate::log!("player: seek FAILED: {e}");
                }
            }
            if generation.load(Ordering::Relaxed) == my_gen {
                pos_ms.store(tp.get_pos().as_millis() as u64, Ordering::Relaxed);
            }
        })
}

/// A track's pending seek target, taken by its track clock.
type SeekSlot = Arc<Mutex<Option<Duration>>>;

/// What the command loop shares with the watcher and the decoder threads.
#[derive(Clone)]
struct Shared {
    pos_ms: Arc<AtomicU64>,
    ended: Arc<AtomicBool>,
    state: Arc<Mutex<PlaybackState>>,
    // generation counter: bumped on every new track, old loaders check it and abort
    generation: Arc<AtomicU64>,
    /// The current track's sink (None when stopped).
    sink: Arc<Mutex<Option<Arc<Sink>>>>,
    dsp: Arc<crate::dsp::DspParams>,
    cache_audio: Arc<AtomicBool>,
}

impl Shared {
    fn current_sink(&self) -> Option<Arc<Sink>> {
        self.sink.lock().unwrap().clone()
    }
}

/// A started track: its generation, a sink of its own, its pending seek,
/// and its fade-in when it crossfades in.
struct Playback {
    gen: u64,
    sink: Arc<Sink>,
    seek: SeekSlot,
    fade_in: Option<Duration>,
}

/// The audio output. A WASAPI stream whose device goes away (headphones
/// switched off, DAC unplugged) just stops pulling audio, and rodio only
/// prints the error, so before each new track the default device's name and
/// a heartbeat from the mixer are checked, and a lost output is reopened.
struct Output {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    /// The default output device's name when the stream was opened.
    device: Option<String>,
    opened: Instant,
    /// When the device last pulled audio, in ms since `opened`.
    beat: Arc<AtomicU64>,
}

impl Output {
    fn open() -> Option<Output> {
        let device = default_output_name();
        let (stream, handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                crate::log!("player: OutputStream::try_default FAILED ({e})! No default audio output device available.");
                return None;
            }
        };
        let opened = Instant::now();
        let beat = Arc::new(AtomicU64::new(0));
        let b = beat.clone();
        // A silent source that stays in the mixer: it ticks whenever the
        // device pulls audio, playing or not.
        let _ = handle.play_raw(
            rodio::source::Zero::<f32>::new(2, 48_000)
                .periodic_access(Duration::from_millis(100), move |_| {
                    b.store(opened.elapsed().as_millis() as u64, Ordering::Relaxed)
                }),
        );
        crate::log!(
            "player: audio output opened ({})",
            device.as_deref().unwrap_or("unknown device")
        );
        Some(Output {
            _stream: stream,
            handle,
            device,
            opened,
            beat,
        })
    }

    /// The default device changed or went away, or the output hasn't pulled
    /// audio for 2s. (A stream stuck on a slow download stalls the heartbeat
    /// too; reopening it then is harmless, as it only happens between tracks.)
    fn is_lost(&self) -> bool {
        let now = self.opened.elapsed().as_millis() as u64;
        now.saturating_sub(self.beat.load(Ordering::Relaxed)) > 2000
            || default_output_name() != self.device
    }
}

fn default_output_name() -> Option<String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}

/// Start a new track: bump the generation (old loaders, pipes and decoder
/// threads see it and stop), drop the previous track's sink, and give the new
/// track a sink of its own, on a live output.
fn begin_track(
    sh: &Shared,
    output: &mut Option<Output>,
    amplitude: f32,
    speed: f32,
    fade_in: Option<Duration>,
) -> Option<Playback> {
    let gen = sh.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *sh.state.lock().unwrap() = PlaybackState::Loading(gen);
    sh.ended.store(false, Ordering::SeqCst);
    sh.pos_ms.store(0, Ordering::SeqCst);
    // A fresh sink per track instead of stop() + append() on one sink:
    // append() on a stopped sink waits for the old source to end, and if that
    // source or the output device was stuck, the player thread hung for good.
    // Dropping a sink only flags it stopped.
    drop(sh.sink.lock().unwrap().take());
    if output.as_ref().map_or(true, Output::is_lost) {
        if output.is_some() {
            crate::log!("player: audio output lost or changed, reopening");
        }
        *output = None; // close the old stream before opening a new one
        *output = Output::open();
    }
    let Some(out) = output.as_ref() else {
        *sh.state.lock().unwrap() = PlaybackState::Idle;
        return None;
    };
    let sink = match Sink::try_new(&out.handle) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            crate::log!("player: Sink::try_new FAILED ({e})! Could not create audio sink.");
            *sh.state.lock().unwrap() = PlaybackState::Idle;
            return None;
        }
    };
    sink.set_volume(amplitude);
    sink.set_speed(speed);
    *sh.sink.lock().unwrap() = Some(sink.clone());
    Some(Playback {
        gen,
        sink,
        seek: Arc::new(Mutex::new(None)),
        fade_in,
    })
}

/// Hand a decoder to its track's sink, unless a newer Play or Stop has taken
/// over since the track began. The check and the append happen under the
/// state lock, so they can't interleave with one.
fn play_decoded<R>(sh: &Shared, t: &Playback, dec: Decoder<R>) -> bool
where
    R: Read + Seek + Send + Sync + 'static,
{
    let mut st = sh.state.lock().unwrap();
    if *st != PlaybackState::Loading(t.gen) {
        return false;
    }
    let clocked = with_track_clock(dec, &sh.pos_ms, &sh.generation, t.gen, &t.seek);
    let staged = crate::dsp::Dsp::new(clocked.convert_samples::<f32>(), sh.dsp.clone());
    let src: Box<dyn Source<Item = f32> + Send> = match t.fade_in {
        Some(d) => Box::new(staged.fade_in(d)),
        None => Box::new(staged),
    };
    t.sink.append(src);
    // a Pause that came in while it was loading still holds
    *st = if t.sink.is_paused() {
        PlaybackState::Paused(t.gen)
    } else {
        PlaybackState::Playing(t.gen)
    };
    true
}

/// Probe a stream on a thread of its own, then hand it to its track's sink.
/// The probe waits for the first data (up to 15s on a slow link), and the
/// command loop must stay free meanwhile for a Stop, Pause or the next Play.
/// (It used to run on the command loop, where the generation couldn't change
/// under it, so a stale decoder was never caught.)
fn spawn_decoder(
    sh: &Shared,
    t: Playback,
    src: PipeSource,
    kind: &'static str,
    track_id: Option<i64>,
) {
    let sh = sh.clone();
    std::thread::spawn(move || match open_decoder(src) {
        Ok(dec) => {
            if play_decoded(&sh, &t, dec) {
                crate::log!("player: playing ({kind}: {:?})", track_id);
            } else {
                crate::log!(
                    "player: decoded {kind} is stale (current gen {}, my_gen {}), dropping",
                    sh.generation.load(Ordering::SeqCst),
                    t.gen
                );
            }
        }
        Err(e) => {
            let mut st = sh.state.lock().unwrap();
            if *st == PlaybackState::Loading(t.gen) {
                *st = PlaybackState::Idle;
                crate::log!("player: decode FAILED: {e}");
            }
        }
    });
}

/// A decoder for the track's disk-cached audio, if there is a usable file.
fn open_cached(track_id: i64) -> Option<Decoder<std::io::BufReader<std::fs::File>>> {
    let cache_path = crate::config::cached_audio_path(track_id);
    if !cache_path
        .metadata()
        .map(|m| m.len() > 10000)
        .unwrap_or(false)
    {
        return None;
    }
    let file = std::fs::File::open(&cache_path).ok()?;
    rodio::Decoder::new(std::io::BufReader::new(file)).ok()
}

pub fn spawn() -> Result<PlayerHandle> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PlayerCommand>();
    let sh = Shared {
        pos_ms: Arc::new(AtomicU64::new(0)),
        ended: Arc::new(AtomicBool::new(false)),
        state: Arc::new(Mutex::new(PlaybackState::Idle)),
        generation: Arc::new(AtomicU64::new(0)),
        sink: Arc::new(Mutex::new(None)),
        dsp: Arc::new(crate::dsp::DspParams::default()),
        cache_audio: Arc::new(AtomicBool::new(false)),
    };
    let handle_pos = sh.pos_ms.clone();
    let handle_dsp = sh.dsp.clone();
    let handle_cache = sh.cache_audio.clone();
    let handle_ended = sh.ended.clone();

    std::thread::spawn(move || {
        // No device at startup isn't fatal: every Play tries to open one again.
        let mut output = Output::open();
        let mut volume = 0.7f32;
        let mut speed = 1.0f32;
        // the current track's pending seek (see with_track_clock)
        let mut seek: SeekSlot = Arc::new(Mutex::new(None));
        // set by FadeOut: the next track fades in over the same time
        let mut fade_next: Option<Duration> = None;
        let client = blocking_client(None);

        #[inline]
        fn to_perceptual_amplitude(v: f32) -> f32 {
            let v = v.clamp(0.0, 1.0);
            v * v
        }

        // watcher: reliable end-of-track detection
        {
            let sh = sh.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(60));
                    // pos_ms is written by the track clock (with_track_clock)
                    // The state stays locked through the check, so a Play or
                    // Stop meanwhile can't have its new state overwritten.
                    let mut st = sh.state.lock().unwrap();
                    // In Loading, Paused, or Idle states, NEVER declare ended!
                    // `ended` is left alone then: only the next Play/Stop or the
                    // UI's swap clears it. (Clearing it here wiped it ~60ms after
                    // a natural end, often before the UI polled it, and the queue
                    // never advanced.)
                    let PlaybackState::Playing(gen) = *st else {
                        continue;
                    };
                    let Some(sink) = sh.current_sink() else {
                        continue;
                    };
                    if sink.is_paused() || !sink.empty() {
                        continue;
                    }
                    let cur_pos = sh.pos_ms.load(Ordering::Relaxed);
                    // Safety: Only declare natural end-of-track if it has played for at least 1200ms.
                    // An empty sink at 0ms or <1200ms means it is still transitioning or buffering.
                    if cur_pos >= 1200 {
                        *st = PlaybackState::Idle;
                        sh.ended.store(true, Ordering::SeqCst);
                        crate::log!("player: track finished naturally at {cur_pos}ms (gen {gen})");
                    }
                }
            });
        }

        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                PlayerCommand::PlayCached { track_id, path } => {
                    let Some(t) = begin_track(
                        &sh,
                        &mut output,
                        to_perceptual_amplitude(volume),
                        speed,
                        fade_next.take(),
                    ) else {
                        continue;
                    };
                    seek = t.seek.clone();
                    crate::log!(
                        "player: playing cached audio ({track_id}) from {}",
                        path.display()
                    );
                    match std::fs::File::open(&path) {
                        Ok(file) => {
                            let reader = std::io::BufReader::new(file);
                            match rodio::Decoder::new(reader) {
                                Ok(dec) => {
                                    if play_decoded(&sh, &t, dec) {
                                        crate::log!(
                                            "player: playing (cached disk file: {track_id})"
                                        );
                                    }
                                }
                                Err(e) => {
                                    crate::log!("player: decode cached file FAILED: {e}");
                                    let _ = std::fs::remove_file(&path);
                                    *sh.state.lock().unwrap() = PlaybackState::Idle;
                                }
                            }
                        }
                        Err(e) => {
                            crate::log!("player: open cached file FAILED: {e}");
                            *sh.state.lock().unwrap() = PlaybackState::Idle;
                        }
                    }
                }
                PlayerCommand::PlaySingle {
                    track_id,
                    url,
                    proxy,
                } => {
                    let Some(t) = begin_track(
                        &sh,
                        &mut output,
                        to_perceptual_amplitude(volume),
                        speed,
                        fade_next.take(),
                    ) else {
                        continue;
                    };
                    seek = t.seek.clone();

                    // Check if already in disk cache
                    if let Some(tid) = track_id.filter(|_| sh.cache_audio.load(Ordering::Relaxed)) {
                        if let Some(dec) = open_cached(tid) {
                            if play_decoded(&sh, &t, dec) {
                                crate::log!("player: playing (instant cache hit: {tid})");
                            }
                            continue;
                        }
                    }

                    let buf = Arc::new(Mutex::new(Vec::new()));
                    let done = Arc::new(AtomicBool::new(false));
                    let gen = sh.generation.clone();
                    let my_gen = t.gen;
                    let buf2 = buf.clone();
                    let done2 = done.clone();
                    let client2 = match &proxy {
                        Some(p) => blocking_client(Some(p)),
                        None => client.clone(),
                    };
                    let keep = sh.cache_audio.clone();
                    std::thread::spawn(move || {
                        let check = || {
                            my_gen != gen.load(Ordering::SeqCst) || done2.load(Ordering::Relaxed)
                        };
                        match client2.get(&url).send() {
                            // An error page (403 on an expired URL, 5xx) is not
                            // audio: it must reach neither the decoder nor the cache.
                            Ok(resp) if !resp.status().is_success() => {
                                crate::log!("player: download FAILED: HTTP {}", resp.status());
                                done2.store(true, Ordering::Relaxed);
                            }
                            Ok(mut resp) => {
                                use std::io::Read as _;
                                let expected = resp.content_length();
                                // Set only when the body ended cleanly: every later
                                // play takes the cache hit, so a cut-off file would
                                // end early for good.
                                let mut complete = false;
                                let mut tmp = [0u8; 65536];
                                loop {
                                    if check() {
                                        done2.store(true, Ordering::Relaxed); // no reader may wait on us
                                        return;
                                    }
                                    match resp.read(&mut tmp) {
                                        Ok(0) => {
                                            complete = true;
                                            break;
                                        }
                                        Ok(n) => {
                                            if let Ok(mut b) = buf2.lock() {
                                                b.extend_from_slice(&tmp[..n]);
                                            }
                                        }
                                        Err(e) => {
                                            crate::log!("player: download FAILED: {e}");
                                            break;
                                        }
                                    }
                                }
                                done2.store(true, Ordering::Relaxed);

                                if let Some(tid) = track_id.filter(|_| keep.load(Ordering::Relaxed))
                                {
                                    let cache_path = crate::config::cached_audio_path(tid);
                                    if let Ok(b) = buf2.lock() {
                                        // shorter than its Content-Length: cut off too
                                        let whole = complete
                                            && expected.map_or(true, |n| n == b.len() as u64);
                                        if !whole {
                                            crate::log!("player: download incomplete ({} bytes), not cached", b.len());
                                        } else if b.len() > 10000 && !cache_path.exists() {
                                            let tmp = cache_path.with_extension("tmp");
                                            if std::fs::write(&tmp, &*b).is_ok() {
                                                let _ = std::fs::rename(tmp, &cache_path);
                                                crate::log!(
                                                    "player: cached {} bytes to {}",
                                                    b.len(),
                                                    cache_path.display()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                crate::log!("player: download FAILED: {e}");
                                done2.store(true, Ordering::Relaxed);
                            }
                        }
                    });
                    let src = PipeSource {
                        buf,
                        pos: 0,
                        done,
                        generation: sh.generation.clone(),
                        my_gen: t.gen,
                    };
                    spawn_decoder(&sh, t, src, "single stream", track_id);
                }
                PlayerCommand::PlayChunks {
                    track_id,
                    init,
                    chunks,
                    proxy,
                } => {
                    let Some(t) = begin_track(
                        &sh,
                        &mut output,
                        to_perceptual_amplitude(volume),
                        speed,
                        fade_next.take(),
                    ) else {
                        continue;
                    };
                    seek = t.seek.clone();

                    // Check if already in disk cache
                    if let Some(tid) = track_id.filter(|_| sh.cache_audio.load(Ordering::Relaxed)) {
                        if let Some(dec) = open_cached(tid) {
                            if play_decoded(&sh, &t, dec) {
                                crate::log!("player: playing (instant cache hit: {tid})");
                            }
                            continue;
                        }
                    }

                    let mut sources = Vec::with_capacity(chunks.len() + 1);
                    if let Some(i) = init {
                        sources.push(i);
                    }
                    sources.extend(chunks);
                    crate::log!("player: starting chunked stream ({} parts)", sources.len());
                    let buf = Arc::new(Mutex::new(Vec::new()));
                    let done = Arc::new(AtomicBool::new(false));
                    start_loader(
                        sh.cache_audio.clone(),
                        match &proxy {
                            Some(p) => blocking_client(Some(p)),
                            None => client.clone(),
                        },
                        sources,
                        buf.clone(),
                        done.clone(),
                        track_id,
                        sh.generation.clone(),
                        t.gen,
                    );
                    let src = PipeSource {
                        buf,
                        pos: 0,
                        done,
                        generation: sh.generation.clone(),
                        my_gen: t.gen,
                    };
                    spawn_decoder(&sh, t, src, "chunked stream", track_id);
                }
                PlayerCommand::FadeOut(ms) => {
                    // like Stop for everyone else (its end is nobody's, its
                    // position no longer the playhead), but the sink goes on
                    // on its own, fading to nothing
                    sh.generation.fetch_add(1, Ordering::SeqCst);
                    *sh.state.lock().unwrap() = PlaybackState::Idle;
                    sh.ended.store(false, Ordering::SeqCst);
                    let old = sh.sink.lock().unwrap().take();
                    if let Some(old) = old.filter(|s| !s.is_paused() && !s.empty()) {
                        let d = Duration::from_millis(ms.max(100));
                        fade_next = Some(d);
                        std::thread::spawn(move || {
                            const STEPS: u32 = 40;
                            let from = old.volume();
                            for i in 1..=STEPS {
                                std::thread::sleep(d / STEPS);
                                old.set_volume(from * (1.0 - i as f32 / STEPS as f32));
                            }
                            drop(old);
                        });
                    }
                    crate::log!("player: fading out over {ms}ms");
                }
                PlayerCommand::Stop => {
                    fade_next = None;
                    sh.generation.fetch_add(1, Ordering::SeqCst);
                    *sh.state.lock().unwrap() = PlaybackState::Idle;
                    sh.ended.store(false, Ordering::SeqCst);
                    // dropping the sink stops its source
                    drop(sh.sink.lock().unwrap().take());
                    sh.pos_ms.store(0, Ordering::SeqCst);
                    crate::log!("player: stopped");
                }
                PlayerCommand::SetVolume(v) => {
                    volume = v.clamp(0.0, 1.0);
                    if let Some(sink) = sh.current_sink() {
                        sink.set_volume(to_perceptual_amplitude(volume));
                    }
                }
                PlayerCommand::SetSpeed(s) => {
                    speed = s.clamp(0.1, 3.0); // UI range is 0.1x..2.0x
                    if let Some(sink) = sh.current_sink() {
                        sink.set_speed(speed);
                    }
                    crate::log!("player: speed set to {speed}x");
                }
                PlayerCommand::Pause => {
                    if let Some(sink) = sh.current_sink() {
                        sink.pause();
                    }
                    let mut s = sh.state.lock().unwrap();
                    if let PlaybackState::Playing(g) = *s {
                        *s = PlaybackState::Paused(g);
                    }
                }
                PlayerCommand::Resume => {
                    if let Some(sink) = sh.current_sink() {
                        sink.play();
                    }
                    let mut s = sh.state.lock().unwrap();
                    if let PlaybackState::Paused(g) = *s {
                        *s = PlaybackState::Playing(g);
                    }
                }
                // Only leaves the target for the track clock, which seeks below
                // the sink's speed stage, so it stays in track time. Nothing
                // here waits for the seek (see with_track_clock).
                PlayerCommand::SeekMs(ms) => {
                    sh.pos_ms.store(ms, Ordering::Relaxed);
                    *seek.lock().unwrap() = Some(Duration::from_millis(ms));
                    crate::log!("player: seek to {ms}ms");
                }
            }
        }
    });
    Ok(PlayerHandle {
        tx,
        pos_ms: handle_pos,
        ended: handle_ended,
        dsp: handle_dsp,
        cache_audio: handle_cache,
    })
}

/// Sniff the pipe's first bytes and pick the right decoder.
/// Wraps construction in catch_unwind so a decoder panic cannot kill the player thread.
fn open_decoder(src: PipeSource) -> Result<Decoder<PipeSource>> {
    // wait until we have enough bytes to sniff and probe the format reliably
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if src.is_stale() {
                anyhow::bail!("playback moved on before the stream started");
            }
            let avail = src.available();
            if avail >= 8192 || src.done.load(Ordering::Relaxed) {
                break;
            }
            if std::time::Instant::now() > deadline {
                if avail > 0 {
                    break;
                }
                anyhow::bail!("timeout waiting for audio data");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let head: Vec<u8> = {
        let b = src.buf.lock().unwrap();
        b.iter().take(32).copied().collect()
    };
    let is_mp4 = head.len() >= 8 && (&head[4..8] == b"ftyp" || &head[4..8] == b"styp");
    let is_mp3 = head.starts_with(b"ID3") || head.first() == Some(&0xFF);
    crate::log!(
        "player: sniff {} bytes, mp4={}, mp3={}",
        head.len(),
        is_mp4,
        is_mp3
    );

    if is_mp3 {
        // Fix #3: catch_unwind guards against any rodio/symphonia internal panics.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Decoder::new_mp3(src)));
        return match result {
            Ok(Ok(dec)) => Ok(dec),
            Ok(Err(e)) => anyhow::bail!("mp3 decode init failed: {e}"),
            Err(_) => anyhow::bail!("mp3 decoder panicked during initialization"),
        };
    }
    if is_mp4 {
        // fMP4 (MPEG-4 AAC in fragmented MP4): use generic decoder
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Decoder::new(src)));
        return match result {
            Ok(Ok(dec)) => Ok(dec),
            Ok(Err(e)) => anyhow::bail!("fMP4 decode init failed: {e}"),
            Err(_) => anyhow::bail!("fMP4 decoder panicked during initialization"),
        };
    }
    // fallback: let symphonia probe all registered container/codec formats
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Decoder::new(src)));
    match result {
        Ok(Ok(dec)) => Ok(dec),
        Ok(Err(e)) => anyhow::bail!("fallback decode init failed: {e}"),
        Err(_) => anyhow::bail!("fallback decoder panicked during initialization"),
    }
}
