//! Development-only signal generator: no device access and no sound output.
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use super::{AudioEvent, AudioSource, AudioStatus, SourceKind};
use crate::analysis::AnalysisRuntime;

pub struct DemoEngine {
    pub analysis: AnalysisRuntime,
    events: mpsc::Receiver<AudioEvent>,
    sender: mpsc::Sender<AudioEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl DemoEngine {
    pub fn start() -> Self {
        let (mut producer, consumer) = rtrb::RingBuffer::new(65_536);
        let drops = Arc::new(AtomicU64::new(0));
        let analysis = AnalysisRuntime::start(
            consumer,
            Arc::new(AtomicU32::new(48_000)),
            Arc::new(AtomicU32::new(2)),
            drops.clone(),
        );
        let stop = Arc::new(AtomicBool::new(false));
        let signal_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("symphos-demo".into())
            .spawn(move || {
                let start = Instant::now();
                let mut cursor = 0_u64;
                while !signal_stop.load(Ordering::Acquire) {
                    for _ in 0..480 {
                        if producer.push(signal(cursor)).is_err() {
                            drops.fetch_add(1, Ordering::Relaxed);
                        }
                        cursor += 1;
                    }
                    let deadline = start + Duration::from_secs_f64(cursor as f64 / 48_000.0);
                    if let Some(wait) = deadline.checked_duration_since(Instant::now()) {
                        thread::sleep(wait);
                    }
                    // Do not generate a burst to catch up after a suspended process.
                    if start.elapsed().as_secs_f64() - cursor as f64 / 48_000.0 > 0.1 {
                        cursor = (start.elapsed().as_secs_f64() * 48_000.0) as u64;
                    }
                }
            })
            .expect("start development signal");
        let (sender, events) = mpsc::channel();
        Self {
            analysis,
            events,
            sender,
            stop,
            worker: Some(worker),
        }
    }
    pub fn refresh_sources(&self) {
        let _ = self.sender.send(AudioEvent::Sources(vec![AudioSource {
            global_id: 0,
            node_name: "demo:speakers".into(),
            display_name: "Demo audio · generated stereo".into(),
            kind: SourceKind::SystemOutput,
        }]));
        let _ = self.sender.send(AudioEvent::Status(AudioStatus::Streaming));
    }
    pub fn select_source(&self) {
        let _ = self.sender.send(AudioEvent::Status(AudioStatus::Streaming));
    }
    pub fn drain_events(&self) -> std::vec::IntoIter<AudioEvent> {
        self.events.try_iter().collect::<Vec<_>>().into_iter()
    }
}
impl Drop for DemoEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn signal(index: u64) -> [f32; 2] {
    let time = index as f64 / 48_000.0;
    let cycle = (time * 2.0).fract();
    let bass = (-cycle * 9.0).exp() * (std::f64::consts::TAU * 110.0 * time).sin() * 0.45;
    let shimmer = 0.5 + 0.5 * (time * 0.4).sin();
    [
        (bass
            + 0.15 * (std::f64::consts::TAU * 440.0 * time).sin()
            + 0.08 * shimmer * (std::f64::consts::TAU * 1760.0 * time).sin()) as f32,
        (bass
            + 0.13 * (std::f64::consts::TAU * 660.0 * time).sin()
            + 0.07 * shimmer * (std::f64::consts::TAU * 2200.0 * time).sin()) as f32,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_signal_is_bounded_stereo_and_drives_real_analysis() {
        for index in 0..48_000 {
            let sample = signal(index);
            assert!(sample.iter().all(|v| v.is_finite() && v.abs() < 1.0));
        }
        assert_ne!(signal(1234)[0], signal(1234)[1]);
        let engine = DemoEngine::start();
        let deadline = Instant::now() + Duration::from_secs(2);
        while engine.analysis.snapshot.load().sequence == 0 {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(engine.analysis.snapshot.load().rms[0] > 0.01);
    }
}
