//! Connection abuse protection, separate from disposable media delivery budgets.

use std::time::{Duration, Instant};

use crate::state::MAX_RIDERS;
use crate::tls::IDLE_TIMEOUT;

// Twice a full room's 50 Hz speech plus 5 Hz state. A single delayed batch is
// disposable congestion; only repeated overloaded windows prove sustained excess.
const PACKETS_PER_SECOND: usize = MAX_RIDERS * 110;
const OVERLOADED_WINDOWS: u8 = 3;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Flood;

pub(crate) struct DatagramIngress {
    window: Instant,
    packets: usize,
    overloaded: u8,
}

impl DatagramIngress {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            window: now,
            packets: 0,
            overloaded: 0,
        }
    }

    pub(crate) fn accept(&mut self, now: Instant) -> Result<bool, Flood> {
        let seconds = now.saturating_duration_since(self.window).as_secs();
        if seconds > 0 {
            self.overloaded = if seconds == 1 && self.packets > PACKETS_PER_SECOND {
                self.overloaded.saturating_add(1)
            } else {
                0
            };
            self.window += Duration::from_secs(seconds);
            self.packets = 0;
        }
        self.packets = self.packets.saturating_add(1);
        if self.packets <= PACKETS_PER_SECOND {
            return Ok(true);
        }
        if self.overloaded >= OVERLOADED_WINDOWS - 1 {
            return Err(Flood);
        }
        Ok(false)
    }
}

// An organizer's reliable controls to a rider in an app lobby, at most 64 a second. A riding
// group sends about six a second (five workout clocks, one latency report). A rider's
// connection may stall until its idle timeout, when everything sent meanwhile arrives at
// once (#698), so up to eight a second of the stall may wait.
const CONTROL_EVERY: Duration = Duration::from_micros(1_000_000 / 64);
#[allow(clippy::cast_possible_truncation)] // the idle timeout is 30 s
const CONTROL_BACKLOG: u32 = 8 * IDLE_TIMEOUT.as_secs() as u32;

/// Rate limit for one connection's controls: the allowance builds up while quiet, up to
/// [`CONTROL_BACKLOG`], and refills at 64 a second.
pub(crate) struct ControlIngress {
    // When the allowance is full again.
    full: Instant,
}

impl ControlIngress {
    pub(crate) fn new(now: Instant) -> Self {
        Self { full: now }
    }

    pub(crate) fn accept(&mut self, now: Instant) -> Result<(), Flood> {
        let full = self.full.max(now) + CONTROL_EVERY;
        if full > now + CONTROL_EVERY * CONTROL_BACKLOG {
            return Err(Flood);
        }
        self.full = full;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_held_up_by_a_stall_until_the_idle_timeout_arrive_together() {
        let now = Instant::now();
        let mut guard = ControlIngress::new(now);
        for second in 0..60 {
            for _ in 0..6 {
                assert_eq!(guard.accept(now + Duration::from_secs(second)), Ok(()));
            }
        }
        // #698: a riding group's controls from a stall as long as the idle timeout.
        let resumed = now + Duration::from_secs(60) + IDLE_TIMEOUT;
        for _ in 0..6 * IDLE_TIMEOUT.as_secs() {
            assert_eq!(guard.accept(resumed), Ok(()));
        }
    }

    #[test]
    fn a_flood_of_controls_is_rejected() {
        let now = Instant::now();
        let mut guard = ControlIngress::new(now);
        for _ in 0..CONTROL_BACKLOG {
            assert_eq!(guard.accept(now), Ok(()));
        }
        assert_eq!(guard.accept(now), Err(Flood));
        // Refilled at 64 a second, a sustained flood is rejected again within a second.
        let later = now + Duration::from_secs(1);
        let accepted = (0..1000)
            .take_while(|_| guard.accept(later).is_ok())
            .count();
        assert!((63..=65).contains(&accepted), "{accepted}");
    }

    #[test]
    fn a_large_delayed_batch_is_dropped_without_ending_the_connection() {
        let now = Instant::now();
        let mut guard = DatagramIngress::new(now);
        for packet in 0..PACKETS_PER_SECOND * 4 {
            assert_eq!(guard.accept(now), Ok(packet < PACKETS_PER_SECOND));
        }
        assert_eq!(guard.accept(now + Duration::from_secs(1)), Ok(true));
        assert_eq!(guard.accept(now + Duration::from_secs(2)), Ok(true));
    }

    #[test]
    fn sustained_excess_is_rejected_but_a_quiet_window_resets_it() {
        let now = Instant::now();
        let mut guard = DatagramIngress::new(now);
        for second in 0..2 {
            for _ in 0..=PACKETS_PER_SECOND {
                assert!(guard.accept(now + Duration::from_secs(second)).is_ok());
            }
        }
        let third = now + Duration::from_secs(2);
        for _ in 0..PACKETS_PER_SECOND {
            assert_eq!(guard.accept(third), Ok(true));
        }
        assert_eq!(guard.accept(third), Err(Flood));
        assert_eq!(guard.accept(now + Duration::from_secs(4)), Ok(true));
    }

    #[test]
    fn a_full_room_keeps_its_connections_through_periodic_delivery_bursts() {
        let now = Instant::now();
        let mut guard = DatagramIngress::new(now);
        for batch in 0..100 {
            let at = now + Duration::from_millis(batch * 500);
            for _ in 0..MAX_RIDERS * 28 {
                assert_eq!(guard.accept(at), Ok(true));
            }
        }
    }

    #[test]
    fn overloads_separated_by_normal_traffic_do_not_accumulate_toward_disconnection() {
        let now = Instant::now();
        let mut guard = DatagramIngress::new(now);
        for second in 0..60 {
            let count = if second % 3 == 2 {
                400
            } else {
                PACKETS_PER_SECOND + 100
            };
            for packet in 0..count {
                assert_eq!(
                    guard.accept(now + Duration::from_secs(second)),
                    Ok(packet < PACKETS_PER_SECOND)
                );
            }
        }
    }
}
