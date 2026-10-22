//! A single ordered recovery writer. Rendering never validates, serializes, or
//! writes a document; at most one checkpoint is outstanding during playback.
use super::{Preset, Session, atomic_write};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    thread::JoinHandle,
};

pub(super) struct Job {
    pub session: Session,
    pub preset: Option<Preset>,
}
pub(super) struct Outcome {
    pub active: String,
    pub modified: Option<bool>,
    pub error: Option<String>,
}
impl Outcome {
    fn disconnected() -> Self {
        Self {
            active: String::new(),
            modified: None,
            error: Some("Workspace save worker stopped. Previous recovery file retained.".into()),
        }
    }
}
pub(super) struct Writer {
    sender: Option<SyncSender<Job>>,
    results: Receiver<Outcome>,
    worker: Option<JoinHandle<()>>,
    busy: bool,
}
impl Writer {
    pub fn new(path: PathBuf) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let (results_tx, results) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut last_bytes = Vec::new();
            while let Ok(job) = jobs.recv() {
                let mut outcome = Outcome {
                    active: job.session.active_preset.clone(),
                    modified: None,
                    error: None,
                };
                if !job.session.validate() {
                    outcome.error =
                        Some("Invalid workspace. Previous recovery file retained.".into());
                } else {
                    outcome.modified = job
                        .preset
                        .as_ref()
                        .map(|p| !crate::project::matches(&job.session, p));
                    // Recovery is machine-read. Compact JSON avoids expanding preview
                    // pixel arrays into millions of whitespace-heavy lines.
                    match serde_json::to_vec(&job.session).map_err(std::io::Error::other) {
                        Ok(bytes) if bytes.len() > crate::scenes::MAX_DOCUMENT_BYTES => {
                            outcome.error = Some("Workspace exceeds the 64 MB document limit. Previous recovery file retained.".into());
                        }
                        Ok(bytes) if bytes != last_bytes => match atomic_write(&path, &bytes) {
                            Ok(()) => last_bytes = bytes,
                            Err(error) => {
                                outcome.error = Some(format!("Workspace save failed: {error}"))
                            }
                        },
                        Ok(_) => {}
                        Err(error) => {
                            outcome.error = Some(format!("Workspace save failed: {error}"))
                        }
                    }
                }
                if results_tx.send(outcome).is_err() {
                    break;
                }
            }
        });
        Self {
            sender: Some(sender),
            results,
            worker: Some(worker),
            busy: false,
        }
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn submit(&mut self, job: Job) -> Result<(), String> {
        // The caller checks busy before constructing a potentially large snapshot.
        if self.busy {
            return Err("Workspace checkpoint already pending.".into());
        }
        self.sender.as_ref().unwrap().try_send(job).map_err(|_| {
            "Workspace save worker unavailable. Previous recovery file retained.".to_owned()
        })?;
        self.busy = true;
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Outcome> {
        if !self.busy {
            return None;
        }
        let outcome = match self.results.try_recv() {
            Ok(outcome) => outcome,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Outcome::disconnected(),
        };
        self.busy = false;
        Some(outcome)
    }
    pub fn finish(&mut self) -> Option<Outcome> {
        if !self.busy {
            return None;
        }
        let outcome = self
            .results
            .recv()
            .unwrap_or_else(|_| Outcome::disconnected());
        self.busy = false;
        Some(outcome)
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
