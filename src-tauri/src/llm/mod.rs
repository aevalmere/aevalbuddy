//! Talking to models: the curated free-model table, the four providers
//! (OpenRouter, Anthropic, OpenAI, Gemini) behind one request shape,
//! failover, streaming, prompts and error text. Port of lib/openrouter.js
//! and lib/fake-llm.js plus the model half of main.js (PARITY MODEL-*,
//! FAIL-*, PROMPT-*, POINT-*, ERR-*). ARCHITECTURE.md section 10 documents
//! the seams.

// Nothing calls into this module until ipc.rs routes (b)'s channels; the
// allow comes off when the wiring lands.
#![allow(dead_code)]

pub mod catalogue;
pub mod client;
pub mod elements;
pub mod errors;
pub mod failover;
pub mod fake;
pub mod points;
pub mod prompt;
pub mod provider;
pub mod settings;
pub mod sse;
pub mod wire;
