//! Server-sent events, read the way `readSse` in lib/openrouter.js reads them
//! (PARITY PROMPT-05): bytes are split on `\n`, only complete lines count,
//! and only `data:` lines carry anything (keepalive comments such as
//! `: OPENROUTER PROCESSING` are skipped).

use std::time::Instant;

/// Bytes in, complete lines out. A multi-byte character split across two
/// chunks is joined before it is decoded, as `TextDecoder({stream: true})`
/// does.
#[derive(Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(at) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=at).collect();
            out.push(String::from_utf8_lossy(&line[..at]).into_owned());
        }
        out
    }

}

/// The payload of a `data:` line, trimmed; `None` for any other line.
pub fn data_payload(raw: &str) -> Option<&str> {
    let line = raw.trim();
    line.strip_prefix("data:").map(str::trim)
}

/// Tokens and cost of one reply.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub prompt: u64,
    pub completion: u64,
    pub cost: Option<f64>,
}

/// A per-request record of how long the first word took and what it cost
/// (`newMeter` in lib/openrouter.js).
#[derive(Clone, Debug)]
pub struct Meter {
    pub t0: Instant,
    pub ttft_ms: u64,
    pub prompt: u64,
    pub completion: u64,
    pub cost: Option<f64>,
}

impl Default for Meter {
    fn default() -> Self {
        Meter {
            t0: Instant::now(),
            ttft_ms: 0,
            prompt: 0,
            completion: 0,
            cost: None,
        }
    }
}

impl Meter {
    pub fn take(&mut self, usage: &Usage) {
        self.prompt += usage.prompt;
        self.completion += usage.completion;
        if let Some(c) = usage.cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + c);
        }
    }

    /// Called on the first content delta.
    pub fn first_token(&mut self) {
        if self.ttft_ms == 0 {
            self.ttft_ms = self.elapsed_ms();
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_survive_any_split() {
        let text = "data: {\"a\":\"é\"}\n\n: keepalive\ndata: [DONE]\n";
        let bytes = text.as_bytes();
        for cut in 1..bytes.len() {
            let mut b = LineBuffer::default();
            let mut lines = b.push(&bytes[..cut]);
            lines.extend(b.push(&bytes[cut..]));
            let want = ["data: {\"a\":\"é\"}", "", ": keepalive", "data: [DONE]"];
            assert_eq!(lines, want);
        }
    }

    #[test]
    fn only_data_lines_count() {
        assert_eq!(data_payload("  data:  [DONE] "), Some("[DONE]"));
        assert_eq!(data_payload(": OPENROUTER PROCESSING"), None);
        assert_eq!(data_payload("event: ping"), None);
    }

    #[test]
    fn meter_adds_usage_and_cost() {
        let mut m = Meter::default();
        m.take(&Usage {
            prompt: 10,
            completion: 2,
            cost: None,
        });
        assert_eq!(m.cost, None);
        m.take(&Usage {
            prompt: 1,
            completion: 1,
            cost: Some(0.5),
        });
        assert_eq!((m.prompt, m.completion, m.cost), (11, 3, Some(0.5)));
    }
}
