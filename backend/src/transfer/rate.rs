use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateSample {
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TransferRateEstimator {
    last_sample_at: Instant,
    last_bytes: u64,
    smoothed_speed: Option<f64>,
}

impl TransferRateEstimator {
    pub fn new(now: Instant, initial_bytes: u64) -> Self {
        Self {
            last_sample_at: now,
            last_bytes: initial_bytes,
            smoothed_speed: None,
        }
    }

    pub fn reset(&mut self, now: Instant, current_bytes: u64) {
        self.last_sample_at = now;
        self.last_bytes = current_bytes;
        self.smoothed_speed = None;
    }

    pub fn observe(&mut self, now: Instant, current_bytes: u64, total_bytes: u64) -> RateSample {
        let elapsed_secs = now
            .saturating_duration_since(self.last_sample_at)
            .as_secs_f64();
        if elapsed_secs >= 0.01 {
            let delta_bytes = current_bytes.saturating_sub(self.last_bytes);
            let instantaneous = delta_bytes as f64 / elapsed_secs;
            self.smoothed_speed = Some(match self.smoothed_speed {
                Some(prev) => 0.25 * instantaneous + 0.75 * prev,
                None => instantaneous,
            });
            self.last_sample_at = now;
            self.last_bytes = current_bytes;
        }

        let speed = self.smoothed_speed.unwrap_or(0.0).max(0.0).round() as u64;
        let eta = if speed > 0 && total_bytes > current_bytes {
            Some((total_bytes - current_bytes) / speed)
        } else if total_bytes > 0 && current_bytes >= total_bytes {
            Some(0)
        } else {
            None
        };

        RateSample {
            speed_bytes_per_sec: speed,
            eta_seconds: eta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_estimator_ewma_and_saturating_sub() {
        let t0 = Instant::now();
        let mut est = TransferRateEstimator::new(t0, 0);

        // t1: 1s, 100 bytes -> 100 B/s
        let t1 = t0 + Duration::from_secs(1);
        let sample1 = est.observe(t1, 100, 1000);
        assert_eq!(sample1.speed_bytes_per_sec, 100);
        assert_eq!(sample1.eta_seconds, Some(9)); // 900 / 100

        // t2: 2s, 300 bytes (delta 200 bytes in 1s) -> instantaneous 200 B/s
        // smoothed = 0.25 * 200 + 0.75 * 100 = 50 + 75 = 125 B/s
        let t2 = t1 + Duration::from_secs(1);
        let sample2 = est.observe(t2, 300, 1000);
        assert_eq!(sample2.speed_bytes_per_sec, 125);
        assert_eq!(sample2.eta_seconds, Some(5)); // (1000 - 300) / 125 = 700 / 125 = 5

        // Counter reset / drop (saturating_sub protects against underflow)
        let t3 = t2 + Duration::from_secs(1);
        let sample3 = est.observe(t3, 50, 1000);
        // delta is 0 (saturating_sub) -> instantaneous 0 B/s
        // smoothed = 0.25 * 0 + 0.75 * 125 = 93.75 -> 94 B/s
        assert_eq!(sample3.speed_bytes_per_sec, 94);
    }
}
