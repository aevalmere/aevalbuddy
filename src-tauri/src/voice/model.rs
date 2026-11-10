//! Whisper's weights: downloaded once, on first use, from the whisper.cpp
//! repository on Hugging Face into `<userData>\voice\`, checked against the
//! SHA-256 below before they are used. A `<file>.sha256` beside the model
//! records the check, so a later start does not hash the file again; a file of
//! the wrong size, or without that record, is fetched again.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// One model file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// English-only models are a little better at English; the others take any
/// language. The quantized (q5_1) files are the ones in use: a third of the
/// size of the f16 ones, so they download and load about three times faster
/// and run at least as fast on a laptop CPU, at the same accuracy within a
/// word or two in a hundred. The f16 files are what earlier builds fetched;
/// one already on disk is used until its replacement has arrived
/// ([`legacy_for`]). Sizes and hashes from huggingface.co/ggerganov/whisper.cpp
/// (the LFS object ids), checked 2026-11-09.
pub const MODELS: [Spec; 8] = [
    Spec {
        id: "tiny.en-q5_1",
        file: "ggml-tiny.en-q5_1.bin",
        bytes: 32_166_155,
        sha256: "c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b",
    },
    Spec {
        id: "tiny-q5_1",
        file: "ggml-tiny-q5_1.bin",
        bytes: 32_152_673,
        sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
    },
    Spec {
        id: "base.en-q5_1",
        file: "ggml-base.en-q5_1.bin",
        bytes: 59_721_011,
        sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
    },
    Spec {
        id: "base-q5_1",
        file: "ggml-base-q5_1.bin",
        bytes: 59_707_625,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    },
    Spec {
        id: "small.en-q5_1",
        file: "ggml-small.en-q5_1.bin",
        bytes: 190_098_681,
        sha256: "bfdff4894dcb76bbf647d56263ea2a96645423f1669176f4844a1bf8e478ad30",
    },
    Spec {
        id: "small-q5_1",
        file: "ggml-small-q5_1.bin",
        bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    },
    // What earlier builds downloaded, still used while it is on disk.
    Spec {
        id: "base.en",
        file: "ggml-base.en.bin",
        bytes: 147_964_211,
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    },
    Spec {
        id: "base",
        file: "ggml-base.bin",
        bytes: 147_951_465,
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    },
];

/// The f16 tiny models earlier builds fetched, for [`legacy_for`] only.
const LEGACY_TINY: [Spec; 2] = [
    Spec {
        id: "tiny.en",
        file: "ggml-tiny.en.bin",
        bytes: 77_704_715,
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    },
    Spec {
        id: "tiny",
        file: "ggml-tiny.bin",
        bytes: 77_691_713,
        sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    },
];

const SOURCE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

pub const DOWNLOAD_FAILED: &str =
    "Couldn't download the voice model. Check the connection and try again.";
pub const DAMAGED: &str = "The voice model arrived damaged. Try again.";

/// The model for a Whisper size (`tiny`, `base`, `small`) and language.
pub fn spec_for(size: &str, language: &str) -> &'static Spec {
    let english = language == "en";
    let id = match (size, english) {
        ("tiny", true) => "tiny.en-q5_1",
        ("tiny", false) => "tiny-q5_1",
        ("small", true) => "small.en-q5_1",
        ("small", false) => "small-q5_1",
        (_, true) => "base.en-q5_1",
        (_, false) => "base-q5_1",
    };
    by_id(id).unwrap_or(&MODELS[2])
}

pub fn by_id(id: &str) -> Option<&'static Spec> {
    MODELS.iter().chain(LEGACY_TINY.iter()).find(|m| m.id == id)
}

/// The f16 file an earlier build fetched in place of `spec`, if any.
pub fn legacy_for(spec: &Spec) -> Option<&'static Spec> {
    by_id(spec.id.strip_suffix("-q5_1")?).filter(|l| l.id != spec.id)
}

/// The file to run for `spec` now: its own, or the earlier build's while the
/// new one has not arrived.
pub fn usable(spec: &'static Spec) -> Option<PathBuf> {
    if ready(spec) {
        return Some(path(spec));
    }
    legacy_for(spec).filter(|l| ready(l)).map(path)
}

/// Once `spec` is on disk, the f16 file it replaces is deleted.
pub fn drop_legacy(spec: &Spec) {
    if let Some(l) = legacy_for(spec).filter(|_| ready(spec)) {
        let p = path(l);
        let _ = std::fs::remove_file(marker(&p));
        let _ = std::fs::remove_file(p);
    }
}

pub fn dir() -> PathBuf {
    crate::paths::user_data_dir().join("voice")
}

pub fn path(spec: &Spec) -> PathBuf {
    dir().join(spec.file)
}

fn marker(file: &Path) -> PathBuf {
    let mut m = file.as_os_str().to_owned();
    m.push(".sha256");
    PathBuf::from(m)
}

/// The file is there, whole, and was checked.
pub fn ready(spec: &Spec) -> bool {
    ready_at(&path(spec), spec)
}

fn ready_at(file: &Path, spec: &Spec) -> bool {
    let size_ok = std::fs::metadata(file).is_ok_and(|m| m.len() == spec.bytes);
    let checked = std::fs::read_to_string(marker(file)).is_ok_and(|s| s.trim() == spec.sha256);
    size_ok && checked
}

/// Whether bytes that hashed to `hex` are `spec`.
pub fn verify(spec: &Spec, len: u64, hex: &str) -> Result<(), String> {
    if len != spec.bytes || !hex.eq_ignore_ascii_case(spec.sha256) {
        return Err(DAMAGED.into());
    }
    Ok(())
}

/// What is downloading now: (model id, received, total).
static PROGRESS: Mutex<Option<(&'static str, u64, u64)>> = parking_lot::const_mutex(None);

pub fn progress() -> Option<(&'static str, u64, u64)> {
    *PROGRESS.lock()
}

fn downloads() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// The model's path, downloading it first if need be. One download at a
/// time; a second caller waits for the first and then finds the file.
/// `on_progress(received, total)` runs about once per percent.
pub async fn ensure(
    spec: &'static Spec,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<PathBuf, String> {
    let file = path(spec);
    if ready_at(&file, spec) {
        return Ok(file);
    }
    let _one = downloads().lock().await;
    if ready_at(&file, spec) {
        return Ok(file);
    }
    *PROGRESS.lock() = Some((spec.id, 0, spec.bytes));
    let got = download(spec, &file, on_progress).await;
    *PROGRESS.lock() = None;
    got.map(|()| file)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(concat!("Aevaleron/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| DOWNLOAD_FAILED.to_string())
}

async fn download(
    spec: &'static Spec,
    file: &Path,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<(), String> {
    let folder = file.parent().ok_or(DOWNLOAD_FAILED)?;
    std::fs::create_dir_all(folder).map_err(|_| DOWNLOAD_FAILED.to_string())?;
    let part = folder.join(format!("{}.part", spec.file));
    let failed = |why: String| {
        let _ = std::fs::remove_file(&part);
        crate::crash::write("voice-model", &why);
        DOWNLOAD_FAILED.to_string()
    };
    let url = format!("{SOURCE}{}", spec.file);
    let resp = client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| failed(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(failed(format!("{url}: HTTP {}", resp.status())));
    }
    let total = resp.content_length().unwrap_or(spec.bytes);
    let mut out = std::fs::File::create(&part).map_err(|e| failed(e.to_string()))?;
    let mut hash = Sha256::new();
    let mut got: u64 = 0;
    let mut told: u64 = 0;
    let mut body = resp.bytes_stream();
    loop {
        // A connection that goes quiet for 30 s is given up.
        let next = tokio::time::timeout(Duration::from_secs(30), body.next()).await;
        let chunk = match next {
            Err(_) => return Err(failed(format!("{url}: stalled at {got} bytes"))),
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(failed(e.to_string())),
            Ok(Some(Ok(c))) => c,
        };
        hash.update(&chunk);
        out.write_all(&chunk).map_err(|e| failed(e.to_string()))?;
        got += chunk.len() as u64;
        if got - told >= total / 100 || got == total {
            told = got;
            *PROGRESS.lock() = Some((spec.id, got, total));
            on_progress(got, total);
        }
    }
    out.flush().map_err(|e| failed(e.to_string()))?;
    drop(out);
    let hex = format!("{:x}", hash.finalize());
    if let Err(e) = verify(spec, got, &hex) {
        let _ = std::fs::remove_file(&part);
        crate::crash::write(
            "voice-model",
            &format!("{}: {got} bytes, sha256 {hex}", spec.file),
        );
        return Err(e);
    }
    std::fs::rename(&part, file).map_err(|e| failed(e.to_string()))?;
    std::fs::write(marker(file), spec.sha256).map_err(|e| failed(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_gets_the_english_model() {
        assert_eq!(spec_for("base", "en").id, "base.en-q5_1");
        assert_eq!(spec_for("base", "auto").id, "base-q5_1");
        assert_eq!(spec_for("tiny", "en").id, "tiny.en-q5_1");
        assert_eq!(spec_for("small", "fr").id, "small-q5_1");
        assert_eq!(spec_for("huge", "en").id, "base.en-q5_1");
        assert!(MODELS
            .iter()
            .chain(LEGACY_TINY.iter())
            .all(|m| m.sha256.len() == 64 && m.file.ends_with(".bin")));
        assert_eq!(
            legacy_for(spec_for("base", "en")).map(|l| l.id),
            Some("base.en")
        );
        assert_eq!(
            legacy_for(spec_for("tiny", "fr")).map(|l| l.id),
            Some("tiny")
        );
        assert_eq!(legacy_for(spec_for("small", "en")), None);
    }

    #[test]
    fn only_a_whole_checked_file_is_ready() {
        let spec = Spec {
            id: "t",
            file: "t.bin",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        let dir = std::env::temp_dir().join(format!("aeval-voice-model-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(spec.file);
        std::fs::write(&file, b"abc").unwrap();
        assert!(!ready_at(&file, &spec), "no record of the check yet");
        let hex = format!("{:x}", Sha256::digest(b"abc"));
        assert!(verify(&spec, 3, &hex).is_ok());
        assert_eq!(verify(&spec, 3, &"0".repeat(64)), Err(DAMAGED.to_string()));
        assert!(verify(&spec, 4, &hex).is_err());
        std::fs::write(marker(&file), spec.sha256).unwrap();
        assert!(ready_at(&file, &spec));
        std::fs::write(&file, b"abcd").unwrap();
        assert!(!ready_at(&file, &spec), "a file of the wrong size");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
