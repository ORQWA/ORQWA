//! Tacx FE-C over Bluetooth, used by the NEO 2 Smart and other non-FTMS Tacx trainers.
//!
//! Wire format: Tacx's “How-to FE-C over BLE” v1.0.0 (2015-02-17). Complete ANT
//! messages travel over the Tacx UART service; this needs no ANT radio or USB dongle.

use std::time::Duration;

use orqwa_domain::telemetry::{Telemetry, TrainerControl};
use orqwa_domain::units::{MetersPerSecond, Rpm, Watts};

use crate::trainer::TrainerDriver;

/// Tacx FE-C UART service.
pub const SERVICE: &str = "6e40fec1-b5a3-f393-e0a9-e50e24dcca9e";
/// Trainer-to-app notifications.
pub const DATA: &str = "6e40fec2-b5a3-f393-e0a9-e50e24dcca9e";
/// App-to-trainer control writes.
pub const CONTROL: &str = "6e40fec3-b5a3-f393-e0a9-e50e24dcca9e";
/// Tacx recommends sending one ANT message at the native 4 Hz rate.
pub const CONTROL_INTERVAL: Duration = Duration::from_millis(250);

/// Stateless FE-C control encoder; Bluetooth owns the connection and command pacing.
pub struct Tacx;

impl TrainerDriver for Tacx {
    // Every u8 conversion is capped at a maximum <= 254 by `scaled`.
    #[allow(clippy::cast_possible_truncation)]
    fn encode_control(&self, control: &TrainerControl) -> Vec<Vec<u8>> {
        let mut page = [0xff; 8];
        match control {
            TrainerControl::Simulation(p) => {
                if ![p.grade.0, p.wind_speed.0, p.cw.0, p.crr]
                    .iter()
                    .all(|v| v.is_finite())
                {
                    return self.encode_control(&TrainerControl::Resistance(
                        orqwa_domain::units::Percent(0.0),
                    ));
                }
                page[0] = 0x32;
                page[5] = scaled(p.cw.0, 100.0, 186) as u8;
                page[6] = scaled(p.wind_speed.0 * 3.6 + 127.0, 1.0, 254) as u8;
                page[7] = 100; // no drafting
                let wind = frame(page);
                page = [0xff; 8];
                page[0] = 0x33;
                page[5..7].copy_from_slice(&scaled(p.grade.0 + 200.0, 100.0, 40000).to_le_bytes());
                page[7] = scaled(p.crr, 20000.0, 254) as u8;
                vec![wind, frame(page)]
            }
            TrainerControl::TargetPower(power) => {
                page[0] = 0x31;
                page[6..8].copy_from_slice(&scaled(power.0, 4.0, 65534).to_le_bytes());
                vec![frame(page)]
            }
            TrainerControl::Resistance(share) => {
                page[0] = 0x30;
                page[7] = scaled(share.0, 2.0, 200) as u8;
                vec![frame(page)]
            }
        }
    }
}

/// The latest control, repeated at 4 Hz; replacing it discards any unsent old mode.
#[derive(Default)]
pub(crate) struct Controls {
    commands: Vec<Vec<u8>>,
    index: usize,
}

impl Controls {
    pub(crate) fn is_active(&self) -> bool {
        !self.commands.is_empty()
    }

    pub(crate) fn replace(&mut self, commands: Vec<Vec<u8>>) {
        // Frequent grade updates must not starve the second simulation page.
        if commands.len() != self.commands.len() {
            self.index = 0;
        }
        self.commands = commands;
    }

    pub(crate) fn next(&mut self) -> &[u8] {
        let command = &self.commands[self.index];
        self.index = (self.index + 1) % self.commands.len();
        command
    }
}

// Bounds exclude FE-C's invalid-value sentinels.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scaled(value: f64, scale: f64, max: u16) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    (value * scale).round().clamp(0.0, f64::from(max)) as u16
}

fn frame(page: [u8; 8]) -> Vec<u8> {
    // Channel 5 is Tacx's tunnel, not an ANT radio channel opened by ORQWA.
    let mut bytes = vec![0xa4, 9, 0x4f, 5];
    bytes.extend(page);
    bytes.push(bytes.iter().fold(0, |checksum, byte| checksum ^ byte));
    bytes
}

/// A malformed message on the Tacx UART service.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// A complete ANT data message must contain thirteen bytes.
    #[error("invalid FE-C frame length: {0}")]
    Length(usize),
    /// The sync, payload length, message type or checksum is invalid.
    #[error("invalid FE-C frame header or checksum")]
    Frame,
}

/// Validates a complete ANT data message and returns its eight-byte FE-C page.
///
/// # Errors
/// Returns [`ParseError`] for truncated or corrupt messages.
pub fn parse_page(bytes: &[u8]) -> Result<&[u8], ParseError> {
    if bytes.len() != 13 {
        return Err(ParseError::Length(bytes.len()));
    }
    if bytes[0] != 0xa4
        || bytes[1] != 9
        || !matches!(bytes[2], 0x4e | 0x4f)
        || bytes.iter().fold(0, |checksum, byte| checksum ^ byte) != 0
    {
        return Err(ParseError::Frame);
    }
    Ok(&bytes[4..12])
}

/// Decodes general FE data (page 16) and trainer power/cadence (page 25).
/// Unknown pages and unavailable values contribute no measurements.
///
/// # Errors
/// Returns [`ParseError`] for truncated or corrupt messages.
pub fn parse_telemetry(bytes: &[u8]) -> Result<Option<Telemetry>, ParseError> {
    let page = parse_page(bytes)?;
    let telemetry = match page[0] {
        0x10 => {
            let speed = u16::from_le_bytes([page[4], page[5]]);
            Telemetry {
                speed: (speed != u16::MAX).then(|| MetersPerSecond(f64::from(speed) * 0.001)),
                ..Telemetry::default()
            }
        }
        0x19 => {
            let power = u16::from_le_bytes([page[5], page[6]]) & 0x0fff;
            Telemetry {
                power: (power != 0x0fff).then(|| Watts(f64::from(power))),
                cadence: (page[2] != 0xff).then(|| Rpm(f64::from(page[2]))),
                ..Telemetry::default()
            }
        }
        _ => return Ok(None),
    };
    Ok((telemetry != Telemetry::default()).then_some(telemetry))
}

#[cfg(test)]
mod tests {
    use orqwa_domain::telemetry::SimulationParameters;
    use orqwa_domain::units::{GradePercent, KilogramsPerMeter, Percent};

    use super::*;

    #[test]
    fn latest_controls_replace_old_modes_without_starving_simulation_pages() {
        let mut controls = Controls::default();
        assert!(!controls.is_active());
        let simulation = TrainerControl::Simulation(SimulationParameters::default());
        controls.replace(Tacx.encode_control(&simulation));
        assert_eq!(parse_page(controls.next()).unwrap()[0], 0x32);
        controls.replace(Tacx.encode_control(&simulation));
        assert_eq!(parse_page(controls.next()).unwrap()[0], 0x33);
        controls.replace(Tacx.encode_control(&TrainerControl::TargetPower(Watts(200.0))));
        for _ in 0..4 {
            assert_eq!(parse_page(controls.next()).unwrap()[0], 0x31);
        }
        // Reconnection starts a new queue from the last desired control.
        let mut reconnected = Controls::default();
        reconnected.replace(Tacx.encode_control(&simulation));
        assert_eq!(parse_page(reconnected.next()).unwrap()[0], 0x32);
    }

    #[test]
    fn simulation_limits_and_non_finite_controls_remain_valid() {
        for (grade, raw) in [(-300.0, 0_u16), (300.0, 40000_u16)] {
            let commands = Tacx.encode_control(&TrainerControl::Simulation(SimulationParameters {
                grade: GradePercent(grade),
                wind_speed: MetersPerSecond(-100.0),
                crr: 1.0,
                cw: KilogramsPerMeter(10.0),
            }));
            assert_eq!(&parse_page(&commands[0]).unwrap()[5..], &[186, 0, 100]);
            let track = parse_page(&commands[1]).unwrap();
            assert_eq!(&track[5..7], &raw.to_le_bytes());
            assert_eq!(track[7], 254);
        }
        let invalid = TrainerControl::Simulation(SimulationParameters {
            grade: GradePercent(f64::NAN),
            ..SimulationParameters::default()
        });
        assert_eq!(
            Tacx.encode_control(&invalid),
            Tacx.encode_control(&TrainerControl::Resistance(Percent(0.0)))
        );
    }

    #[test]
    fn erg_uses_quarter_watts_and_ant_checksum() {
        // Independently specified 100 W FE-C page, with acknowledged data on channel 5.
        assert_eq!(
            Tacx.encode_control(&TrainerControl::TargetPower(Watts(100.0))),
            vec![vec![
                0xa4, 9, 0x4f, 5, 0x31, 0xff, 0xff, 0xff, 0xff, 0xff, 0x90, 1, 0xb8
            ]]
        );
    }

    #[test]
    fn simulation_carries_wind_grade_and_rolling_resistance() {
        let commands = Tacx.encode_control(&TrainerControl::Simulation(SimulationParameters {
            grade: GradePercent(-5.0),
            wind_speed: MetersPerSecond(5.0),
            cw: KilogramsPerMeter(0.51),
            crr: 0.004,
        }));
        assert_eq!(
            parse_page(&commands[0]).unwrap(),
            &[0x32, 255, 255, 255, 255, 51, 145, 100]
        );
        assert_eq!(
            parse_page(&commands[1]).unwrap(),
            &[0x33, 255, 255, 255, 255, 0x2c, 0x4c, 80]
        );
    }

    #[test]
    fn controls_clamp_before_encoding_and_never_emit_unavailable_sentinels() {
        for (share, raw) in [(-1.0, 0), (50.0, 100), (100.0, 200), (120.0, 200)] {
            let commands = Tacx.encode_control(&TrainerControl::Resistance(Percent(share)));
            assert_eq!(
                parse_page(&commands[0]).unwrap(),
                &[0x30, 255, 255, 255, 255, 255, 255, raw]
            );
        }
        let commands = Tacx.encode_control(&TrainerControl::TargetPower(Watts(20000.0)));
        assert_eq!(&parse_page(&commands[0]).unwrap()[6..], &[0xfe, 0xff]);
    }

    #[test]
    fn power_is_twelve_bits_and_pages_merge_without_erasing_other_measurements() {
        let mut telemetry = parse_telemetry(&frame([0x19, 1, 90, 0, 0, 250, 0xa0, 0x30]))
            .unwrap()
            .unwrap();
        assert_eq!(telemetry.power, Some(Watts(250.0)));
        assert_eq!(telemetry.cadence, Some(Rpm(90.0)));
        telemetry.merge(
            &parse_telemetry(&frame([0x10, 0x19, 0, 0, 0x10, 0x27, 255, 0x30]))
                .unwrap()
                .unwrap(),
        );
        assert_eq!(telemetry.speed, Some(MetersPerSecond(10.0)));
        assert_eq!(telemetry.power, Some(Watts(250.0)));
    }

    #[test]
    fn unavailable_is_not_zero_and_non_telemetry_pages_are_ignored() {
        assert_eq!(
            parse_telemetry(&frame([0x19, 1, 255, 0, 0, 255, 255, 0x30])),
            Ok(None)
        );
        assert_eq!(
            parse_telemetry(&frame([0x10, 0x19, 0, 0, 255, 255, 255, 0x30])),
            Ok(None)
        );
        assert_eq!(parse_telemetry(&frame([0x50; 8])), Ok(None));
        let zero = parse_telemetry(&frame([0x19, 1, 0, 0, 0, 0, 0, 0x30]))
            .unwrap()
            .unwrap();
        assert_eq!(zero.power, Some(Watts(0.0)));
        assert_eq!(zero.cadence, Some(Rpm(0.0)));
    }

    #[test]
    fn malformed_packets_are_rejected() {
        let valid = frame([0x19, 1, 90, 0, 0, 250, 0, 0x30]);
        for len in 0..valid.len() {
            assert!(parse_telemetry(&valid[..len]).is_err());
        }
        for i in 0..valid.len() {
            let mut corrupt = valid.clone();
            corrupt[i] ^= 1;
            assert!(parse_telemetry(&corrupt).is_err());
        }
        let mut broadcast = valid;
        broadcast[2] = 0x4e;
        broadcast[12] ^= 1;
        assert!(parse_telemetry(&broadcast).unwrap().is_some());
    }
}
