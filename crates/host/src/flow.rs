//! Per-window consumption credit and conservative rate adaptation. Independent
//! implementation informed by RustDesk's decode feedback / VideoQoS design.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub fps: u32,
    pub bitrate: u32,
}

pub struct Flow {
    outstanding: VecDeque<(u64, u64, Instant)>,
    ceiling: Option<Rate>,
    rate: Option<Rate>,
    baseline_ms: f64,
    sampled: Instant,
    changed: Instant,
    last_feedback: Option<Instant>,
    sums: [f64; 3],
    samples: u32,
    local_bad: u8,
    network_bad: u8,
    good: u8,
    reason: &'static str,
}
impl Default for Flow {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            outstanding: VecDeque::new(),
            ceiling: None,
            rate: None,
            baseline_ms: f64::INFINITY,
            sampled: now,
            changed: now,
            last_feedback: None,
            sums: [0.; 3],
            samples: 0,
            local_bad: 0,
            network_bad: 0,
            good: 0,
            reason: "stable",
        }
    }
}
impl Flow {
    fn reset_samples(&mut self, now: Instant) {
        self.sampled = now;
        self.sums = [0.; 3];
        self.samples = 0;
        self.local_bad = 0;
        self.network_bad = 0;
        self.good = 0;
    }
    pub fn set_ceiling(&mut self, rate: Option<Rate>, now: Instant) {
        self.ceiling = rate;
        self.rate = rate;
        self.reset_samples(now);
        self.baseline_ms = f64::INFINITY;
        self.last_feedback = None;
        self.changed = now;
        self.reason = "stable";
        // Outstanding old-epoch frames retain their credits until consumed.
    }
    /// Resizing changes the pixel budget, not the user's quality choice. Preserve
    /// existing congestion reductions instead of resetting to full rate each time.
    pub fn resize_ceiling(&mut self, cap: Rate, now: Instant) -> bool {
        if self.ceiling == Some(cap) {
            return false;
        }
        let old = self.rate;
        self.rate = Some(match (self.ceiling, self.rate) {
            (Some(previous), Some(rate)) => Rate {
                fps: rate.fps.min(cap.fps),
                bitrate: ((u64::from(rate.bitrate) * u64::from(cap.bitrate)
                    / u64::from(previous.bitrate.max(1))) as u32)
                    .clamp(250_000.min(cap.bitrate), cap.bitrate),
            },
            _ => cap,
        });
        self.ceiling = Some(cap);
        self.reset_samples(now);
        self.changed = now;
        old != self.rate
    }
    pub fn rate(&self) -> Option<Rate> {
        self.rate
    }
    pub fn ceiling(&self) -> Option<Rate> {
        self.ceiling
    }
    pub fn reason(&self) -> &'static str {
        self.reason
    }
    pub fn ready(&self) -> bool {
        self.outstanding.len() < 2
    }
    pub fn sent(&mut self, epoch: u64, sequence: u64, now: Instant) {
        assert!(self.ready());
        self.outstanding.push_back((epoch, sequence, now));
    }
    pub fn consumed(
        &mut self,
        epoch: u64,
        sequence: u64,
        active_epoch: u64,
        decode_ms: f64,
        queue_ms: f64,
        now: Instant,
    ) -> Result<bool, &'static str> {
        if !decode_ms.is_finite()
            || !queue_ms.is_finite()
            || !(0.0..=60_000.0).contains(&decode_ms)
            || !(0.0..=60_000.0).contains(&queue_ms)
        {
            return Err("Invalid playback feedback");
        }
        let index = self
            .outstanding
            .iter()
            .position(|(e, s, _)| *e == epoch && *s == sequence)
            .ok_or("Unknown playback frame")?;
        let (_, _, sent) = self.outstanding.remove(index).unwrap();
        if epoch != active_epoch {
            return Ok(false);
        }
        let (Some(cap), Some(mut rate)) = (self.ceiling, self.rate) else {
            return Ok(false);
        };
        if self
            .last_feedback
            .is_some_and(|last| now.saturating_duration_since(last) > Duration::from_secs(5))
        {
            self.reset_samples(now);
            self.baseline_ms = f64::INFINITY;
        }
        self.last_feedback = Some(now);
        let elapsed = now.saturating_duration_since(sent).as_secs_f64() * 1000.;
        let transit = (elapsed - decode_ms - queue_ms).max(0.);
        self.baseline_ms = self.baseline_ms.min(transit);
        self.sums[0] += decode_ms;
        self.sums[1] += queue_ms;
        self.sums[2] += transit;
        self.samples += 1;
        if now.saturating_duration_since(self.sampled) < Duration::from_secs(1) {
            return Ok(false);
        }
        let n = f64::from(self.samples);
        let local_busy =
            self.sums[0] / n > 1000. / f64::from(rate.fps) * 0.85 || self.sums[1] / n > 80.;
        let congested = self.sums[2] / n - self.baseline_ms > 100.;
        self.sums = [0.; 3];
        self.samples = 0;
        self.sampled = now;
        if local_busy {
            self.local_bad = self.local_bad.saturating_add(1);
            self.network_bad = 0;
            self.good = 0;
        } else if congested {
            self.network_bad = self.network_bad.saturating_add(1);
            self.local_bad = 0;
            self.good = 0;
        } else {
            self.good = self.good.saturating_add(1);
            self.local_bad = 0;
            self.network_bad = 0;
        }
        let cooldown = now.saturating_duration_since(self.changed);
        if cooldown < Duration::from_secs(4) {
            return Ok(false);
        }
        if self.local_bad >= 3 {
            // Decoder, compositor and UI congestion cannot be cured by blurring text.
            rate.fps = (rate.fps * 4 / 5).max(5).min(cap.fps);
            self.reason = "client_busy";
            self.local_bad = 0;
        } else if self.network_bad >= 4 {
            // Reduce traffic by sending fewer frames while preserving bits/frame.
            let next_fps = (rate.fps * 4 / 5).max(10.min(cap.fps)).min(rate.fps);
            let nominal =
                (u64::from(cap.bitrate) * u64::from(next_fps) / u64::from(cap.fps.max(1))) as u32;
            let floor = (nominal * 4 / 5).max(250_000).min(cap.bitrate);
            rate.bitrate = if next_fps < rate.fps {
                rate.bitrate.min(nominal).max(floor)
            } else {
                (rate.bitrate * 9 / 10).max(floor)
            };
            rate.fps = next_fps;
            self.reason = "delivery_backlog";
            self.network_bad = 0;
        } else if self.good >= 6 && cooldown >= Duration::from_secs(8) {
            let next_fps = (rate.fps + (cap.fps / 10).max(1)).min(cap.fps);
            let required =
                (u64::from(cap.bitrate) * u64::from(next_fps) / u64::from(cap.fps.max(1))) as u32;
            rate.bitrate = (rate.bitrate + (cap.bitrate / 5).max(1)).min(cap.bitrate);
            if rate.bitrate >= required {
                rate.fps = next_fps;
            }
            self.reason = if rate == cap { "stable" } else { "recovering" };
            self.good = 0;
        }
        if Some(rate) != self.rate {
            self.rate = Some(rate);
            self.changed = now;
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CAP: Rate = Rate {
        fps: 30,
        bitrate: 6_000_000,
    };
    fn report(f: &mut Flow, t: Instant, n: u64, decode: f64, queue: f64, transit: f64) {
        let now = t + Duration::from_secs(n);
        f.sent(
            1,
            n,
            now - Duration::from_secs_f64((decode + queue + transit) / 1000.),
        );
        f.consumed(1, n, 1, decode, queue, now).unwrap();
    }
    #[test]
    fn credits_reject_forged_duplicate_and_invalid_feedback() {
        let mut f = Flow::default();
        let t = Instant::now();
        f.sent(1, 1, t);
        f.sent(1, 2, t);
        assert!(!f.ready());
        assert!(f.consumed(1, 3, 1, 0., 0., t).is_err());
        assert!(f.consumed(1, 1, 1, f64::NAN, 0., t).is_err());
        assert!(!f.ready());
        assert_eq!(f.consumed(1, 1, 2, 0., 0., t), Ok(false));
        assert!(f.ready());
        assert!(f.consumed(1, 1, 2, 0., 0., t).is_err());
    }
    #[test]
    fn slow_decode_reduces_fps_then_recovers_under_user_ceiling() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=4 {
            report(&mut f, t, n, 60., 0., 20.);
        }
        assert!(f.rate().unwrap().fps < 30);
        assert_eq!(f.rate().unwrap().bitrate, CAP.bitrate);
        for n in 5..=90 {
            report(&mut f, t, n, 3., 0., 20.);
        }
        assert_eq!(f.rate(), Some(CAP));
        assert_eq!(f.reason(), "stable");
    }
    #[test]
    fn client_queue_never_reduces_bitrate_even_after_prolonged_pressure() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=100 {
            report(&mut f, t, n, 3., 160., 20.);
            assert_eq!(f.rate().unwrap().bitrate, CAP.bitrate);
        }
        assert_eq!(f.rate().unwrap().fps, 5);
        assert_eq!(f.reason(), "client_busy");
    }
    #[test]
    fn sustained_delivery_backlog_preserves_per_frame_budget_and_recovers() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=100 {
            report(&mut f, t, n, 3., 0., if n < 5 { 200. } else { 500. });
            let rate = f.rate().unwrap();
            if n < 8 {
                assert_eq!(rate, CAP);
            }
            assert!(
                u64::from(rate.bitrate) * u64::from(CAP.fps)
                    >= u64::from(CAP.bitrate) * u64::from(rate.fps) * 4 / 5
            );
        }
        assert_eq!(
            f.rate(),
            Some(Rate {
                fps: 10,
                bitrate: 1_600_000
            })
        );
        assert_eq!(f.reason(), "delivery_backlog");
        for n in 101..=180 {
            report(&mut f, t, n, 3., 0., 200.);
        }
        assert_eq!(f.rate(), Some(CAP));
    }
    #[test]
    fn isolated_spikes_are_averaged_and_mixed_causes_do_not_compound() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=300 {
            let now = t + Duration::from_millis(n * 40);
            let delay = if n % 25 == 0 { 500 } else { 20 };
            f.sent(1, n, now - Duration::from_millis(delay));
            f.consumed(1, n, 1, 2., 0., now).unwrap();
        }
        assert_eq!(f.rate(), Some(CAP));
        for n in 13..=30 {
            report(
                &mut f,
                t,
                n,
                3.,
                if n % 2 == 0 { 120. } else { 0. },
                if n % 2 == 0 { 20. } else { 400. },
            );
        }
        assert_eq!(f.rate(), Some(CAP));
    }
    #[test]
    fn resize_preserves_reduction_preset_resets_and_old_epoch_is_not_sampled() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=8 {
            report(&mut f, t, n, 3., 0., if n < 5 { 20. } else { 400. });
        }
        let reduced = f.rate().unwrap();
        assert!(reduced.bitrate < CAP.bitrate);
        assert!(f.resize_ceiling(
            Rate {
                fps: 30,
                bitrate: 12_000_000
            },
            t + Duration::from_secs(9)
        ));
        assert_eq!(f.rate().unwrap().bitrate, reduced.bitrate * 2);
        assert_eq!(f.rate().unwrap().fps, reduced.fps);
        f.sent(1, 99, t);
        f.consumed(1, 99, 2, 300., 300., t + Duration::from_secs(10))
            .unwrap();
        assert_eq!(f.rate().unwrap().bitrate, reduced.bitrate * 2);
        f.set_ceiling(Some(CAP), t + Duration::from_secs(11));
        assert_eq!(f.rate(), Some(CAP));
    }
    #[test]
    fn idle_gap_does_not_combine_stale_bad_samples() {
        let t = Instant::now();
        let mut f = Flow::default();
        f.set_ceiling(Some(CAP), t);
        for n in 1..=2 {
            report(&mut f, t, n, 80., 0., 20.);
        }
        report(&mut f, t, 30, 80., 0., 20.);
        assert_eq!(f.rate(), Some(CAP));
        for n in 31..=40 {
            report(&mut f, t, n, 3., 0., 20.);
        }
        assert_eq!(f.rate(), Some(CAP));
    }
}
