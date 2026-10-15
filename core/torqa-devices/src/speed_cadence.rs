//! Bluetooth Cycling Speed and Cadence Service, as cadence sensors report it (R72).

use crate::bytes::{ParseError, Reader};
use crate::cycling_power::CrankRevolutions;

/// Cycling Speed and Cadence service.
pub const SERVICE: u16 = 0x1816;
/// CSC Measurement characteristic (notify).
pub const MEASUREMENT: u16 = 0x2A5B;

const WHEEL_REVOLUTIONS: u8 = 1 << 0;
const CRANK_REVOLUTIONS: u8 = 1 << 1;

/// Decodes a CSC Measurement's crank counter; `None` from a speed-only sensor.
///
/// # Errors
/// Returns [`ParseError`] if the value is shorter than its flags announce.
pub fn parse_measurement(data: &[u8]) -> Result<Option<CrankRevolutions>, ParseError> {
    let mut r = Reader::new(data);
    let flags = r.u8()?;
    if flags & WHEEL_REVOLUTIONS != 0 {
        r.skip(6)?;
    }
    if flags & CRANK_REVOLUTIONS == 0 {
        return Ok(None);
    }
    Ok(Some(CrankRevolutions {
        revolutions: r.u16()?,
        event_time: r.u16()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_crank_counter() {
        assert_eq!(
            parse_measurement(&[0x02, 0x0A, 0x00, 0x00, 0x04]).unwrap(),
            Some(CrankRevolutions {
                revolutions: 10,
                event_time: 0x0400,
            })
        );
    }

    #[test]
    fn skips_wheel_data_before_the_crank() {
        let data = [0x03, 1, 2, 3, 4, 5, 6, 0x07, 0x00, 0x00, 0x08];
        assert_eq!(
            parse_measurement(&data).unwrap(),
            Some(CrankRevolutions {
                revolutions: 7,
                event_time: 0x0800,
            })
        );
    }

    #[test]
    fn a_speed_sensor_has_no_cadence() {
        assert_eq!(parse_measurement(&[0x01, 1, 2, 3, 4, 5, 6]).unwrap(), None);
    }

    #[test]
    fn truncated_crank_data_is_an_error() {
        assert!(parse_measurement(&[0x02, 0x0A, 0x00]).is_err());
    }
}
