//! One full-resolution render, reused for the workspace preview and recording.
use super::{Stats, Video};
use eframe::egui_wgpu::{self, wgpu};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc::SyncSender,
};
#[derive(Default)]
struct ReadbackOrder {
    next: u64,
    ready: std::collections::BTreeMap<u64, Option<Video>>,
}
impl ReadbackOrder {
    fn complete(
        &mut self,
        sequence: u64,
        video: Option<Video>,
        send: &SyncSender<Video>,
        stats: &Stats,
        pending: &AtomicUsize,
    ) {
        self.ready.insert(sequence, video);
        while let Some(video) = self.ready.remove(&self.next) {
            if video.is_none_or(|video| send.try_send(video).is_err()) {
                stats.missed.fetch_add(1, Ordering::Relaxed);
            }
            self.next += 1;
            pending.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
pub struct Output {
    state: egui_wgpu::RenderState,
    renderer: egui_wgpu::Renderer,
    ctx: egui::Context,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub id: egui::TextureId,
    size: [u32; 2],
    pending: Arc<AtomicUsize>,
    buffers: Arc<Mutex<Vec<wgpu::Buffer>>>,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    pointer_captured: bool,
    sequence: u64,
    order: Arc<Mutex<ReadbackOrder>>,
}
impl Output {
    pub fn poll(&self) {
        let _ = self.state.device.poll(wgpu::PollType::Poll);
    }

    pub fn finish_readbacks(&self) {
        // Only used when quitting; normal Stop remains asynchronous in the UI.
        let _ = self.state.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_millis(200)),
        });
    }
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn new(state: egui_wgpu::RenderState, size: [u32; 2], fps: u32) -> Self {
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("recording output"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let id = state.renderer.write().register_native_texture(
            &state.device,
            &view,
            wgpu::FilterMode::Linear,
        );
        let renderer = egui_wgpu::Renderer::new(&state.device, format, Default::default());
        let ctx = egui::Context::default();
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("visual-output-format"), format);
            d.insert_temp(egui::Id::new("recording-output"), true);
            d.insert_temp(egui::Id::new("visual-target-fps"), fps);
        });
        Self {
            state,
            renderer,
            ctx,
            texture,
            view,
            id,
            size,
            pending: Arc::default(),
            buffers: Arc::default(),
            events: Vec::new(),
            modifiers: Default::default(),
            pointer_captured: false,
            sequence: 0,
            order: Arc::default(),
        }
    }
    /// Forward only this preview's gestures, transformed into output pixels.
    /// Events accumulate until the next output frame when recording below UI FPS.
    pub fn preview_input(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let response = ui.interact(
            rect,
            ui.id().with("recording-camera"),
            egui::Sense::click_and_drag(),
        );
        let hovered = response.contains_pointer();
        let scale = self.size[0] as f32 / rect.width().max(1.0);
        let transform = |p: egui::Pos2| egui::Pos2::ZERO + (p - rect.min) * scale;
        ui.input(|input| {
            if self.modifiers != input.modifiers {
                self.modifiers = input.modifiers;
                self.events
                    .push(egui::Event::ModifiersChanged(self.modifiers));
            }
            for event in &input.events {
                let mapped = match event {
                    egui::Event::PointerMoved(pos) if hovered || self.pointer_captured => {
                        Some(egui::Event::PointerMoved(transform(*pos)))
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    } if (hovered && *pressed) || self.pointer_captured => {
                        self.pointer_captured = *pressed;
                        Some(egui::Event::PointerButton {
                            pos: transform(*pos),
                            button: *button,
                            pressed: *pressed,
                            modifiers: *modifiers,
                        })
                    }
                    egui::Event::MouseWheel { .. } | egui::Event::Zoom(_) if hovered => {
                        Some(event.clone())
                    }
                    egui::Event::PointerGone => {
                        self.pointer_captured = false;
                        Some(event.clone())
                    }
                    _ => None,
                };
                if let Some(event) = mapped {
                    self.events.push(event);
                }
            }
        });
        if !hovered && !self.pointer_captured {
            self.events.push(egui::Event::PointerGone);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pane: &mut crate::modules::ModulePane,
        frame: &crate::analysis::AnalysisFrame,
        theme: &crate::theme::AppTheme,
        live: bool,
        time: f64,
        send: Option<&SyncSender<Video>>,
        stats: &Arc<Stats>,
    ) {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.size[0] as f32, self.size[1] as f32),
        );
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                time: Some(time),
                events: std::mem::take(&mut self.events),
                ..Default::default()
            },
            |ui| {
                pane.draw(ui, rect, frame, theme, live);
            },
        );
        let jobs = self.ctx.tessellate(output.shapes, 1.0);
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in deltas {
                self.renderer
                    .update_texture(&self.state.device, &self.state.queue, id, &delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.size,
            pixels_per_point: 1.0,
        };
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        let mut buffers = self.renderer.update_buffers(
            &self.state.device,
            &self.state.queue,
            &mut encoder,
            &jobs,
            &screen,
        );
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("recording composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        let stride = (self.size[0] * 4).div_ceil(256) * 256;
        // Preparation compiles shaders and allocates textures, but does not
        // enter the readback sequence or race with the first captured frame.
        let readback = if send.is_none() {
            None
        } else if self.pending.load(Ordering::Relaxed) < 3 {
            let buffer = self
                .buffers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop()
                .unwrap_or_else(|| {
                    self.state.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("recording readback"),
                        size: u64::from(stride) * u64::from(self.size[1]),
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    })
                });
            encoder.copy_texture_to_buffer(
                self.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(self.size[1]),
                    },
                },
                self.texture.size(),
            );
            Some(buffer)
        } else {
            stats.missed.fetch_add(1, Ordering::Relaxed);
            None
        };
        buffers.push(encoder.finish());
        self.state.queue.submit(buffers);
        for id in output.textures_delta.free.drain() {
            self.renderer.free_texture(&id);
        }
        if let (Some(buffer), Some(send)) = (readback, send) {
            self.pending.fetch_add(1, Ordering::Relaxed);
            let pending = self.pending.clone();
            let mapped = buffer.clone();
            let send = send.clone();
            let stats = stats.clone();
            let size = self.size;
            let buffers = self.buffers.clone();
            let order = self.order.clone();
            let sequence = self.sequence;
            self.sequence += 1;
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let video = if result.is_ok() {
                        let video = mapped.slice(..).get_mapped_range().ok().map(|data| {
                            let mut bytes = Vec::with_capacity((size[0] * size[1] * 4) as usize);
                            for row in data.chunks_exact(stride as usize).take(size[1] as usize) {
                                bytes.extend_from_slice(&row[..size[0] as usize * 4]);
                            }
                            Video { bytes, time }
                        });
                        mapped.unmap();
                        buffers
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(mapped.clone());
                        video
                    } else {
                        None
                    };
                    // Mapping callbacks may arrive in a different order from submissions.
                    // Keep at most three readbacks outstanding and deliver monotonic PTS.
                    order
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .complete(sequence, video, &send, &stats, &pending);
                });
        }
        let _ = self.state.device.poll(wgpu::PollType::Poll);
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.state.renderer.write().free_texture(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readbacks_reorder_without_blocking_and_failed_frames_leave_no_holes() {
        let (send, receive) = std::sync::mpsc::sync_channel(3);
        let stats = Stats::default();
        let pending = AtomicUsize::new(3);
        let mut order = ReadbackOrder::default();
        order.complete(
            2,
            Some(Video {
                bytes: vec![2],
                time: 2.0,
            }),
            &send,
            &stats,
            &pending,
        );
        assert!(receive.try_recv().is_err());
        order.complete(0, None, &send, &stats, &pending);
        order.complete(
            1,
            Some(Video {
                bytes: vec![1],
                time: 1.0,
            }),
            &send,
            &stats,
            &pending,
        );
        assert_eq!(receive.try_recv().unwrap().time, 1.0);
        assert_eq!(receive.try_recv().unwrap().time, 2.0);
        assert_eq!(pending.load(Ordering::Relaxed), 0);
        assert_eq!(stats.missed.load(Ordering::Relaxed), 1);
    }
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        struct Wake(std::thread::Thread);
        impl std::task::Wake for Wake {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = std::task::Waker::from(Arc::new(Wake(std::thread::current())));
        let mut cx = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                std::task::Poll::Ready(v) => return v,
                std::task::Poll::Pending => {
                    std::thread::park_timeout(std::time::Duration::from_millis(1))
                }
            }
        }
    }
    #[test]
    #[ignore = "requires a GPU; exercises the live recording scheduler"]
    fn live_scheduler_keeps_timestamps_on_grid_despite_jitter_and_hitches() {
        use super::super::{Recorder, Session};
        use std::{
            sync::atomic::{AtomicBool, AtomicU64},
            time::{Duration, Instant},
        };
        let instance = wgpu::Instance::default();
        let state = block_on(egui_wgpu::RenderState::create(
            &Default::default(),
            &instance,
            None,
            Default::default(),
        ))
        .unwrap();
        let frame = crate::analysis::test_audio_frame(&[(220.0, 0.2)]);
        let theme = crate::theme::AppTheme::default();
        for fps in [60, 120] {
            let mut pane = crate::modules::ModulePane::new(crate::modules::ModuleKind::Waveform);
            let (send, receive) = std::sync::mpsc::sync_channel(3);
            let (ready_send, ready) = std::sync::mpsc::channel();
            ready_send.send(()).unwrap();
            let (audio_send, _audio) = std::sync::mpsc::sync_channel(3);
            let (_result, result) = std::sync::mpsc::channel();
            let mut session = Session {
                target: pane.instance_id().into(),
                ready,
                armed: false,
                audio_send,
                started: Instant::now(),
                running: Arc::new(AtomicBool::new(true)),
                send,
                result,
                stats: Arc::new(Stats::default()),
                output: Output::new(state.clone(), [64, 64], fps),
                next_frame: 0,
                last_ui_frame: None,
                stopping: false,
                stop_time: Arc::new(AtomicU64::new(0)),
                worker: None,
            };
            for (ui_frame, tick) in [0_u64, 1, 4].into_iter().enumerate() {
                // Wake up a quarter frame late, then skip two whole frames.
                session.started =
                    Instant::now() - Duration::from_secs_f64((tick as f64 + 0.25) / fps as f64);
                Recorder::render_session(
                    &mut session,
                    fps,
                    ui_frame as u64,
                    &mut pane,
                    &frame,
                    &theme,
                    true,
                );
                // Hidden-pane fallback must not render a second time in this UI frame.
                Recorder::render_session(
                    &mut session,
                    fps,
                    ui_frame as u64,
                    &mut pane,
                    &frame,
                    &theme,
                    true,
                );
                state
                    .device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .unwrap();
                let captured = receive.recv_timeout(Duration::from_secs(2)).unwrap();
                assert_eq!(captured.time, tick as f64 / fps as f64);
                assert!(receive.try_recv().is_err());
            }
            assert_eq!(session.stats.missed.load(Ordering::Relaxed), 2);
            assert_eq!(session.output.sequence, 3);
            super::super::audio::install(None);
        }
    }
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires GPU and native hardware movie encoder"]
    fn output_to_movie_preserves_audio_and_60_or_120_fps_timestamps() {
        let instance = wgpu::Instance::default();
        let state = block_on(egui_wgpu::RenderState::create(
            &Default::default(),
            &instance,
            None,
            Default::default(),
        ))
        .unwrap();
        let frame = crate::analysis::test_audio_frame(&[(220.0, 0.2), (880.0, 0.15)]);
        for (fps, kind, size) in [
            (60, crate::modules::ModuleKind::Cymatics, [1080, 1920]),
            (120, crate::modules::ModuleKind::Waveform, [1280, 720]),
        ] {
            let path = std::env::temp_dir()
                .join(format!("symphos-pipeline-{fps}-{}.mov", std::process::id()));
            let mut pane = crate::modules::ModulePane::new(kind);
            let mut output = Output::new(state.clone(), size, fps);
            let (send, receive) = std::sync::mpsc::sync_channel(3);
            let (audio_send, audio_receive) = std::sync::mpsc::sync_channel(128);
            let stats = Arc::new(Stats::default());
            // Exclude one-time shader compilation from a performance capture.
            output.draw(
                &mut pane,
                &frame,
                &crate::theme::AppTheme::default(),
                true,
                0.0,
                Some(&send),
                &stats,
            );
            state
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            receive.recv().unwrap();
            let counters = stats.clone();
            let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let run = running.clone();
            let stopped = Arc::new(std::sync::atomic::AtomicU64::new(1_000_000));
            let (ready_send, ready) = std::sync::mpsc::channel();
            let started = std::time::Instant::now();
            let worker = std::thread::spawn(move || {
                super::super::encode_performance(
                    path,
                    size,
                    fps,
                    48000,
                    started,
                    run,
                    stopped,
                    counters,
                    receive,
                    audio_receive,
                    Some(ready_send),
                )
            });
            ready
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let started = std::time::Instant::now();
            for i in 0..fps {
                let time = i as f64 / fps as f64;
                output.draw(
                    &mut pane,
                    &frame,
                    &crate::theme::AppTheme::default(),
                    true,
                    time,
                    Some(&send),
                    &stats,
                );
                state
                    .device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .unwrap();
                let audio = vec![[0.01, -0.01]; (48000 / fps) as usize];
                audio_send
                    .send(super::super::audio::Chunk {
                        samples: audio,
                        time,
                        rate: 48000,
                    })
                    .unwrap();
                // Pace like a real performance; the encoding worker stays asynchronous.
                let deadline =
                    started + std::time::Duration::from_secs_f64((i + 1) as f64 / fps as f64);
                if let Some(wait) = deadline.checked_duration_since(std::time::Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
            running.store(false, Ordering::Release);
            let saved = worker.join().unwrap().unwrap();
            eprintln!("{saved}");
            assert_eq!(stats.frames.load(Ordering::Relaxed), fps as u64);
            assert_eq!(stats.missed.load(Ordering::Relaxed), 0);
            assert_eq!(stats.audio_dropped.load(Ordering::Relaxed), 0);
        }
    }
    #[test]
    #[ignore = "manual full-resolution CPU/GPU/readback benchmark; requires GPU"]
    fn benchmark_all_output_paths_at_120_fps() {
        let instance = wgpu::Instance::default();
        let state = block_on(egui_wgpu::RenderState::create(
            &Default::default(),
            &instance,
            None,
            Default::default(),
        ))
        .unwrap();
        let mut frame = crate::analysis::test_audio_frame(&[(120.0, 0.2), (440.0, 0.15)]);
        let theme = crate::theme::AppTheme::default();
        for kind in crate::modules::ModuleKind::ALL {
            let mut pane = crate::modules::ModulePane::new(kind);
            let now = std::time::Instant::now();
            let magnitudes: Arc<[f32]> = frame.bins.iter().map(|b| b.magnitude).collect();
            let rows: Vec<_> = (0..60)
                .map(|i| {
                    Arc::new(crate::capture_history::SpectralFrame {
                        sequence: i + 1,
                        time: now - std::time::Duration::from_secs_f64((59 - i) as f64 / 30.0),
                        sample_rate: frame.sample_rate,
                        fft_size: frame.fft_size,
                        magnitudes: magnitudes.clone(),
                    })
                })
                .collect();
            pane.ingest(&rows);
            let mut output = Output::new(state.clone(), [1920, 1080], 120);
            let (send, receive) = std::sync::mpsc::sync_channel(3);
            let stats = Arc::new(Stats::default());
            let mut timings = Vec::new();
            for i in 0..90 {
                frame.sequence = i + 61;
                let start = std::time::Instant::now();
                output.draw(
                    &mut pane,
                    &frame,
                    &theme,
                    true,
                    i as f64 / 120.0,
                    Some(&send),
                    &stats,
                );
                state
                    .device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .unwrap();
                let pixels = receive
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                assert_eq!(pixels.bytes.len(), 1920 * 1080 * 4);
                if i >= 30 {
                    timings.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                let period = std::time::Duration::from_secs_f64(1.0 / 120.0);
                if let Some(wait) = period.checked_sub(start.elapsed()) {
                    std::thread::sleep(wait);
                }
            }
            timings.sort_by(f64::total_cmp);
            eprintln!(
                "{}: median {:.3} ms, p95 {:.3} ms, worst {:.3} ms (1080p output + GPU readback)",
                kind.label(),
                timings[30],
                timings[57],
                timings[59]
            );
            assert_eq!(stats.missed.load(Ordering::Relaxed), 0);
        }
    }
    #[test]
    #[ignore = "requires Metal/Vulkan GPU and native MilkDrop renderer"]
    fn all_visualizers_render_to_exact_output_dimensions() {
        let instance = wgpu::Instance::default();
        let state = block_on(egui_wgpu::RenderState::create(
            &Default::default(),
            &instance,
            None,
            Default::default(),
        ))
        .unwrap();
        let scope = state.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let frame = crate::analysis::test_audio_frame(&[(120.0, 0.2), (440.0, 0.15)]);
        let theme = crate::theme::AppTheme::default();
        for kind in crate::modules::ModuleKind::ALL {
            let mut pane = crate::modules::ModulePane::new(kind);
            let mut output = Output::new(state.clone(), [322, 182], 60);
            let (send, receive) = std::sync::mpsc::sync_channel(3);
            let stats = Arc::new(Stats::default());
            for i in 0..4 {
                output.draw(
                    &mut pane,
                    &frame,
                    &theme,
                    true,
                    i as f64 / 60.0,
                    Some(&send),
                    &stats,
                );
                state
                    .device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .unwrap();
                let image = receive
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                assert_eq!(image.bytes.len(), 322 * 182 * 4, "{}", kind.label());
                assert!(
                    image.bytes.as_chunks::<4>().0.iter().all(|p| p[3] > 0),
                    "opaque canvas for {}",
                    kind.label()
                );
                std::thread::sleep(std::time::Duration::from_millis(17));
            }
            assert_eq!(stats.missed.load(Ordering::Relaxed), 0);
            eprintln!("Output validated: {}", kind.label());
        }
        assert!(block_on(scope.pop()).is_none());
    }
}
