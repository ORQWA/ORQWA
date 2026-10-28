//! Bounded group ride networking (R159–R161, ADR 0035).
//!
//! Pasted codes are untrusted input. Destination policy is checked when decoding and again
//! before any network operation. LAN and loopback access require separate explicit choices.

pub mod code;
pub mod limits;
pub mod policy;
pub mod state;
