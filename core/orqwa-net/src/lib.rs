//! Bounded group ride networking (R159–R161, ADR 0035).
//!
//! Pasted codes are untrusted input. Destination policy is checked when decoding and again
//! before any network operation. LAN and loopback access require separate explicit choices.

pub mod code;
pub mod crypto;
pub mod discovery;
pub mod limits;
pub mod link;
pub mod lobby;
pub mod policy;
mod runtime;
pub mod state;
pub mod stun;
mod text;
mod tls;
pub mod voice;
mod wire;
