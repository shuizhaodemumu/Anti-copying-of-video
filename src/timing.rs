//! Rates here describe GTK's frame callbacks, not a measurement of panel scan-out.
use std::collections::VecDeque;

const WINDOW: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Refresh,
    Duplicate,
    Stalled,
}

#[derive(Default)]
pub struct Cadence {
    previous: Option<i64>,
    intervals: VecDeque<i64>,
}

impl Cadence {
    pub fn observe(&mut self, now: i64) -> Tick {
        let Some(previous) = self.previous else {
            self.previous = Some(now);
            return Tick::Refresh;
        };
        let elapsed = now - previous;
        // Wayland may dispatch an extra animation callback for a UI invalidation
        // within the same refresh. Keep that refresh's texture and time origin.
        if (0..6_000).contains(&elapsed) {
            return Tick::Duplicate;
        }
        self.previous = Some(now);
        if elapsed < 0 || elapsed > 100_000 {
            self.intervals.clear();
            return Tick::Stalled;
        }
        if self.intervals.len() == WINDOW {
            self.intervals.pop_front();
        }
        self.intervals.push_back(elapsed);
        if elapsed > 45_000 {
            Tick::Stalled
        } else {
            Tick::Refresh
        }
    }

    pub fn hz(&self) -> Option<f64> {
        (self.intervals.len() == WINDOW)
            .then(|| 1_000_000.0 * WINDOW as f64 / self.intervals.iter().sum::<i64>() as f64)
    }

    pub fn suitable(&self, monitor_hz: Option<f64>) -> bool {
        let acceptable = |hz: f64| (57.0..=63.0).contains(&hz);
        monitor_hz.map(acceptable).unwrap_or(true)
            && self.hz().is_some_and(acceptable)
            && self
                .intervals
                .iter()
                .filter(|&&us| !(12_500..=21_500).contains(&us))
                .count()
                <= 2
    }
}

// A single lost presentation can spoil one pair, but should not end a whole recording.
// A stall or a monitor change ends the experiment immediately.
#[derive(Default)]
pub struct Stability {
    bad_since: Option<i64>,
}

impl Stability {
    pub fn stop(&mut self, now: i64, tick: Tick, suitable: bool, monitor_hz: Option<f64>) -> bool {
        if tick == Tick::Stalled || monitor_hz.is_some_and(|hz| !(57.0..=63.0).contains(&hz)) {
            return true;
        }
        if suitable {
            self.bad_since = None;
            return false;
        }
        now - *self.bad_since.get_or_insert(now) >= 750_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_rate(period: i64) -> Cadence {
        let mut clock = Cadence::default();
        for i in 0..=WINDOW {
            clock.observe(i as i64 * period);
        }
        clock
    }

    #[test]
    fn accepts_5994_and_rejects_wrong_monitor_and_unsteady_callbacks() {
        assert!(at_rate(16_683).suitable(Some(59.94)));
        assert!(!at_rate(16_667).suitable(Some(120.0)));
        assert!(!at_rate(8_333).suitable(None));
        assert!(!at_rate(33_333).suitable(None));
        let mut clock = Cadence::default();
        let mut now = 0;
        for i in 0..=WINDOW {
            now += if i % 2 == 0 { 23_334 } else { 10_000 };
            clock.observe(now);
        }
        assert!(!clock.suitable(None));
    }

    #[test]
    fn duplicate_wayland_callbacks_do_not_advance_the_pair() {
        let mut clock = Cadence::default();
        let mut now = 0;
        for _ in 0..35 {
            assert_eq!(clock.observe(now), Tick::Refresh);
            assert_eq!(clock.observe(now + 2_000), Tick::Duplicate);
            now += 16_667;
        }
        assert!(clock.suitable(Some(60.0)));
        assert!(!at_rate(8_333).suitable(None));
    }

    #[test]
    fn occasional_jitter_recovers_but_sustained_faults_stop() {
        let mut guard = Stability::default();
        assert!(!guard.stop(0, Tick::Refresh, false, Some(60.0)));
        assert!(!guard.stop(310_000, Tick::Refresh, true, Some(60.0)));
        assert!(!guard.stop(900_000, Tick::Refresh, false, Some(60.0)));
        assert!(guard.stop(1_660_000, Tick::Refresh, false, Some(60.0)));
        assert!(guard.stop(1_670_000, Tick::Stalled, true, None));
        assert!(guard.stop(1_680_000, Tick::Refresh, true, Some(120.0)));
    }

    #[test]
    fn resuming_after_a_stall_requires_a_new_sample() {
        let mut clock = at_rate(16_667);
        assert_eq!(clock.observe(2_000_000), Tick::Stalled);
        assert!(!clock.suitable(None));
    }
}
