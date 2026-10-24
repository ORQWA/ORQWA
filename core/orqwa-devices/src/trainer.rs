//! The protocol boundary for controllable trainers sharing a Bluetooth transport.

use orqwa_domain::telemetry::TrainerControl;

/// Encodes a trainer's controls independently of Bluetooth discovery and reconnection.
pub trait TrainerDriver: Send + Sync {
    /// Encodes a control as the characteristic writes needed by this protocol, in order.
    fn encode_control(&self, control: &TrainerControl) -> Vec<Vec<u8>>;
}

impl TrainerDriver for crate::ftms::ResistanceRange {
    fn encode_control(&self, control: &TrainerControl) -> Vec<Vec<u8>> {
        vec![crate::ftms::encode_control(control, *self)]
    }
}
