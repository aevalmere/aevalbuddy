//! The two seams split (b) reaches the rest of the app through, so Find, the
//! agent and Ask compile and test without the desktop helper or a window
//! (ARCHITECTURE.md section 10).
//!
//! [`DesktopOps`] is lib/desktop/index.js as JSON: the method names and the
//! option objects are the ones main.js passed, and the replies are the JSON
//! split (c)'s `desktop::Desktop` returns. The adapter is one match over
//! `method` calling (c)'s typed methods; it lands with the IPC wiring.

use async_trait::async_trait;
use serde_json::{json, Value};

/// `DesktopError` from lib/desktop: a stable code and a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskError {
    pub code: String,
    pub message: String,
}

impl DeskError {
    pub fn new(code: &str, message: impl Into<String>) -> DeskError {
        DeskError {
            code: code.to_string(),
            message: message.into(),
        }
    }

    // For the smoke harness.
    #[allow(dead_code)]
    pub fn to_json(&self) -> Value {
        json!({ "code": self.code, "message": self.message })
    }
}

impl std::fmt::Display for DeskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The desktop engine. `method` is an index.js export name (`listWindows`,
/// `foregroundWindow`, `focusWindow`, `closeWindow`, `readTree`, `find`,
/// `getText`, `screenshot`, `zoom`, `elementAt`, `resolve`, `click`,
/// `hover`, `drag`, `type`, `key`, `scroll`, `scrollTo`, `setValue`,
/// `invoke`, `restoreCursor`, `windowStates`); `args` is the one options
/// object it took (positional arguments go under their parameter names).
#[async_trait]
pub trait DesktopOps: Send + Sync {
    async fn op(&self, method: &str, args: Value) -> Result<Value, DeskError>;
}

/// Sends an event to the windows that render it (`emit`/`sendToWindow`).
pub trait Emitter: Send + Sync {
    fn emit(&self, channel: &str, payload: Value);
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::HashMap;

    /// Canned desktop replies by method, and every call made.
    #[derive(Default)]
    pub struct FakeDesktop {
        pub replies: Mutex<HashMap<String, Result<Value, DeskError>>>,
        pub calls: Mutex<Vec<(String, Value)>>,
        /// Methods that take this many milliseconds to answer.
        pub delays: Mutex<HashMap<String, u64>>,
    }

    impl FakeDesktop {
        pub fn reply(&self, method: &str, value: Value) {
            self.replies.lock().insert(method.to_string(), Ok(value));
        }

        pub fn slow(&self, method: &str, ms: u64) {
            self.delays.lock().insert(method.to_string(), ms);
        }

        pub fn fail(&self, method: &str, code: &str) {
            let err = DeskError::new(code, code);
            self.replies.lock().insert(method.to_string(), Err(err));
        }

        pub fn methods(&self) -> Vec<String> {
            self.calls.lock().iter().map(|(m, _)| m.clone()).collect()
        }
    }

    #[async_trait]
    impl DesktopOps for FakeDesktop {
        async fn op(&self, method: &str, args: Value) -> Result<Value, DeskError> {
            self.calls.lock().push((method.to_string(), args));
            let delay = self.delays.lock().get(method).copied();
            if let Some(ms) = delay {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
            let reply = self.replies.lock().get(method).cloned();
            reply.unwrap_or(Ok(Value::Null))
        }
    }

    /// Every event emitted, in order.
    #[derive(Default)]
    pub struct Recorder {
        pub events: Mutex<Vec<(String, Value)>>,
    }

    impl Recorder {
        pub fn last(&self, channel: &str) -> Option<Value> {
            let events = self.events.lock();
            let hit = events.iter().rev().find(|(c, _)| c == channel);
            hit.map(|(_, v)| v.clone())
        }
    }

    impl Emitter for Recorder {
        fn emit(&self, channel: &str, payload: Value) {
            self.events.lock().push((channel.to_string(), payload));
        }
    }
}
