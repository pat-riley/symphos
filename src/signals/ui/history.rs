//! Keep preview time independent of render FPS and occasional missed frames.
use std::collections::VecDeque;

const SECONDS: f64 = 2.0;
const CAPACITY: usize = 512;

#[derive(Default)]
pub(super) struct History {
    samples: VecDeque<(f64, f32)>,
}
impl History {
    pub fn push(&mut self, time: f64, value: f32) {
        if let Some(&(previous, _)) = self.samples.back() {
            if time == previous {
                return;
            }
            if time < previous {
                self.samples.clear();
            }
        }
        self.samples.push_back((time, value));
        // Keep one preceding point for the clipped segment at the left edge.
        while self.samples.len() > CAPACITY
            || self
                .samples
                .get(1)
                .is_some_and(|(t, _)| *t < time - SECONDS)
        {
            self.samples.pop_front();
        }
    }
    pub fn points(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        let now = self.samples.back().map_or(0.0, |s| s.0);
        self.samples
            .iter()
            .map(move |&(time, value)| [((time - now + SECONDS) / SECONDS) as f32, value])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missed_frames_do_not_kink_a_linear_signal() {
        for fps in [30, 60, 120] {
            let mut history = History::default();
            for i in 0..=fps * 3 {
                let time = i as f64 / fps as f64;
                // Simulate a 100 ms UI hitch while the signal keeps running.
                if (2.0..2.1).contains(&time) {
                    continue;
                }
                history.push(time, time as f32 * 0.25);
            }
            for [x, value] in history.points() {
                assert!((value - (1.0 + 2.0 * x) * 0.25).abs() < 1e-6);
            }
            assert_eq!(history.points().last(), Some([1.0, 0.75]));
        }
    }
    #[test]
    fn history_is_bounded_and_resets_with_the_source_clock() {
        let mut history = History::default();
        for i in 0..10_000 {
            history.push(i as f64 / 1000.0, 0.5);
        }
        assert!(history.samples.len() <= CAPACITY);
        history.push(20.0, 0.8);
        assert_eq!(history.samples.len(), 2);
        history.push(0.0, 0.2);
        history.push(0.0, 0.2);
        assert_eq!(history.points().collect::<Vec<_>>(), vec![[1.0, 0.2]]);
    }
}
