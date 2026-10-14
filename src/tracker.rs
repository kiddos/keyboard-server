use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// In-memory tracker for keypress metrics such as real-time keypresses per minute (KPM).
#[derive(Debug, Clone, Default)]
pub struct KeyTracker {
    timestamps: Arc<Mutex<VecDeque<DateTime<Utc>>>>,
}

impl KeyTracker {
    pub fn new() -> Self {
        Self {
            timestamps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Record a keypress at the current timestamp
    pub fn record_keypress(&self) {
        self.record_at(Utc::now());
    }

    /// Record a keypress at a specific timestamp
    pub fn record_at(&self, time: DateTime<Utc>) {
        let mut ts = self.timestamps.lock().unwrap();
        if ts.back().map_or(true, |&last| time >= last) {
            ts.push_back(time);
        } else {
            // Out-of-order insertion fallback: sort after push
            ts.push_back(time);
            ts.make_contiguous().sort();
        }
        self.cleanup_locked(&mut ts, Utc::now());
    }

    /// Get number of keypresses recorded in the last 60 seconds
    pub fn get_kpm(&self) -> usize {
        let mut ts = self.timestamps.lock().unwrap();
        self.cleanup_locked(&mut ts, Utc::now());
        ts.len()
    }

    fn cleanup_locked(&self, ts: &mut VecDeque<DateTime<Utc>>, now: DateTime<Utc>) {
        let window_start = now - chrono::Duration::seconds(60);
        while let Some(&front) = ts.front() {
            if front < window_start {
                ts.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_tracker_kpm() {
        let tracker = KeyTracker::new();
        assert_eq!(tracker.get_kpm(), 0);

        tracker.record_keypress();
        tracker.record_keypress();
        tracker.record_keypress();
        assert_eq!(tracker.get_kpm(), 3);

        let old_time = Utc::now() - chrono::Duration::seconds(61);
        tracker.record_at(old_time);
        // Old time should be sorted and cleaned up
        assert_eq!(tracker.get_kpm(), 3);
    }
}
