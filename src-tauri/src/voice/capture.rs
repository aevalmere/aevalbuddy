//! The microphone, through cpal (WASAPI shared mode), on a thread of its
//! own: cpal's stream must stay on the thread that made it. Samples gather
//! in memory, mono, at the device's rate, and become 16 kHz when recording
//! stops. Nothing is written anywhere; dropping the [`Recorder`] drops the
//! audio.
//!
//! CI runners have no microphone, so a WAV file can stand in for one:
//! `AEVAL_VOICE_WAV=<path>`, or [`set_fixture`] from the smoke probe. It is
//! "spoken" in real time, so the level meters move as they would.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use super::wav;

/// The longest utterance: five minutes, then it stops by itself.
pub const MAX_SECONDS: u64 = 300;

pub const NO_MIC: &str = "No microphone found. Plug one in, or pick another in Settings.";
pub const MIC_BLOCKED: &str =
    "Windows is blocking the microphone. Turn on microphone access for desktop apps in Windows Settings, Privacy.";

/// One utterance, at [`wav::RATE`], mono.
pub struct Audio {
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / wav::RATE as f32
    }

    /// The loudness of the loudest tenth of a second: a whisper still
    /// counts, a silent room does not.
    pub fn peak_level(&self) -> f32 {
        let step = (wav::RATE / 10) as usize;
        self.samples
            .chunks(step)
            .map(|c| wav::level(wav::rms(c)))
            .fold(0.0, f32::max)
    }
}

/// A recording in progress.
pub struct Recorder {
    stop: Option<mpsc::Sender<()>>,
    done: Option<JoinHandle<Result<Audio, String>>>,
    level: Arc<AtomicU32>,
    tap: Tap,
}

/// A look at a recording while it is still going, for transcribing the
/// first part of an utterance before the key is let go.
#[derive(Clone)]
pub enum Tap {
    /// The microphone's samples so far, mono at `rate`.
    Mic { buf: Shared, rate: u32 },
    /// A fixture "spoken" since `began`.
    File {
        samples: Arc<Vec<f32>>,
        began: Instant,
    },
}

impl Tap {
    /// What was heard from `from` (a 16 kHz sample index) on, at 16 kHz.
    pub fn since(&self, from: usize) -> Vec<f32> {
        match self {
            Tap::Mic { buf, rate } => {
                let ratio = *rate as f64 / wav::RATE as f64;
                let start = (from as f64 * ratio).floor() as usize;
                let raw = {
                    let b = buf.lock();
                    b.get(start..).map(<[f32]>::to_vec).unwrap_or_default()
                };
                wav::resample(&raw, *rate, wav::RATE)
            }
            Tap::File { samples, began } => {
                let at = (began.elapsed().as_secs_f32() * wav::RATE as f32) as usize;
                let at = at.min(samples.len());
                samples
                    .get(from..at)
                    .map(<[f32]>::to_vec)
                    .unwrap_or_default()
            }
        }
    }
}

impl Recorder {
    pub fn tap(&self) -> Tap {
        self.tap.clone()
    }

    /// The current loudness, 0..1.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// Stops and hands back what was heard.
    pub fn finish(mut self) -> Result<Audio, String> {
        self.stop.take();
        match self.done.take().map(JoinHandle::join) {
            Some(Ok(r)) => r,
            _ => Err("The recording stopped unexpectedly.".into()),
        }
    }
}

impl Drop for Recorder {
    // Dropping the sender ends the thread, which drops the audio.
    fn drop(&mut self) {
        self.stop.take();
    }
}

static FIXTURE: Mutex<Option<PathBuf>> = parking_lot::const_mutex(None);

/// Makes the next recordings read `path` instead of the microphone (the
/// smoke probe); None gives the microphone back.
pub fn set_fixture(path: Option<PathBuf>) {
    *FIXTURE.lock() = path;
}

fn fixture() -> Option<PathBuf> {
    let set = FIXTURE.lock().clone();
    set.or_else(|| std::env::var_os("AEVAL_VOICE_WAV").map(PathBuf::from))
}

/// Starts recording from the named input device (empty: the Windows
/// default). Errors are sentences for the person.
pub fn start(device: &str) -> Result<Recorder, String> {
    match fixture() {
        Some(path) => start_fixture(path),
        None => start_mic(device.to_string()),
    }
}

/// Every input device's name, the default first.
pub fn devices() -> Vec<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut names: Vec<String> = host
        .input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    names.sort();
    names.dedup();
    if let Some(d) = default {
        names.retain(|n| *n != d);
        names.insert(0, d);
    }
    names
}

fn mic_error(e: impl std::fmt::Display) -> String {
    let text = e.to_string();
    let lower = text.to_lowercase();
    if lower.contains("0x80070005") || lower.contains("denied") {
        MIC_BLOCKED.to_string()
    } else if lower.contains("not available") || lower.contains("no longer") {
        NO_MIC.to_string()
    } else {
        format!("The microphone did not start ({text}).")
    }
}

pub type Shared = Arc<Mutex<Vec<f32>>>;

/// Appends one callback's frames as mono and updates the level.
fn push(buf: &Shared, level: &AtomicU32, frames: impl Iterator<Item = f32>, channels: usize) {
    let interleaved: Vec<f32> = frames.collect();
    let mono = wav::downmix(&interleaved, channels);
    let now = wav::level(wav::rms(&mono));
    // Fast attack, slower release, so speech reads as a steady meter.
    let was = f32::from_bits(level.load(Ordering::Relaxed));
    let next = if now > was {
        now
    } else {
        was * 0.8 + now * 0.2
    };
    level.store(next.to_bits(), Ordering::Relaxed);
    buf.lock().extend_from_slice(&mono);
}

fn open(device: &str, buf: Shared, level: Arc<AtomicU32>) -> Result<(cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let named = || {
        host.input_devices()
            .ok()?
            .find(|d| d.name().ok().as_deref() == Some(device))
    };
    let dev = if device.is_empty() { None } else { named() };
    let dev = dev.or_else(|| host.default_input_device()).ok_or(NO_MIC)?;
    let cfg = dev.default_input_config().map_err(mic_error)?;
    let channels = cfg.channels() as usize;
    let rate = cfg.sample_rate().0;
    let sc: cpal::StreamConfig = cfg.config();
    let on_err = |_e: cpal::StreamError| {};
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev.build_input_stream(
            &sc,
            move |d: &[f32], _: &cpal::InputCallbackInfo| {
                push(&buf, &level, d.iter().copied(), channels)
            },
            on_err,
            None,
        ),
        cpal::SampleFormat::I16 => dev.build_input_stream(
            &sc,
            move |d: &[i16], _: &cpal::InputCallbackInfo| {
                push(
                    &buf,
                    &level,
                    d.iter().map(|s| *s as f32 / 32768.0),
                    channels,
                )
            },
            on_err,
            None,
        ),
        cpal::SampleFormat::U16 => dev.build_input_stream(
            &sc,
            move |d: &[u16], _: &cpal::InputCallbackInfo| {
                push(
                    &buf,
                    &level,
                    d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0),
                    channels,
                )
            },
            on_err,
            None,
        ),
        other => {
            return Err(format!(
                "The microphone sends {other:?} audio, which is not supported."
            ))
        }
    }
    .map_err(mic_error)?;
    stream.play().map_err(mic_error)?;
    Ok((stream, rate))
}

fn start_mic(device: String) -> Result<Recorder, String> {
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(Shared, u32), String>>(1);
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let level = Arc::new(AtomicU32::new(0));
    let lv = level.clone();
    let done = thread::Builder::new()
        .name("voice-mic".into())
        .spawn(move || {
            let buf: Shared = Arc::new(Mutex::new(Vec::new()));
            let (stream, rate) = match open(&device, buf.clone(), lv) {
                Ok(x) => x,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.clone()));
                    return Err(e);
                }
            };
            let _ = ready_tx.send(Ok((buf.clone(), rate)));
            // Ends on stop, on the Recorder being dropped, or at the cap.
            let _ = stop_rx.recv_timeout(Duration::from_secs(MAX_SECONDS));
            drop(stream);
            let raw = std::mem::take(&mut *buf.lock());
            Ok(Audio {
                samples: wav::resample(&raw, rate, wav::RATE),
            })
        })
        .map_err(|e| format!("The microphone did not start ({e})."))?;
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok((buf, rate))) => Ok(Recorder {
            stop: Some(stop_tx),
            done: Some(done),
            level,
            tap: Tap::Mic { buf, rate },
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("The microphone did not start in time.".into()),
    }
}

fn start_fixture(path: PathBuf) -> Result<Recorder, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mono, rate) = wav::read(&bytes)?;
    let samples = Arc::new(wav::resample(&mono, rate, wav::RATE));
    let began = Instant::now();
    let tap = Tap::File {
        samples: samples.clone(),
        began,
    };
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let level = Arc::new(AtomicU32::new(0));
    let lv = level.clone();
    let done = thread::Builder::new()
        .name("voice-fixture".into())
        .spawn(move || {
            let step = (wav::RATE / 20) as usize;
            let tick = Duration::from_millis(50);
            while let Err(mpsc::RecvTimeoutError::Timeout) = stop_rx.recv_timeout(tick) {
                let at = (began.elapsed().as_secs_f32() * wav::RATE as f32) as usize;
                let now = samples.get(at..(at + step).min(samples.len()));
                let l = now.map(|s| wav::level(wav::rms(s))).unwrap_or(0.0);
                lv.store(l.to_bits(), Ordering::Relaxed);
                if began.elapsed() > Duration::from_secs(MAX_SECONDS) {
                    break;
                }
            }
            // The whole file, however long the hold: the probe's hold is
            // timed loosely.
            Ok(Audio {
                samples: samples.to_vec(),
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(Recorder {
        stop: Some(stop_tx),
        done: Some(done),
        level,
        tap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixture_records_like_a_microphone() {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/voice/fixtures/dictation.wav"
        ));
        let rec = start_fixture(path).unwrap();
        // The file starts with a little silence: wait for the first word.
        let mut moved = false;
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            moved |= rec.level() > 0.0;
        }
        assert!(moved, "the meter never moved");
        let audio = rec.finish().unwrap();
        assert!(audio.seconds() > 5.0);
        assert!(audio.peak_level() > 0.5);
        let quiet = Audio {
            samples: vec![0.0005; wav::RATE as usize],
        };
        assert!(quiet.peak_level() < 0.1);
    }

    #[test]
    fn microphone_errors_read_as_sentences() {
        assert_eq!(mic_error("Access is denied. (0x80070005)"), MIC_BLOCKED);
        assert_eq!(
            mic_error("The requested device is no longer available."),
            NO_MIC
        );
        assert!(mic_error("boom").starts_with("The microphone did not start"));
    }
}
