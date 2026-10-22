//! Tap full-rate, unprocessed stereo PCM on the analysis worker, never the UI.
use arc_swap::ArcSwapOption;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::SyncSender,
    },
    time::Instant,
};
#[derive(Debug)]
pub struct Chunk {
    pub samples: Vec<[f32; 2]>,
    pub time: f64,
    pub rate: u32,
}
pub struct Tap {
    pub sender: SyncSender<Chunk>,
    pub started: Instant,
    pub dropped: Arc<AtomicU64>,
    pub running: Arc<AtomicBool>,
}
static TAP: OnceLock<ArcSwapOption<Tap>> = OnceLock::new();
pub fn current() -> Option<Arc<Tap>> {
    TAP.get_or_init(ArcSwapOption::empty).load_full()
}
pub fn install(tap: Option<Arc<Tap>>) {
    TAP.get_or_init(ArcSwapOption::empty).store(tap);
}
impl Tap {
    pub fn submit(&self, samples: Vec<[f32; 2]>, rate: u32) {
        if samples.is_empty() || !self.running.load(Ordering::Relaxed) {
            return;
        }
        let time = (self.started.elapsed().as_secs_f64() - samples.len() as f64 / f64::from(rate))
            .max(0.0);
        let len = samples.len();
        if self
            .sender
            .try_send(Chunk {
                samples,
                time,
                rate,
            })
            .is_err()
        {
            self.dropped.fetch_add(len as u64, Ordering::Relaxed);
        }
    }
}
