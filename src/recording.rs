//! Live selected-visualizer recording. Pixels are rendered at output resolution;
//! raw audio is captured before analysis gain/smoothing. Bounded queues never
//! stall the audio callback or wait for the encoder on the UI thread.
pub mod audio;
#[cfg(target_os = "macos")]
mod native;
mod render;
use eframe::egui_wgpu;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
};
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Video {
    bytes: Vec<u8>,
    time: f64,
}
#[derive(Default)]
struct Stats {
    frames: AtomicU64,
    missed: AtomicU64,
    audio_dropped: Arc<AtomicU64>,
}
struct Session {
    target: String,
    ready: Receiver<()>,
    armed: bool,
    audio_send: SyncSender<audio::Chunk>,
    started: Instant,
    running: Arc<AtomicBool>,
    send: SyncSender<Video>,
    result: Receiver<Result<String, String>>,
    stats: Arc<Stats>,
    output: render::Output,
    next_frame: u64,
    last_ui_frame: Option<u64>,
    stopping: bool,
    stop_time: Arc<AtomicU64>,
    worker: Option<std::thread::JoinHandle<()>>,
}
pub struct Recorder {
    pub open: bool,
    size: [u32; 2],
    fps: u32,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    state: Option<egui_wgpu::RenderState>,
    session: Option<Session>,
    notice: Option<String>,
    preset: usize,
}
impl Default for Recorder {
    fn default() -> Self {
        Self {
            open: false,
            size: [1920, 1080],
            fps: 60,
            state: None,
            session: None,
            notice: None,
            preset: 0,
        }
    }
}
impl Recorder {
    pub fn new(state: Option<egui_wgpu::RenderState>) -> Self {
        let mut recorder = Self::default();
        recorder.state = state;
        recorder
    }
    pub fn target_fps(&self) -> u32 {
        if self.active() { self.fps } else { 0 }
    }
    pub fn target(&self) -> Option<&str> {
        self.session
            .as_ref()
            .filter(|s| !s.stopping)
            .map(|s| s.target.as_str())
    }
    pub fn active(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| !s.stopping && s.running.load(Ordering::Acquire))
    }
    pub fn stop(&mut self) {
        if let Some(s) = &mut self.session {
            s.stop_time
                .compare_exchange(
                    0,
                    s.started.elapsed().as_micros().max(1) as u64,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .ok();
            s.stopping = true;
            s.running.store(false, Ordering::Release);
            audio::install(None);
        }
    }
    #[cfg(target_os = "macos")]
    fn start(&mut self, path: PathBuf, target: String, rate: u32) -> Result<(), String> {
        if self.session.is_some() {
            return Err("A recording is already active or finishing.".into());
        }
        let state = self.state.clone().ok_or("GPU output is unavailable.")?;
        let size = self.size;
        let fps = self.fps;
        if size.iter().any(|v| !(64..=4096).contains(v) || v % 2 != 0)
            || size[0] * size[1] > 4096 * 2160
        {
            return Err(
                "Choose even dimensions from 64 to 4096 pixels, up to 4096 × 2160 total pixels."
                    .into(),
            );
        }
        let started = Instant::now();
        let running = Arc::new(AtomicBool::new(true));
        let stats = Arc::new(Stats::default());
        let (send, video) = mpsc::sync_channel::<Video>(3);
        let (audio_send, audio_rx) = mpsc::sync_channel::<audio::Chunk>(128);
        let (done, result) = mpsc::channel();
        let (ready_send, ready) = mpsc::channel();
        let run = running.clone();
        let counters = stats.clone();
        let stop_time = Arc::new(AtomicU64::new(0));
        let stopped = stop_time.clone();
        let worker = std::thread::Builder::new()
            .name("symphos-recording".into())
            .spawn(move || {
                let result = encode_performance(
                    path,
                    size,
                    fps,
                    rate,
                    started,
                    run,
                    stopped,
                    counters,
                    video,
                    audio_rx,
                    Some(ready_send),
                );
                let _ = done.send(result);
            })
            .map_err(|e| e.to_string())?;
        self.session = Some(Session {
            target,
            ready,
            armed: false,
            audio_send,
            started,
            running,
            send,
            result,
            stats,
            output: render::Output::new(state, size, fps),
            next_frame: 0,
            last_ui_frame: None,
            stopping: false,
            stop_time,
            worker: Some(worker),
        });
        self.notice = None;
        Ok(())
    }
    /// Draw selected component exactly once, then scale its texture only for preview.
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        pane: &mut crate::modules::ModulePane,
        frame: &crate::analysis::AnalysisFrame,
        theme: &crate::theme::AppTheme,
        live: bool,
    ) -> bool {
        let Some(s) = self.session.as_mut().filter(|s| {
            s.target == pane.instance_id() && !s.stopping && s.running.load(Ordering::Acquire)
        }) else {
            return false;
        };
        let scale = (rect.width() / self.size[0] as f32).min(rect.height() / self.size[1] as f32);
        let dest = egui::Rect::from_center_size(
            rect.center(),
            egui::vec2(self.size[0] as f32 * scale, self.size[1] as f32 * scale),
        );
        s.output.preview_input(ui, dest);
        Self::render_session(
            s,
            self.fps,
            ui.ctx().cumulative_frame_nr(),
            pane,
            frame,
            theme,
            live,
        );
        if !s.armed {
            return false;
        }
        if !crate::background::pane_painted(ui) {
            pane.background.paint(ui, rect);
        }
        ui.painter().image(
            s.output.id,
            dest,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        true
    }
    #[allow(clippy::too_many_arguments)]
    fn render_session(
        s: &mut Session,
        fps: u32,
        ui_frame: u64,
        pane: &mut crate::modules::ModulePane,
        frame: &crate::analysis::AnalysisFrame,
        theme: &crate::theme::AppTheme,
        live: bool,
    ) {
        if s.last_ui_frame == Some(ui_frame) {
            return;
        }
        s.last_ui_frame = Some(ui_frame);
        if !s.armed {
            if s.ready.try_recv().is_err() {
                return;
            }
            // Compile/lazily allocate the real component output before starting
            // the performance clock or tapping audio. Preview remains available.
            s.output.draw(pane, frame, theme, live, 0.0, None, &s.stats);
            s.started = Instant::now();
            s.armed = true;
            audio::install(Some(Arc::new(audio::Tap {
                sender: s.audio_send.clone(),
                started: s.started,
                dropped: s.stats.audio_dropped.clone(),
                running: s.running.clone(),
            })));
            // Capture frame zero in this same UI frame. Waiting until the next
            // repaint would skip it whenever UI and recording cadences match.
        }
        let time = s.started.elapsed().as_secs_f64();
        let tick = (time * f64::from(fps)).floor() as u64;
        if tick >= s.next_frame {
            if tick > s.next_frame {
                s.stats
                    .missed
                    .fetch_add(tick - s.next_frame, Ordering::Relaxed);
            }
            // Timestamp on the requested frame grid. UI wakeup jitter must not
            // turn a 60 FPS recording into irregular 120/40 FPS packet intervals.
            s.output.draw(
                pane,
                frame,
                theme,
                live,
                tick as f64 / f64::from(fps),
                Some(&s.send),
                &s.stats,
            );
            s.next_frame = tick + 1;
        }
    }
    pub fn render_hidden(
        &mut self,
        ctx: &egui::Context,
        pane: &mut crate::modules::ModulePane,
        frame: &crate::analysis::AnalysisFrame,
        theme: &crate::theme::AppTheme,
        live: bool,
    ) {
        if let Some(s) = self.session.as_mut().filter(|s| {
            !s.stopping && s.running.load(Ordering::Acquire) && s.target == pane.instance_id()
        }) {
            Self::render_session(
                s,
                self.fps,
                ctx.cumulative_frame_nr(),
                pane,
                frame,
                theme,
                live,
            );
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, selected: Option<(&str, &str)>, rate: u32) {
        #[cfg(not(target_os = "macos"))]
        let _ = rate;
        if let Some(session) = &self.session {
            // A stopped/hidden component no longer draws; continue delivering its
            // last GPU readbacks while the encoder drains the bounded queues.
            session.output.poll();
        }
        let completed = self
            .session
            .as_ref()
            .and_then(|s| match s.result.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("The recording worker stopped unexpectedly.".into()))
                }
            });
        if let Some(result) = completed {
            // A worker failure must also disarm the audio tap and capture clock.
            // Otherwise a disconnected result channel leaves a permanent
            // "Preparing recorder" or "Recording" state.
            self.stop();
            audio::install(None);
            self.session = None;
            self.notice = Some(result.unwrap_or_else(|e| format!("Recording failed: {e}")));
            self.open = true;
        }

        if !self.open {
            return;
        }
        let mut open = self.open;
        let mut start = false;
        let mut stop = false;
        egui::Window::new("Record visualizer")
            .open(&mut open)
            .resizable(false)
            .default_width(430.0)
            .show(ctx, |ui| {
                if let Some(session) = &self.session {
                    ui.strong(if session.stopping {
                        "Finishing recording…"
                    } else if !session.armed {
                        "Preparing recorder…"
                    } else {
                        "Recording performance"
                    });
                    let elapsed = if session.armed { session.started.elapsed().as_secs_f64() } else { 0.0 };
                    ui.label(format!("{elapsed:.1} s · {} × {} · {} FPS target", self.size[0], self.size[1], self.fps));
                    ui.separator();
                    egui::Grid::new("recording-counters").num_columns(2).show(ui, |ui| {
                        for (label, count) in [
                            ("Captured frames", session.stats.frames.load(Ordering::Relaxed)),
                            ("Missed frames", session.stats.missed.load(Ordering::Relaxed)),
                            ("Dropped audio samples", session.stats.audio_dropped.load(Ordering::Relaxed)),
                        ] {
                            ui.label(label);
                            ui.monospace(count.to_string());
                            ui.end_row();
                        }
                    });
                    ui.add_space(8.0);
                    stop = ui.add_enabled(!session.stopping, egui::Button::new(if session.armed { "Stop & save" } else { "Cancel preparation" })).clicked();
                    ui.weak("Closing this window keeps recording. Reopen it using Record Visualizer in the File menu.");
                } else {
                    ui.strong(selected.map_or("Select a visualizer in the workspace.", |(_, name)| name));
                    ui.add_space(6.0);
                    let presets = [
                        ("Landscape · 1080p", [1920, 1080]),
                        ("Portrait · phone", [1080, 1920]),
                        ("Square", [1080, 1080]),
                        ("Landscape · 4K", [3840, 2160]),
                        ("Custom", self.size),
                    ];
                    egui::Grid::new("recording-options").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                        ui.label("Output size");
                        crate::dropdown::ComboBox::from_id_salt("recording-size").width(220.0)
                            .selected_text(presets[self.preset].0).show_ui(ui, |ui| {
                                for (index, (label, size)) in presets.iter().enumerate() {
                                    if ui.selectable_value(&mut self.preset, index, *label).clicked() {
                                        self.size = *size;
                                    }
                                }
                            });
                        ui.end_row();
                        if self.preset == 4 {
                            ui.label("Dimensions");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut self.size[0]).range(64..=4096).prefix("W "));
                                ui.add(egui::DragValue::new(&mut self.size[1]).range(64..=4096).prefix("H "));
                            });
                            ui.end_row();
                        }
                        ui.label("Frame rate");
                        crate::dropdown::ComboBox::from_id_salt("recording-rate").width(220.0)
                            .selected_text(format!("{} FPS", self.fps)).show_ui(ui, |ui| {
                                for fps in [30, 60, 120] {
                                    ui.selectable_value(&mut self.fps, fps, format!("{fps} FPS"));
                                }
                            });
                        ui.end_row();
                        ui.label("Format");
                        ui.label("H.264 · AAC stereo · MOV");
                        ui.end_row();
                    });
                    ui.add_space(8.0);
                    ui.weak("Captures this visualizer at the selected resolution with incoming audio. Live capture reports missed frames; guaranteed frame-perfect offline export is not available yet.");
                    start = ui.add_enabled(selected.is_some() && cfg!(target_os = "macos"), egui::Button::new("Choose file & record…")).clicked();
                    if !cfg!(target_os = "macos") {
                        ui.weak("Live encoding currently requires macOS.");
                    }
                }
                if let Some(notice) = &self.notice {
                    ui.separator();
                    ui.label(notice);
                }
            });
        self.open = open;
        if stop {
            self.stop();
        }
        if start {
            #[cfg(target_os = "macos")]
            if let Some(path) = native::choose()
                && let Some((target, _)) = selected
                && let Err(e) = self.start(path, target.into(), rate)
            {
                self.notice = Some(e);
            }
        }
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
        if let Some(mut session) = self.session.take()
            && let Some(worker) = session.worker.take()
        {
            session.output.finish_readbacks();
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
fn encode_performance(
    path: PathBuf,
    size: [u32; 2],
    fps: u32,
    rate: u32,
    started: Instant,
    run: Arc<AtomicBool>,
    stopped: Arc<AtomicU64>,
    counters: Arc<Stats>,
    video: Receiver<Video>,
    audio_rx: Receiver<audio::Chunk>,
    ready: Option<mpsc::Sender<()>>,
) -> Result<String, String> {
    // The native save panel confirms replacement. Keep the previous movie intact
    // until the entire replacement has been successfully finalized.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_file_name(format!(".symphos-recording-{unique}.mov"));
    let result = (|| {
        let mut encoder = native::Encoder::open(&temporary, size, fps, rate)?;
        if let Some(ready) = ready {
            let _ = ready.send(());
        }
        let mut audio_end: Option<f64> = None;
        let mut stopping_at = None;
        let mut ended_reason = None;
        loop {
            // Bound each drain so an unusually busy audio producer cannot starve video.
            for _ in 0..32 {
                let Ok(chunk) = audio_rx.try_recv() else {
                    break;
                };
                if chunk.rate != rate {
                    // Preserve the performance already captured rather than
                    // discarding it or encoding the new rate at the wrong pitch.
                    ended_reason = Some("Audio sample rate changed; capture stopped");
                    stopped
                        .compare_exchange(
                            0,
                            (chunk.time * 1_000_000.0).max(1.0) as u64,
                            Ordering::AcqRel,
                            Ordering::Relaxed,
                        )
                        .ok();
                    run.store(false, Ordering::Release);
                    break;
                }
                // Sample count, not scheduler jitter, determines consecutive audio
                // timestamps. Keep real gaps after a disconnected source.
                let time = audio_end
                    .filter(|end| chunk.time - *end < 0.1)
                    .unwrap_or(chunk.time);
                if retry_encoder(|| encoder.audio(&chunk.samples, time))? {
                    audio_end = Some(time + chunk.samples.len() as f64 / f64::from(rate));
                } else {
                    audio_end = None;
                    counters
                        .audio_dropped
                        .fetch_add(chunk.samples.len() as u64, Ordering::Relaxed);
                }
            }
            match video.recv_timeout(std::time::Duration::from_millis(5)) {
                Ok(frame) => {
                    if retry_encoder(|| encoder.video(&frame.bytes, frame.time))? {
                        counters.frames.fetch_add(1, Ordering::Relaxed);
                    } else {
                        counters.missed.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if !run.load(Ordering::Acquire) {
                let time = stopping_at.get_or_insert_with(Instant::now);
                if time.elapsed().as_millis() > 250 {
                    break;
                }
            }
        }
        if counters.frames.load(Ordering::Relaxed) == 0 {
            return Err("No video frames were captured. The destination was not changed.".into());
        }
        let end = stopped.load(Ordering::Acquire);
        encoder.finish(if end > 0 {
            end as f64 / 1_000_000.0
        } else {
            started.elapsed().as_secs_f64()
        })?;
        std::fs::rename(&temporary, &path)
            .map_err(|e| format!("Could not finish saving movie: {e}"))?;
        Ok(format!(
            "{}Saved {} · {} frames · {} missed frames · {} dropped audio samples",
            ended_reason.map_or(String::new(), |reason| format!("{reason}. ")),
            path.display(),
            counters.frames.load(Ordering::Relaxed),
            counters.missed.load(Ordering::Relaxed),
            counters.audio_dropped.load(Ordering::Relaxed)
        ))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
#[cfg(target_os = "macos")]
fn retry_encoder(mut append: impl FnMut() -> Result<bool, String>) -> Result<bool, String> {
    let start = Instant::now();
    loop {
        if append()? {
            return Ok(true);
        }
        if start.elapsed().as_millis() >= 50 {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires native macOS video/audio encoder"]
    fn capture_without_audio_and_source_rate_changes_preserve_movies() {
        for change_rate in [false, true] {
            let path = std::env::temp_dir().join(format!(
                "symphos-capture-lifecycle-{}-{change_rate}.mov",
                std::process::id()
            ));
            let worker_path = path.clone();
            let (send, video) = mpsc::sync_channel(3);
            let (audio_send, audio_rx) = mpsc::sync_channel(128);
            let (ready_send, ready) = mpsc::channel();
            let running = Arc::new(AtomicBool::new(true));
            let run = running.clone();
            let counters = Arc::new(Stats::default());
            let stats = counters.clone();
            let stopped = Arc::new(AtomicU64::new(0));
            let end = stopped.clone();
            let worker = std::thread::spawn(move || {
                encode_performance(
                    worker_path,
                    [64, 64],
                    60,
                    48000,
                    Instant::now(),
                    run,
                    end,
                    stats,
                    video,
                    audio_rx,
                    Some(ready_send),
                )
            });
            ready
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            for i in 0..30 {
                let time = i as f64 / 60.0;
                send.send(Video {
                    bytes: vec![128; 64 * 64 * 4],
                    time,
                })
                .unwrap();
                if change_rate {
                    audio_send
                        .send(audio::Chunk {
                            samples: vec![[0.01, -0.01]; 800],
                            time,
                            rate: 48000,
                        })
                        .unwrap();
                }
            }
            if change_rate {
                audio_send
                    .send(audio::Chunk {
                        samples: vec![[0.0; 2]; 735],
                        time: 0.5,
                        rate: 44100,
                    })
                    .unwrap();
            } else {
                stopped.store(500_000, Ordering::Release);
                running.store(false, Ordering::Release);
            }
            let notice = worker.join().unwrap().unwrap();
            assert!(path.metadata().unwrap().len() > 1000);
            assert_eq!(counters.frames.load(Ordering::Relaxed), 30);
            assert_eq!(notice.contains("sample rate changed"), change_rate);
            assert!(!running.load(Ordering::Acquire));
        }
        // Invalid output must never replace an existing destination.
        let path = std::env::temp_dir().join(format!(
            "symphos-recording-preserve-{}.mov",
            std::process::id()
        ));
        std::fs::write(&path, b"existing movie").unwrap();
        let (_v, video) = mpsc::sync_channel(3);
        let (_a, audio) = mpsc::sync_channel(3);
        assert!(
            encode_performance(
                path.clone(),
                [1, 1],
                60,
                48000,
                Instant::now(),
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicU64::new(0)),
                Arc::new(Stats::default()),
                video,
                audio,
                None
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"existing movie");
    }
}
