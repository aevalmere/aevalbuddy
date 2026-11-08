//! The desktop agent: plan, act through the Desktop seam, confirm risky
//! steps, and report a trail (PARITY AGENT-*). Port of lib/agent.js.

// Nothing calls into this module until ipc.rs routes (b)'s channels.
#![allow(dead_code)]

pub mod seams;
