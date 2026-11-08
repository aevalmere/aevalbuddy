//! What AevalBuddy knows about each provider right now (design-ux section
//! 3.2): ready, busy until a time, key rejected, no credit, free models off.
//! In memory only, so a restart starts every provider as unchecked.
//! Failover skips the providers that cannot answer (B-046), and Settings and
//! the overlay show the same words (`provider:status`).
//!
//! An entry belongs to the key it was learned about: a replaced key starts
//! fresh. Only a fingerprint of the key is kept, never the key.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use super::provider::Provider;

/// How long a 429 without `Retry-After` keeps a provider marked busy.
pub const BUSY_DEFAULT_MS: u64 = 60_000;
/// How long a 402 waits before the key is tested again in the background.
pub const NO_CREDIT_RETEST_MS: u64 = 10 * 60_000;

/// The state words Settings and the overlay show for a provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    NotAdded,
    Checking,
    Ready,
    Rejected,
    NoCredit,
    Busy,
    /// OpenRouter's privacy setting refuses free models.
    Blocked,
    #[default]
    Unchecked,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::NotAdded => "not-added",
            Status::Checking => "checking",
            Status::Ready => "ready",
            Status::Rejected => "rejected",
            Status::NoCredit => "no-credit",
            Status::Busy => "busy",
            Status::Blocked => "blocked",
            Status::Unchecked => "unchecked",
        }
    }
}

/// One provider's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub status: Status,
    /// The sentence shown beside the status.
    pub text: String,
    /// When the key was last checked, ms since the epoch.
    pub checked_at: Option<u64>,
    /// Busy until this time, ms since the epoch.
    pub busy_until: Option<u64>,
    /// No credit: when to test the key again without asking.
    pub retest_at: Option<u64>,
    /// OpenRouter credit left, in dollars, when the key check reported it.
    pub credit: Option<f64>,
    /// Fingerprint of the key this entry describes.
    pub key: u64,
}

/// A fingerprint of a key, so an entry can tell a replaced key from the one
/// it describes without keeping the key.
pub fn fingerprint(key: &str) -> u64 {
    let digest = Sha256::digest(key.trim().as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(first)
}

/// Milliseconds since the epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Every provider's entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    entries: HashMap<Provider, Entry>,
}

impl Table {
    /// The entry for `provider` as it applies to `key`: unchecked when the
    /// entry was about another key, or there is none.
    pub fn get(&self, provider: Provider, key: &str) -> Entry {
        match self.entries.get(&provider) {
            Some(e) if e.key == fingerprint(key) => e.clone(),
            _ => Entry {
                key: fingerprint(key),
                ..Entry::default()
            },
        }
    }

    /// Whether failover may send a request to `provider` with `key` now.
    /// `paid_model` is true when the model picked on OpenRouter bills per
    /// token: OpenRouter's "no credit" then skips it, and "free models off"
    /// no longer does.
    pub fn usable(&self, provider: Provider, key: &str, now: u64, paid_model: bool) -> bool {
        let e = self.get(provider, key);
        match e.status {
            Status::Rejected => false,
            Status::NoCredit => provider == Provider::OpenRouter && !paid_model,
            Status::Blocked => provider != Provider::OpenRouter || paid_model,
            Status::Busy => match e.busy_until {
                Some(t) => now >= t,
                None => true,
            },
            _ => true,
        }
    }

    /// Sets an entry. Returns true when what people see changed.
    pub fn set(&mut self, provider: Provider, key: &str, entry: Entry) -> bool {
        let entry = Entry {
            key: fingerprint(key),
            ..entry
        };
        let before = self.entries.insert(provider, entry.clone());
        match before {
            Some(b) => b.status != entry.status || b.text != entry.text,
            None => true,
        }
    }

    /// A 429: busy for `Retry-After`, or a minute.
    pub fn mark_busy(
        &mut self,
        provider: Provider,
        key: &str,
        retry_after_ms: Option<u64>,
        now: u64,
        text: &str,
    ) -> bool {
        let prev = self.get(provider, key);
        let until = now + retry_after_ms.unwrap_or(BUSY_DEFAULT_MS);
        self.set(
            provider,
            key,
            Entry {
                status: Status::Busy,
                text: text.to_string(),
                busy_until: Some(until),
                ..prev
            },
        )
    }

    /// A 401: skipped until the key changes or a test passes.
    pub fn mark_rejected(&mut self, provider: Provider, key: &str, text: &str) -> bool {
        self.set(
            provider,
            key,
            Entry {
                status: Status::Rejected,
                text: text.to_string(),
                ..Entry::default()
            },
        )
    }

    /// A 402: skipped, and tested again quietly after ten minutes.
    pub fn mark_no_credit(&mut self, provider: Provider, key: &str, now: u64, text: &str) -> bool {
        self.set(
            provider,
            key,
            Entry {
                status: Status::NoCredit,
                text: text.to_string(),
                checked_at: Some(now),
                retest_at: Some(now + NO_CREDIT_RETEST_MS),
                ..Entry::default()
            },
        )
    }

    /// A key test or an answer went through.
    pub fn mark_ready(
        &mut self,
        provider: Provider,
        key: &str,
        now: u64,
        text: &str,
        credit: Option<f64>,
    ) -> bool {
        self.set(
            provider,
            key,
            Entry {
                status: Status::Ready,
                text: text.to_string(),
                checked_at: Some(now),
                credit,
                ..Entry::default()
            },
        )
    }

    /// Native providers whose "no credit" mark is due for a quiet re-test.
    pub fn due_for_retest(&self, now: u64) -> Vec<Provider> {
        let mut due: Vec<Provider> = self
            .entries
            .iter()
            .filter(|(_, e)| e.status == Status::NoCredit)
            .filter(|(_, e)| e.retest_at.is_some_and(|t| now >= t))
            .map(|(p, _)| *p)
            .collect();
        due.sort_by_key(|p| Provider::ALL.iter().position(|x| x == p));
        due
    }
}

fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(Table::default()))
}

/// The live table, copied.
pub fn snapshot() -> Table {
    table().lock().clone()
}

/// Changes the live table. Returns what `f` returned (true when the change
/// is one people should see).
pub fn update<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    f(&mut table().lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_key_starts_fresh() {
        let mut t = Table::default();
        t.mark_rejected(Provider::Anthropic, "sk-ant-old", "no");
        assert!(!t.usable(Provider::Anthropic, "sk-ant-old", 0, false));
        assert!(t.usable(Provider::Anthropic, "sk-ant-new", 0, false));
        let fresh = t.get(Provider::Anthropic, "sk-ant-new");
        assert_eq!(fresh.status, Status::Unchecked);
    }

    #[test]
    fn fingerprints_are_stable_and_distinct() {
        assert_eq!(fingerprint("sk-or-a"), fingerprint(" sk-or-a "));
        assert_ne!(fingerprint("sk-or-a"), fingerprint("sk-or-b"));
    }

    #[test]
    fn no_credit_is_due_for_a_retest_after_ten_minutes() {
        let mut t = Table::default();
        t.mark_no_credit(Provider::OpenAi, "sk-proj-x", 1_000, "empty");
        assert!(t.due_for_retest(1_000 + NO_CREDIT_RETEST_MS - 1).is_empty());
        let due = t.due_for_retest(1_000 + NO_CREDIT_RETEST_MS);
        assert_eq!(due, vec![Provider::OpenAi]);
    }
}
