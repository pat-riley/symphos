//! Opt-in diagnostics window; fixed-size sampling adds no permanent UI chrome.
use egui::{self, Color32};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

/// Keep an absolute cadence instead of adding event-loop wakeup latency to
/// every frame. Input-driven early frames do not move the scheduled deadline.
#[derive(Default)]
pub struct Pacer {
    next: Option<Instant>,
    period: Duration,
}
impl Pacer {
    pub fn delay(&mut self, started: Instant, finished: Instant, fps: u32) -> Duration {
        let period = Duration::from_secs_f64(1.0 / f64::from(fps.max(1)));
        let deadline = match self.next {
            Some(next) if self.period == period => {
                if started < next {
                    next
                } else {
                    // Skip elapsed deadlines after a hitch; never queue catch-up frames.
                    let elapsed = started.duration_since(next).as_nanos();
                    let missed = elapsed / period.as_nanos() + 1;
                    next + period.mul_f64(missed as f64)
                }
            }
            _ => started + period,
        };
        self.period = period;
        self.next = Some(deadline);
        deadline.saturating_duration_since(finished)
    }
}

#[derive(Default)]
pub struct Monitor {
    pub open: bool,
    intervals: VecDeque<f32>,
    cpu: VecDeque<f32>,
    panes: BTreeMap<String, f32>,
}
fn push(samples: &mut VecDeque<f32>, value: f32) {
    if samples.len() == 600 {
        samples.pop_front();
    }
    samples.push_back(value);
}
fn percentile(samples: &VecDeque<f32>, percentile: f32) -> f32 {
    let mut values: Vec<_> = samples.iter().copied().collect();
    values.sort_by(f32::total_cmp);
    values
        .get(((values.len().saturating_sub(1)) as f32 * percentile).round() as usize)
        .copied()
        .unwrap_or(0.0)
}
impl Monitor {
    pub fn begin(&mut self, interval: Duration) {
        push(&mut self.intervals, interval.as_secs_f32() * 1000.0);
        self.panes.clear();
    }
    pub fn pane(&mut self, label: String, elapsed: Duration) {
        self.panes.insert(label, elapsed.as_secs_f32() * 1000.0);
    }
    pub fn finish(&mut self, elapsed: Duration) {
        push(&mut self.cpu, elapsed.as_secs_f32() * 1000.0);
    }
    pub fn show(&mut self, ctx: &egui::Context, target: u32) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Performance").open(&mut open).default_width(560.0).show(ctx, |ui| {
            let budget = 1000.0 / target.max(1) as f32;
            ui.heading(format!("{target} FPS · {budget:.2} ms frame budget"));
            ui.weak("Rolling 600 frames. Collection stays bounded while this window is closed.");
            ui.separator();
            egui::Grid::new("frame-metrics").num_columns(2).spacing([28.0,8.0]).show(ui, |ui| {
                for (name, value) in [
                    ("Frame interval · median", percentile(&self.intervals,0.5)),
                    ("Frame interval · 95th percentile", percentile(&self.intervals,0.95)),
                    ("Frame interval · worst", percentile(&self.intervals,1.0)),
                    ("UI CPU work · 95th percentile", percentile(&self.cpu,0.95)),
                ] { ui.label(name); ui.monospace(format!("{value:.2} ms")); ui.end_row(); }
                ui.label("Frames over 1.5× budget");
                ui.monospace(self.intervals.iter().filter(|t| **t > budget*1.5).count().to_string()); ui.end_row();
            });
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(),110.0), egui::Sense::hover());
            let ceiling = (budget*3.0).max(percentile(&self.intervals,0.99)).max(1.0);
            let y = |ms:f32| rect.bottom() - (ms/ceiling).clamp(0.0,1.0)*rect.height();
            ui.painter().hline(rect.x_range(),y(budget),egui::Stroke::new(1.0,ui.visuals().selection.bg_fill));
            let points: Vec<_> = self.intervals.iter().enumerate().map(|(i,ms)| egui::pos2(rect.left()+i as f32/599.0*rect.width(),y(*ms))).collect();
            if points.len()>1 {ui.painter().add(egui::Shape::line(points,egui::Stroke::new(1.0,Color32::from_rgb(80,190,180))));}
            ui.separator(); ui.strong("Current visualizer CPU preparation");
            for (label, ms) in &self.panes { ui.horizontal(|ui| {ui.label(label);ui.monospace(format!("{ms:.2} ms"));}); }
            ui.weak("GPU execution is asynchronous and is not included in these CPU timings. Display scheduling, recording, window occlusion and other apps can affect frame intervals. 120 FPS requires a compatible display and a scene that fits its 8.33 ms budget.");
            ui.weak("Fidelity stays unchanged: diagnostics never reduce quality automatically.");
            if ui.button("Reset measurements").clicked(){ self.intervals.clear(); self.cpu.clear(); }
        });
        self.open = open;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pacing_compensates_wakeup_delay_without_bursts_or_timer_drift() {
        let mut pacer = Pacer::default();
        let start = Instant::now();
        let period = Duration::from_secs_f64(1.0 / 120.0);
        assert_eq!(pacer.delay(start, start, 120), period);
        let late = start + period + Duration::from_micros(700);
        let finished = late + Duration::from_micros(500);
        assert_eq!(
            pacer.delay(late, finished, 120),
            period - Duration::from_micros(1200)
        );
        // A mouse event before the next deadline does not restart the clock.
        let early = start + period + Duration::from_millis(2);
        assert_eq!(
            pacer.delay(early, early, 120),
            period - Duration::from_millis(2)
        );
        let hitch = start + Duration::from_secs(2);
        let wait = pacer.delay(hitch, hitch, 120);
        assert!(wait <= period && wait > Duration::ZERO);
        assert_eq!(
            pacer.delay(hitch, hitch, 60),
            Duration::from_secs_f64(1.0 / 60.0)
        );
    }
    #[test]
    fn catches_hitches_that_average_fps_hides_and_bounds_history() {
        let mut m = Monitor::default();
        for _ in 0..600 {
            m.begin(Duration::from_micros(8333));
        }
        m.begin(Duration::from_millis(60));
        assert_eq!(m.intervals.len(), 600);
        assert!(percentile(&m.intervals, 0.5) < 8.34);
        assert!(percentile(&m.intervals, 1.0) > 59.9);
    }
}
