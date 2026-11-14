//! Transcribing while the person is still talking. Every [`POLL`] the
//! recording so far is checked for a pause; when one follows enough speech,
//! everything before it is transcribed then and there, on this thread. When
//! the key is let go only the part after the last such pause is left, so a
//! long utterance with a breath in it answers as fast as a short one.
//!
//! Pieces are cut only at pauses, never mid-word, and each piece is given
//! the text before it as Whisper's prompt, so casing and names carry over.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{capture::Tap, vad, wav, whisper};

const POLL: Duration = Duration::from_millis(250);
/// At least this much speech before a cut (in 20 ms frames): 1.5 s.
const MIN_SPEECH: usize = 75;
/// A pause this long is a place to cut: 0.5 s.
const MIN_PAUSE: usize = 25;

/// What was transcribed before the release.
#[derive(Default)]
pub struct Done {
    /// The 16 kHz sample index up to which the text below covers.
    pub upto: usize,
    pub text: String,
    pub pieces: usize,
    /// Whisper time spent while the key was held, in ms.
    pub busy_ms: u64,
}

/// Dropping it (a cancelled or too-short utterance) ends the thread at
/// once, or after the piece it is transcribing.
pub struct Streamer {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<Done>>,
}

impl Streamer {
    /// Starts watching `tap`, transcribing with the model at `path`.
    pub fn start(tap: Tap, path: PathBuf, language: String) -> Option<Streamer> {
        let (stop, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("voice-stream".into())
            .spawn(move || watch(&tap, &path, &language, &rx))
            .ok()?;
        Some(Streamer {
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Stops watching. Waits for a piece being transcribed now, and hands
    /// back what was done.
    pub fn finish(mut self) -> Done {
        self.stop.take();
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
    }
}

/// Joins two pieces of text with one space.
pub fn join(a: &str, b: &str) -> String {
    match (a.trim(), b.trim()) {
        ("", b) => b.to_string(),
        (a, "") => a.to_string(),
        (a, b) => format!("{a} {b}"),
    }
}

fn watch(tap: &Tap, path: &std::path::Path, language: &str, stop: &mpsc::Receiver<()>) -> Done {
    let mut done = Done::default();
    // Wakes every POLL, and at once when the Streamer is finished or
    // dropped (its sender goes).
    while let Err(RecvTimeoutError::Timeout) = stop.recv_timeout(POLL) {
        let fresh = tap.since(done.upto);
        let Some(cut) = vad::last_pause(&fresh, MIN_SPEECH, MIN_PAUSE) else {
            continue;
        };
        let piece = &fresh[..cut];
        let began = Instant::now();
        if let Some(heard) = super::engine::piece(path, piece) {
            match whisper::transcribe_after(path, heard, language, &done.text) {
                Ok(t) => {
                    done.text = join(&done.text, &super::cleanup::strip_noise(&t));
                    done.pieces += 1;
                }
                // Left for the release to try again with the rest.
                Err(_) => break,
            }
        }
        done.busy_ms += began.elapsed().as_millis() as u64;
        done.upto += cut;
    }
    done
}

/// Seconds of 16 kHz audio, for the timing log.
pub fn seconds(samples: usize) -> f32 {
    samples as f32 / wav::RATE as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_join_with_one_space() {
        assert_eq!(
            join("So I think", " we should meet."),
            "So I think we should meet."
        );
        assert_eq!(join("", "Hi."), "Hi.");
        assert_eq!(join("Hi.", "  "), "Hi.");
    }
}
