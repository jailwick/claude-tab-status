//! When the plugin's one timer fires: every [`SAMPLE_INTERVAL`] while a tab works (to follow the
//! title's spinner), and every `refresh_interval` for re-reading all titles. zellij's `set_timeout` can't be cancelled, so at most
//! one is pending at a time.

use std::time::Duration;

/// How often a working pane's title is re-read: half of Claude's (and jailwick's) 960 ms spinner
/// frame, so the tab never skips or repeats a frame, and lags the title by half a frame at most.
pub const SAMPLE_INTERVAL: Duration = Duration::from_millis(480);

#[derive(Debug, Default)]
pub struct Ticker {
    refresh: Option<Duration>,
    /// Time since the titles were last re-read, counted in fired timeouts.
    since_refresh: Duration,
    /// The pending timeout's delay.
    pending: Option<Duration>,
}

impl Ticker {
    pub fn new(refresh: Option<Duration>) -> Self {
        Self {
            refresh,
            ..Self::default()
        }
    }

    /// The timeout to set now, if one is needed and none is pending: a sample while `animating`,
    /// else the refresh interval, else none (nothing runs).
    pub fn arm(&mut self, animating: bool) -> Option<Duration> {
        if self.pending.is_some() {
            return None;
        }
        let delay = if animating {
            SAMPLE_INTERVAL
        } else {
            self.refresh?
        };
        self.pending = Some(delay);
        Some(delay)
    }

    /// The pending timeout fired. Returns whether the titles are due for a re-read.
    pub fn fire(&mut self) -> bool {
        let elapsed = self.pending.take().unwrap_or_default();
        let Some(refresh) = self.refresh else {
            return false;
        };
        self.since_refresh += elapsed;
        let due = self.since_refresh >= refresh;
        if due {
            self.since_refresh = Duration::ZERO;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_without_refresh_never_runs() {
        let mut ticker = Ticker::new(None);
        assert_eq!(ticker.arm(false), None);
    }

    #[test]
    fn samples_while_working() {
        let mut ticker = Ticker::new(None);
        assert_eq!(ticker.arm(true), Some(SAMPLE_INTERVAL));
        assert_eq!(ticker.arm(true), None, "one timeout at a time");
        assert!(!ticker.fire());
        assert_eq!(ticker.arm(true), Some(SAMPLE_INTERVAL));
        ticker.fire();
        assert_eq!(ticker.arm(false), None, "stops once no tab works");
    }

    #[test]
    fn refreshes_at_its_own_pace() {
        let refresh = Duration::from_secs(2);
        let mut ticker = Ticker::new(Some(refresh));
        assert_eq!(ticker.arm(false), Some(refresh));
        assert!(ticker.fire());
    }

    #[test]
    fn refresh_rides_on_samples_without_changing_their_pace() {
        let mut ticker = Ticker::new(Some(Duration::from_secs(1)));
        let due: Vec<bool> = (0..6)
            .map(|_| {
                assert_eq!(ticker.arm(true), Some(SAMPLE_INTERVAL));
                ticker.fire()
            })
            .collect();
        // 0.48 s, 0.96 s, 1.44 s (due), 0.48, 0.96, 1.44 (due)
        assert_eq!(due, [false, false, true, false, false, true]);
    }

    #[test]
    fn short_refresh_happens_every_sample_while_animating() {
        let mut ticker = Ticker::new(Some(Duration::from_millis(300)));
        assert_eq!(ticker.arm(true), Some(SAMPLE_INTERVAL));
        assert!(ticker.fire());
    }
}
