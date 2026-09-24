//! The stage every track plays through, between its decoder and its sink:
//! the equalizer, volume normalization and mono audio (Settings > Playback).
//! Everything is read from shared atomics, so a change is heard at once, in
//! the middle of a track. With all three off the samples pass untouched.

use rodio::Source;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The equalizer's bands (Hz), Spotify's six.
pub const EQ_BANDS: [f32; 6] = [60.0, 150.0, 400.0, 1000.0, 2400.0, 15000.0];
/// How far a band goes each way (dB).
pub const EQ_RANGE_DB: f32 = 12.0;

const MAX_CHANNELS: usize = 8;

/// Settings shared with the audio thread.
pub struct DspParams {
    eq_on: AtomicBool,
    eq_gains: [AtomicU32; 6],
    mono: AtomicBool,
    normalize: AtomicBool,
    /// Normalization's target loudness, as an RMS level (f32 bits).
    target: AtomicU32,
    /// Bumped on every change: the stage re-reads what it caches.
    version: AtomicU64,
}

impl Default for DspParams {
    fn default() -> Self {
        Self {
            eq_on: AtomicBool::new(false),
            eq_gains: Default::default(),
            mono: AtomicBool::new(false),
            normalize: AtomicBool::new(false),
            target: AtomicU32::new(NORMAL_RMS.to_bits()),
            version: AtomicU64::new(0),
        }
    }
}

/// Target RMS levels of the three volume levels, about Spotify's -11, -14
/// and -19 LUFS.
pub const LOUD_RMS: f32 = 0.282;
pub const NORMAL_RMS: f32 = 0.2;
pub const QUIET_RMS: f32 = 0.112;

impl DspParams {
    pub fn set_eq(&self, on: bool, gains_db: [f32; 6]) {
        self.eq_on.store(on, Ordering::Relaxed);
        for (slot, g) in self.eq_gains.iter().zip(gains_db) {
            let g = g.clamp(-EQ_RANGE_DB, EQ_RANGE_DB);
            slot.store(g.to_bits(), Ordering::Relaxed);
        }
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn set_mono(&self, on: bool) {
        self.mono.store(on, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn set_normalize(&self, on: bool, target_rms: f32) {
        self.normalize.store(on, Ordering::Relaxed);
        self.target.store(target_rms.to_bits(), Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }
}

/// A second-order filter (RBJ's cookbook), transposed direct form II.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

#[derive(Clone, Copy)]
enum Shape {
    LowShelf,
    Peak,
    HighShelf,
}

impl Biquad {
    fn new(shape: Shape, freq: f32, gain_db: f32, rate: f32) -> Option<Biquad> {
        if gain_db.abs() < 0.05 || freq >= rate * 0.49 {
            return None;
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Peak => {
                let alpha = sin / (2.0 * 1.0); // Q = 1: about an octave and a half
                (
                    1.0 + alpha * a,
                    -2.0 * cos,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos,
                    1.0 - alpha / a,
                )
            }
            Shape::LowShelf | Shape::HighShelf => {
                // shelf slope 1
                let alpha = sin / 2.0 * std::f32::consts::SQRT_2;
                let s = 2.0 * a.sqrt() * alpha;
                if matches!(shape, Shape::LowShelf) {
                    (
                        a * ((a + 1.0) - (a - 1.0) * cos + s),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                        a * ((a + 1.0) - (a - 1.0) * cos - s),
                        (a + 1.0) + (a - 1.0) * cos + s,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                        (a + 1.0) + (a - 1.0) * cos - s,
                    )
                } else {
                    (
                        a * ((a + 1.0) + (a - 1.0) * cos + s),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                        a * ((a + 1.0) + (a - 1.0) * cos - s),
                        (a + 1.0) - (a - 1.0) * cos + s,
                        2.0 * ((a - 1.0) - (a + 1.0) * cos),
                        (a + 1.0) - (a - 1.0) * cos - s,
                    )
                }
            }
        };
        Some(Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        })
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Wraps a track's samples; see the module doc.
pub struct Dsp<S> {
    inner: S,
    params: Arc<DspParams>,
    seen: u64,
    /// The frame being handed out (one sample per channel).
    frame: [f32; MAX_CHANNELS],
    len: usize,
    idx: usize,
    channels: u16,
    rate: u32,
    /// Per channel, the active bands only.
    filters: Vec<Vec<Biquad>>,
    mono: bool,
    normalize: bool,
    target: f32,
    /// Mean square level, and the gain following it.
    power: f32,
    gain: f32,
}

impl<S> Dsp<S>
where
    S: Source<Item = f32>,
{
    pub fn new(inner: S, params: Arc<DspParams>) -> Dsp<S> {
        let channels = inner.channels().max(1);
        let rate = inner.sample_rate().max(1);
        let mut dsp = Dsp {
            inner,
            params,
            seen: u64::MAX,
            frame: [0.0; MAX_CHANNELS],
            len: 0,
            idx: 0,
            channels,
            rate,
            filters: Vec::new(),
            mono: false,
            normalize: false,
            target: NORMAL_RMS,
            power: NORMAL_RMS * NORMAL_RMS,
            gain: 1.0,
        };
        dsp.reload();
        dsp
    }

    /// Re-read the settings (and rebuild the filters for this rate).
    fn reload(&mut self) {
        let p = &self.params;
        self.seen = p.version.load(Ordering::Acquire);
        self.mono = p.mono.load(Ordering::Relaxed);
        let normalize = p.normalize.load(Ordering::Relaxed);
        if normalize && !self.normalize {
            // start at unity: a quiet intro isn't blasted while it measures
            self.power = self.target * self.target;
            self.gain = 1.0;
        }
        self.normalize = normalize;
        self.target = f32::from_bits(p.target.load(Ordering::Relaxed));
        let eq_on = p.eq_on.load(Ordering::Relaxed);
        let bands: Vec<Biquad> = if eq_on {
            EQ_BANDS
                .iter()
                .enumerate()
                .filter_map(|(i, &f)| {
                    let g = f32::from_bits(p.eq_gains[i].load(Ordering::Relaxed));
                    let shape = match i {
                        0 => Shape::LowShelf,
                        5 => Shape::HighShelf,
                        _ => Shape::Peak,
                    };
                    Biquad::new(shape, f, g, self.rate as f32)
                })
                .collect()
        } else {
            Vec::new()
        };
        // keep each filter's state across a change: no click
        let old = std::mem::take(&mut self.filters);
        self.filters = (0..usize::from(self.channels).min(MAX_CHANNELS))
            .map(|c| {
                let mut chain = bands.clone();
                if let Some(prev) = old.get(c) {
                    if prev.len() == chain.len() {
                        for (f, p) in chain.iter_mut().zip(prev) {
                            f.z1 = p.z1;
                            f.z2 = p.z2;
                        }
                    }
                }
                chain
            })
            .collect();
    }

    /// Read and process the next frame; false at the end of the track.
    fn fill(&mut self) -> bool {
        let channels = self.inner.channels().max(1);
        let rate = self.inner.sample_rate().max(1);
        if channels != self.channels || rate != self.rate {
            self.channels = channels;
            self.rate = rate;
            self.filters.clear();
            self.seen = u64::MAX;
        }
        if self.params.version.load(Ordering::Acquire) != self.seen {
            self.reload();
        }
        let ch = usize::from(channels).min(MAX_CHANNELS);
        self.len = 0;
        self.idx = 0;
        for c in 0..usize::from(channels) {
            match self.inner.next() {
                Some(s) if c < ch => {
                    self.frame[c] = s;
                    self.len += 1;
                }
                Some(_) => {} // past the channels this stage handles
                None => break,
            }
        }
        if self.len == 0 {
            return false;
        }
        let active = self.mono || self.normalize || self.filters.iter().any(|f| !f.is_empty());
        if !active {
            return true;
        }
        let frame = &mut self.frame[..self.len];
        for (c, s) in frame.iter_mut().enumerate() {
            if let Some(chain) = self.filters.get_mut(c) {
                for f in chain.iter_mut() {
                    *s = f.run(*s);
                }
            }
        }
        if self.mono && frame.len() > 1 {
            let avg = frame.iter().sum::<f32>() / frame.len() as f32;
            frame.iter_mut().for_each(|s| *s = avg);
        }
        if self.normalize {
            // mean square of the frame; fast to follow a louder part (so a
            // drop isn't blasted), slow to lift a quiet one
            let ms = frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32;
            let secs = if ms > self.power { 0.4 } else { 3.0 };
            let k = 1.0 / (secs * self.rate as f32);
            self.power += (ms - self.power) * k;
            let want = (self.target / self.power.max(1e-6).sqrt()).clamp(0.25, 2.0);
            self.gain += (want - self.gain) * (1.0 / (0.2 * self.rate as f32));
            frame.iter_mut().for_each(|s| *s *= self.gain);
        }
        // a soft limit instead of clipping what the boosts pushed over
        for s in frame.iter_mut() {
            let a = s.abs();
            if a > 0.9 {
                *s = s.signum() * (0.9 + 0.1 * ((a - 0.9) / 0.1).tanh());
            }
        }
        true
    }
}

impl<S> Iterator for Dsp<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        if self.idx >= self.len && !self.fill() {
            return None;
        }
        let s = self.frame[self.idx];
        self.idx += 1;
        Some(s)
    }
}

impl<S> Source for Dsp<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        let buffered = self.len - self.idx;
        match self.inner.current_frame_len() {
            Some(n) => Some(n + buffered),
            None => None,
        }
    }

    fn channels(&self) -> u16 {
        if self.idx < self.len {
            self.channels
        } else {
            self.inner.channels()
        }
    }

    fn sample_rate(&self) -> u32 {
        if self.idx < self.len {
            self.rate
        } else {
            self.inner.sample_rate()
        }
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}
