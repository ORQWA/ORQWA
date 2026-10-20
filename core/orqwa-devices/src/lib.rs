//! Trainer and sensor drivers for ORQWA.
//!
//! Every device — Bluetooth trainer, power meter, cadence sensor, heart-rate strap, shifter or
//! the fake trainer — is driven by a background task and exposed through the same
//! [`DeviceHandle`], so callers never depend on a concrete driver.

#[cfg(target_os = "android")]
pub mod android;
pub mod battery;
pub mod ble;
mod bytes;
pub mod cycling_power;
pub mod di2;
pub mod fake;
pub mod ftms;
mod handle;
pub mod heart_rate;
pub mod shift;
pub mod speed_cadence;
pub mod zwift_click;
pub mod zwift_click_v2;

pub use bytes::ParseError;
pub use handle::{DeviceError, DeviceEvent, DeviceHandle};
