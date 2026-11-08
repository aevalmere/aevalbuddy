//! Electron `safeStorage` on Windows, read and written the same way
//! (PARITY STORE-40, ARCHITECTURE.md D5).
//!
//! safeStorage is Chromium's OSCrypt. Its key is 32 random bytes kept in the
//! user-data folder's `Local State` file as `os_crypt.encrypted_key`: base64
//! of `DPAPI` followed by a CryptProtectData blob. A sealed string is `v10`,
//! a 12-byte nonce, then AES-256-GCM ciphertext with its 16-byte tag. Blobs
//! with no `v10` prefix are the pre-OSCrypt format: DPAPI over the plain
//! bytes. Rust reads both and writes `v10`, so a rollback to the Electron
//! build still reads the key.

use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use serde_json::{Map, Value};

const PREFIX: &[u8] = b"v10";
const DPAPI_PREFIX: &[u8] = b"DPAPI";
const NONCE_LEN: usize = 12;

/// The OSCrypt key for one user-data folder, if it could be read or made.
pub struct OsCrypt {
    key: Option<[u8; 32]>,
}

impl OsCrypt {
    /// Reads the key from `<user_data>/Local State`, creating the file (or the
    /// `os_crypt` entry) with a new key when there is none, as Chromium does
    /// on a fresh profile.
    pub fn load_or_create(user_data: &Path) -> Self {
        let file = user_data.join("Local State");
        let mut root = read_json(&file);
        if let Some(key) = key_from_local_state(&root) {
            return OsCrypt { key: Some(key) };
        }
        // An existing key we cannot unwrap belongs to another Windows user
        // or a reinstalled OS. Replacing it would make the blobs written
        // under it unreadable for good, so it is left alone.
        if encrypted_key_field(&root).is_some() {
            return OsCrypt { key: None };
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let Some(wrapped) = dpapi::protect(&key) else {
            return OsCrypt { key: None };
        };
        let mut field = DPAPI_PREFIX.to_vec();
        field.extend_from_slice(&wrapped);
        let os_crypt = root
            .entry("os_crypt")
            .or_insert_with(|| Value::Object(Map::new()));
        if !os_crypt.is_object() {
            *os_crypt = Value::Object(Map::new());
        }
        if let Some(obj) = os_crypt.as_object_mut() {
            obj.insert("encrypted_key".into(), Value::String(B64.encode(field)));
        }
        if write_json(&file, &root).is_err() {
            return OsCrypt { key: None };
        }
        OsCrypt { key: Some(key) }
    }

    /// A ring with a known key, for tests.
    #[cfg(test)]
    pub fn with_key(key: [u8; 32]) -> Self {
        OsCrypt { key: Some(key) }
    }

    /// `safeStorage.isEncryptionAvailable()`.
    pub fn available(&self) -> bool {
        self.key.is_some()
    }

    /// `safeStorage.encryptString`.
    pub fn encrypt(&self, plain: &str) -> Option<Vec<u8>> {
        let key = self.key?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);
        seal(&key, &nonce, plain)
    }

    /// `safeStorage.decryptString`. None when the blob is not ours.
    pub fn decrypt(&self, blob: &[u8]) -> Option<String> {
        if blob.starts_with(PREFIX) {
            return open(&self.key?, blob);
        }
        let plain = dpapi::unprotect(blob)?;
        String::from_utf8(plain).ok()
    }
}

/// `v10 | nonce | AES-256-GCM(plain) | tag`.
pub fn seal(key: &[u8; 32], nonce: &[u8; NONCE_LEN], plain: &str) -> Option<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let sealed = cipher
        .encrypt(Nonce::from_slice(nonce), plain.as_bytes())
        .ok()?;
    let mut out = PREFIX.to_vec();
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    Some(out)
}

/// The inverse of `seal`.
pub fn open(key: &[u8; 32], blob: &[u8]) -> Option<String> {
    let rest = blob.strip_prefix(PREFIX)?;
    if rest.len() < NONCE_LEN + 16 {
        return None;
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let plain = cipher.decrypt(Nonce::from_slice(nonce), sealed).ok()?;
    String::from_utf8(plain).ok()
}

fn encrypted_key_field(root: &Map<String, Value>) -> Option<&str> {
    root.get("os_crypt")?.get("encrypted_key")?.as_str()
}

fn key_from_local_state(root: &Map<String, Value>) -> Option<[u8; 32]> {
    let field = B64.decode(encrypted_key_field(root)?).ok()?;
    let wrapped = field.strip_prefix(DPAPI_PREFIX)?;
    let key = dpapi::unprotect(wrapped)?;
    key.try_into().ok()
}

fn read_json(file: &Path) -> Map<String, Value> {
    fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

fn write_json(file: &Path, root: &Map<String, Value>) -> std::io::Result<()> {
    let text = serde_json::to_string(root)?;
    let tmp = PathBuf::from(format!("{}.tmp", file.display()));
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&tmp, text)?;
    fs::rename(&tmp, file)
}

/// Windows DPAPI, scoped to the current user.
mod dpapi {
    use std::ptr;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if out.pbData.is_null() {
            return Vec::new();
        }
        // SAFETY: DPAPI returned `cbData` bytes at `pbData`, freed below.
        let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        // SAFETY: the buffer was allocated by DPAPI with LocalAlloc.
        unsafe { LocalFree(out.pbData as _) };
        bytes
    }

    pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
        let input = blob(data);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        // SAFETY: input points at live bytes; out receives a LocalAlloc buffer.
        let ok = unsafe {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        (ok != 0).then(|| take(out))
    }

    pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
        let input = blob(data);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        // SAFETY: as in `protect`.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        (ok != 0).then(|| take(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Made with node's crypto: AES-256-GCM, key 32 x 0x01, nonce 12 x 0x02,
    // plaintext "sk-or-v1-test-key", laid out as OSCrypt does.
    const NODE_BLOB: &str = "djEwAgICAgICAgICAgICdL3kJjh6t8z+uNm7KIqp19uWTpS/vppoltLHijoRuv+A";

    #[test]
    fn opens_a_blob_sealed_by_node() {
        let blob = B64.decode(NODE_BLOB).unwrap();
        assert_eq!(
            open(&[1u8; 32], &blob).as_deref(),
            Some("sk-or-v1-test-key")
        );
    }

    #[test]
    fn seals_byte_for_byte_like_chromium() {
        let blob = seal(&[1u8; 32], &[2u8; 12], "sk-or-v1-test-key").unwrap();
        assert_eq!(B64.encode(blob), NODE_BLOB);
    }

    #[test]
    fn wrong_key_or_garbage_is_none_not_a_panic() {
        let blob = B64.decode(NODE_BLOB).unwrap();
        assert_eq!(open(&[9u8; 32], &blob), None);
        assert_eq!(open(&[1u8; 32], b"v10short"), None);
        assert_eq!(OsCrypt::with_key([1u8; 32]).decrypt(b"not a blob"), None);
    }

    #[test]
    fn round_trip_through_the_ring() {
        let ring = OsCrypt::with_key([7u8; 32]);
        let blob = ring.encrypt("sk-or-abc").unwrap();
        assert!(blob.starts_with(b"v10"));
        assert_eq!(ring.decrypt(&blob).as_deref(), Some("sk-or-abc"));
    }

    #[test]
    fn dpapi_round_trip() {
        let wrapped = dpapi::protect(b"secret").unwrap();
        assert_eq!(dpapi::unprotect(&wrapped).unwrap(), b"secret");
        // A legacy (pre-v10) safeStorage blob is DPAPI over the plain bytes.
        let ring = OsCrypt { key: None };
        assert_eq!(ring.decrypt(&wrapped).as_deref(), Some("secret"));
    }

    #[test]
    fn local_state_is_created_then_reused() {
        let dir = std::env::temp_dir().join(format!("aeval-crypto-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Chromium keeps other things in Local State; they must survive.
        fs::write(dir.join("Local State"), r#"{"browser":{"x":1}}"#).unwrap();
        let first = OsCrypt::load_or_create(&dir);
        assert!(first.available());
        let blob = first.encrypt("k").unwrap();
        let second = OsCrypt::load_or_create(&dir);
        assert_eq!(second.decrypt(&blob).as_deref(), Some("k"));
        let root = read_json(&dir.join("Local State"));
        assert_eq!(root["browser"]["x"], 1);
        let field = B64.decode(encrypted_key_field(&root).unwrap()).unwrap();
        assert!(field.starts_with(b"DPAPI"));
        let _ = fs::remove_dir_all(&dir);
    }
}
