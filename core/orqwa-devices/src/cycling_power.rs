//! Bluetooth Cycling Power Service, as power pedals and crank power meters report it.

use std::time::{Duration, Instant};

use orqwa_domain::units::{Rpm, Watts};

use crate::bytes::{ParseError, Reader};

/// Cycling Power service.
pub const SERVICE: u16 = 0x1818;
/// Cycling Power Measurement characteristic (notify).
pub const MEASUREMENT: u16 = 0x2A63;

const PEDAL_POWER_BALANCE: u16 = 1 << 0;
const ACCUMULATED_TORQUE: u16 = 1 << 2;
const WHEEL_REVOLUTIONS: u16 = 1 << 4;
const CRANK_REVOLUTIONS: u16 = 1 << 5;

/// The crank's revolution counter as a meter reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrankRevolutions {
    /// Revolutions since the meter started counting; wraps at 65 536.
    pub revolutions: u16,
    /// When the last revolution ended, in 1/1024 s; wraps every 64 s.
    pub event_time: u16,
}

/// A decoded Cycling Power Measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// Instantaneous power.
    pub power: Watts,
    /// The crank's counter, if the meter knows its cadence.
    pub crank: Option<CrankRevolutions>,
}

/// Decodes a Cycling Power Measurement.
///
/// # Errors
/// Returns [`ParseError`] if the value is shorter than its flags announce.
pub fn parse_measurement(data: &[u8]) -> Result<Measurement, ParseError> {
    let mut r = Reader::new(data);
    let flags = r.u16()?;
    let power = Watts(f64::from(r.i16()?));
    if flags & PEDAL_POWER_BALANCE != 0 {
        r.skip(1)?;
    }
    if flags & ACCUMULATED_TORQUE != 0 {
        r.skip(2)?;
    }
    if flags & WHEEL_REVOLUTIONS != 0 {
        r.skip(6)?;
    }
    let crank = if flags & CRANK_REVOLUTIONS == 0 {
        None
    } else {
        Some(CrankRevolutions {
            revolutions: r.u16()?,
            event_time: r.u16()?,
        })
    };
    Ok(Measurement { power, crank })
}

/// How long the crank counter may stand still before the rider counts as coasting. Meters
/// report about once a second and repeat the counter between revolutions, which at 30 rpm
/// end two seconds apart.
const COASTING_AFTER: Duration = Duration::from_millis(2500);
/// Faster than anyone pedals: a jump this large means the counter was misread or reset.
const MAX_CADENCE: f64 = 250.0;

/// Works out the cadence from successive crank counters, which is all the Cycling Power
/// Service gives (it has no cadence field).
#[derive(Debug, Clone, Default)]
pub struct Cadence {
    last: Option<CrankRevolutions>,
    /// When the counter last moved.
    last_turn: Option<Instant>,
    rpm: Option<Rpm>,
    /// The counter stood still long enough to count as coasting.
    coasting: bool,
}

impl Cadence {
    /// Takes the counter of a measurement received `at`; returns the cadence, `None` until two
    /// counters have come.
    pub fn update(&mut self, crank: CrankRevolutions, at: Instant) -> Option<Rpm> {
        let Some(last) = self.last.replace(crank) else {
            self.last_turn = Some(at);
            return None;
        };
        let revolutions = crank.revolutions.wrapping_sub(last.revolutions);
        let ticks = crank.event_time.wrapping_sub(last.event_time);
        if revolutions == 0 {
            let still = self.last_turn.map_or(Duration::ZERO, |t| at - t);
            if still >= COASTING_AFTER {
                self.coasting = true;
                self.rpm = Some(Rpm(0.0));
            }
            return self.rpm;
        }
        self.last_turn = Some(at);
        // The first revolution after coasting ended long after the last counter, perhaps more
        // than the 64 s the event time spans: the next one tells the cadence.
        if std::mem::take(&mut self.coasting) {
            return self.rpm;
        }
        if ticks > 0 {
            let rpm = f64::from(revolutions) * 60.0 * 1024.0 / f64::from(ticks);
            if rpm <= MAX_CADENCE {
                self.rpm = Some(Rpm(rpm));
            }
        }
        self.rpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crank(revolutions: u16, event_time: u16) -> CrankRevolutions {
        CrankRevolutions {
            revolutions,
            event_time,
        }
    }

    #[test]
    fn decodes_power_alone() {
        let m = parse_measurement(&[0x00, 0x00, 0xFA, 0x00]).unwrap();
        assert_eq!(m.power, Watts(250.0));
        assert_eq!(m.crank, None);
    }

    #[test]
    fn decodes_crank_revolutions_behind_the_balance() {
        // Balance (bit 0) and crank revolutions (bit 5), as dual-sided pedals send them.
        let data = [0x21, 0x00, 0x2C, 0x01, 0x64, 0x0A, 0x00, 0x00, 0x04];
        let m = parse_measurement(&data).unwrap();
        assert_eq!(m.power, Watts(300.0));
        assert_eq!(m.crank, Some(crank(10, 0x0400)));
    }

    #[test]
    fn skips_torque_and_wheel_data_before_the_crank() {
        let data = [
            0x34, 0x00, 0x96, 0x00, // flags: torque, wheel, crank; 150 W
            0x11, 0x22, // accumulated torque
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, // wheel revolutions and time
            0x07, 0x00, 0x00, 0x08, // crank revolutions and time
        ];
        assert_eq!(
            parse_measurement(&data).unwrap().crank,
            Some(crank(7, 0x0800))
        );
    }

    #[test]
    fn negative_power_is_kept_signed() {
        assert_eq!(
            parse_measurement(&[0x00, 0x00, 0xFF, 0xFF]).unwrap().power,
            Watts(-1.0)
        );
    }

    #[test]
    fn truncated_crank_data_is_an_error() {
        assert!(parse_measurement(&[0x20, 0x00, 0x2C, 0x01, 0x0A, 0x00]).is_err());
    }

    #[test]
    fn cadence_comes_from_revolutions_over_time() {
        let start = Instant::now();
        let mut cadence = Cadence::default();
        assert_eq!(cadence.update(crank(10, 0), start), None);
        let rpm = cadence.update(crank(11, 1024), start + Duration::from_secs(1));
        assert_eq!(rpm, Some(Rpm(60.0)));
        let rpm = cadence.update(crank(14, 3072), start + Duration::from_secs(3));
        assert_eq!(rpm, Some(Rpm(90.0)));
    }

    #[test]
    fn counters_wrapping_around_keep_the_cadence() {
        let start = Instant::now();
        let mut cadence = Cadence::default();
        cadence.update(crank(u16::MAX, u16::MAX - 511), start);
        // Two revolutions in 1024 ticks across both wraps: 120 rpm.
        let rpm = cadence.update(crank(1, 512), start + Duration::from_secs(1));
        assert_eq!(rpm, Some(Rpm(120.0)));
    }

    #[test]
    fn a_repeated_counter_keeps_the_cadence_until_the_rider_coasts() {
        let start = Instant::now();
        let mut cadence = Cadence::default();
        cadence.update(crank(0, 0), start);
        cadence.update(crank(1, 1024), start + Duration::from_secs(1));
        // Between two slow revolutions the meter repeats its counter.
        let rpm = cadence.update(crank(1, 1024), start + Duration::from_secs(2));
        assert_eq!(rpm, Some(Rpm(60.0)));
        let rpm = cadence.update(crank(1, 1024), start + Duration::from_secs(4));
        assert_eq!(rpm, Some(Rpm(0.0)));
        // The first revolution after coasting comes long after the last one.
        let rpm = cadence.update(crank(2, 14_000), start + Duration::from_secs(90));
        assert_eq!(rpm, Some(Rpm(0.0)));
        let rpm = cadence.update(crank(4, 15_024), start + Duration::from_secs(91));
        assert_eq!(rpm, Some(Rpm(120.0)), "pedalling again after coasting");
    }

    #[test]
    fn an_impossible_jump_is_ignored() {
        let start = Instant::now();
        let mut cadence = Cadence::default();
        cadence.update(crank(0, 0), start);
        cadence.update(crank(1, 1024), start + Duration::from_secs(1));
        let rpm = cadence.update(crank(500, 2048), start + Duration::from_secs(2));
        assert_eq!(rpm, Some(Rpm(60.0)));
    }
}
