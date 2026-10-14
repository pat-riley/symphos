//! System playback via Core Audio taps. Native objects live only on the control
//! worker. Each reconnect has its own ring and analysis runtime, so old-format
//! samples cannot leak into the new session.
use std::ffi::{CStr, c_char, c_void};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use super::{AudioEvent, AudioSource, AudioStatus, SourceKind};
use crate::analysis::{AnalysisRuntime, AnalysisSettings, StereoSample};

const AUDIO_RING_CAPACITY: usize = 262_144;
const SYSTEM_SOURCE: &str = "coreaudio:system";
const CAPTURE_HELP: &str = "Allow Symphos in System Settings → Privacy & Security → Screen & System Audio Recording, then use Retry audio capture in Settings.";

type SampleCallback = unsafe extern "C" fn(*mut c_void, *const f32, *const f32, u32, u32, u32);

unsafe extern "C" {
    fn symphos_capture_create(
        callback: SampleCallback,
        context: *mut c_void,
        result: *mut *mut c_void,
        rate: *mut u32,
        error: *mut c_char,
        error_size: u32,
    ) -> i32;
    fn symphos_capture_start(capture: *mut c_void) -> i32;
    fn symphos_capture_destroy(capture: *mut c_void) -> i32;
    fn symphos_capture_changed(capture: *mut c_void) -> bool;
}

struct CallbackData {
    producer: rtrb::Producer<StereoSample>,
    dropped: Arc<AtomicU64>,
    received: Arc<AtomicU64>,
}

// SAFETY: The native IOProc supplies valid Float32 buffers/strides for this call,
// invokes callbacks serially, and is stopped before CallbackData is destroyed.
unsafe extern "C" fn samples(
    context: *mut c_void,
    left: *const f32,
    right: *const f32,
    frames: u32,
    left_stride: u32,
    right_stride: u32,
) {
    let data = unsafe { &mut *context.cast::<CallbackData>() };
    let mut dropped = 0;
    for i in 0..frames as usize {
        let sample = unsafe {
            [
                *left.add(i * left_stride as usize),
                *right.add(i * right_stride as usize),
            ]
        };
        // A malformed device sample must not poison the FFT/history with NaNs.
        let sample = sample.map(|value| if value.is_finite() { value } else { 0.0 });
        if data.producer.push(sample).is_err() {
            dropped += 1;
        }
    }
    data.dropped.fetch_add(dropped, Ordering::Relaxed);
    data.received
        .fetch_add(u64::from(frames), Ordering::Relaxed);
}

struct Capture {
    native: *mut c_void,
    // Heap address remains stable until native callbacks have been detached.
    data: Option<Box<CallbackData>>,
}

impl Capture {
    fn open(settings: Arc<Mutex<AnalysisSettings>>) -> Result<(Self, AnalysisRuntime), String> {
        let (producer, consumer) = rtrb::RingBuffer::new(AUDIO_RING_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let data = Box::new(CallbackData {
            producer,
            dropped: dropped.clone(),
            received: Arc::new(AtomicU64::new(0)),
        });
        let mut capture = Self {
            native: std::ptr::null_mut(),
            data: Some(data),
        };
        let mut rate = 0;
        let mut error = [0 as c_char; 512];
        let context =
            &mut **capture.data.as_mut().expect("capture owns callback state") as *mut CallbackData;
        let status = unsafe {
            symphos_capture_create(
                samples,
                context.cast(),
                &mut capture.native,
                &mut rate,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        if status != 0 {
            return Err(format!(
                "{}. {CAPTURE_HELP}",
                unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
            ));
        }
        let analysis = AnalysisRuntime::start_with_settings(
            consumer,
            Arc::new(AtomicU32::new(rate)),
            Arc::new(AtomicU32::new(2)),
            dropped,
            settings,
        );
        let status = unsafe { symphos_capture_start(capture.native) };
        if status != 0 {
            return Err(format!(
                "Could not start system audio (Core Audio status {status}). {CAPTURE_HELP}"
            ));
        }
        Ok((capture, analysis))
    }

    fn received(&self) -> u64 {
        self.data
            .as_ref()
            .map_or(0, |data| data.received.load(Ordering::Relaxed))
    }

    fn changed(&self) -> bool {
        unsafe { symphos_capture_changed(self.native) }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let status = unsafe { symphos_capture_destroy(self.native) };
        if status != 0 {
            // The OS may still hold the context. Leaking it is safer than UAF.
            if let Some(data) = self.data.take() {
                let _ = Box::leak(data);
            }
            log::error!(
                "Core Audio teardown failed ({status}); retained callback state for safety"
            );
        }
    }
}

enum Command {
    Select,
    Refresh,
    Stop,
}
enum Event {
    Audio(AudioEvent),
    Session(AnalysisRuntime),
}

pub struct AudioEngine {
    pub analysis: AnalysisRuntime,
    command: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn start() -> Self {
        let (_producer, consumer) = rtrb::RingBuffer::new(1);
        let analysis = AnalysisRuntime::start(
            consumer,
            Arc::new(AtomicU32::new(48_000)),
            Arc::new(AtomicU32::new(2)),
            Arc::new(AtomicU64::new(0)),
        );
        let settings = analysis.settings.clone();
        let (command, commands) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("symphos-coreaudio".into())
            .spawn(move || capture_worker(commands, sender, settings))
            .expect("start Core Audio worker");
        Self {
            analysis,
            command,
            events,
            worker: Some(worker),
        }
    }

    pub fn select_source(&self, source: AudioSource) {
        if source.node_name == SYSTEM_SOURCE {
            let _ = self.command.send(Command::Select);
        }
    }

    pub fn refresh_sources(&self) {
        let _ = self.command.send(Command::Refresh);
    }

    pub fn drain_events(&mut self) -> std::vec::IntoIter<AudioEvent> {
        let mut events = Vec::new();
        for event in self.events.try_iter() {
            match event {
                Event::Audio(event) => events.push(event),
                Event::Session(analysis) => self.analysis = analysis,
            }
        }
        events.into_iter()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.command.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn capture_worker(
    commands: mpsc::Receiver<Command>,
    sender: mpsc::Sender<Event>,
    settings: Arc<Mutex<AnalysisSettings>>,
) {
    let send_status = |status| {
        let _ = sender.send(Event::Audio(AudioEvent::Status(status)));
    };
    let mut capture: Option<Capture> = None;
    let mut last_received = 0;
    let mut last_audio = Instant::now();
    let mut streaming = false;
    let mut waiting = false;
    loop {
        let reconnect = match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Refresh) => {
                let _ = sender.send(Event::Audio(AudioEvent::Sources(vec![AudioSource {
                    global_id: 0,
                    node_name: SYSTEM_SOURCE.into(),
                    display_name: "System audio".into(),
                    kind: SourceKind::SystemOutput,
                }])));
                false
            }
            Ok(Command::Select) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => capture.as_ref().is_some_and(Capture::changed),
        };
        if reconnect {
            capture.take(); // Stop callbacks before starting another session.
            send_status(AudioStatus::Connecting);
            match Capture::open(settings.clone()) {
                Ok((next, analysis)) => {
                    if sender.send(Event::Session(analysis)).is_err() {
                        break;
                    }
                    capture = Some(next);
                    last_received = 0;
                    last_audio = Instant::now();
                    streaming = false;
                    waiting = false;
                }
                Err(error) => send_status(AudioStatus::Error(error)),
            }
        }
        if let Some(active) = &capture {
            let received = active.received();
            if received != last_received {
                last_received = received;
                last_audio = Instant::now();
                if !streaming {
                    send_status(AudioStatus::Streaming);
                    streaming = true;
                    waiting = false;
                }
            } else if last_audio.elapsed() >= Duration::from_secs(3) && !waiting {
                send_status(AudioStatus::Paused);
                streaming = false;
                waiting = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_uses_new_samples_and_rate_but_preserves_controls() {
        let settings = Arc::new(Mutex::new(AnalysisSettings {
            fft_size: 512,
            ..Default::default()
        }));
        let start = |rate, samples: StereoSample| {
            let (mut producer, consumer) = rtrb::RingBuffer::new(4096);
            let runtime = AnalysisRuntime::start_with_settings(
                consumer,
                Arc::new(AtomicU32::new(rate)),
                Arc::new(AtomicU32::new(2)),
                Arc::new(AtomicU64::new(0)),
                settings.clone(),
            );
            let deadline = Instant::now() + Duration::from_secs(2);
            while runtime.snapshot.load().sequence == 0 {
                for _ in 0..512 {
                    let _ = producer.push(samples);
                }
                assert!(Instant::now() < deadline, "analysis did not publish");
                thread::sleep(Duration::from_millis(1));
            }
            runtime
        };
        let previous = start(48_000, [0.5, -0.5]);
        let next = start(44_100, [0.0, 0.0]);
        assert!(Arc::ptr_eq(&previous.settings, &next.settings));
        assert_eq!(previous.snapshot.load().sample_rate, 48_000);
        assert!(previous.snapshot.load().rms[0] > 0.49);
        assert_eq!(next.snapshot.load().sample_rate, 44_100);
        assert_eq!(next.snapshot.load().rms, [0.0; 2]);
        assert_eq!(next.snapshot.load().fft_size, 512);
        assert!(
            next.snapshot
                .load()
                .waveform_left
                .iter()
                .all(|sample| *sample == 0.0)
        );
    }

    #[test]
    fn native_callback_handles_interleaved_and_planar_stereo_and_overflow() {
        let (producer, mut consumer) = rtrb::RingBuffer::new(3);
        let dropped = Arc::new(AtomicU64::new(0));
        let received = Arc::new(AtomicU64::new(0));
        let mut data = CallbackData {
            producer,
            dropped: dropped.clone(),
            received: received.clone(),
        };
        let context = (&mut data as *mut CallbackData).cast();
        let interleaved = [0.1, -0.2, 0.3, -0.4];
        unsafe {
            samples(
                context,
                interleaved.as_ptr(),
                interleaved.as_ptr().add(1),
                2,
                2,
                2,
            );
        }
        assert_eq!(consumer.pop(), Ok([0.1, -0.2]));
        assert_eq!(consumer.pop(), Ok([0.3, -0.4]));
        let left = [0.5, f32::NAN, 0.7, 0.8];
        let right = [-0.5, f32::INFINITY, -0.7, -0.8];
        unsafe {
            samples(context, left.as_ptr(), right.as_ptr(), 4, 1, 1);
        }
        assert_eq!(consumer.pop(), Ok([0.5, -0.5]));
        assert_eq!(consumer.pop(), Ok([0.0, 0.0]));
        assert_eq!(consumer.pop(), Ok([0.7, -0.7]));
        assert!(consumer.pop().is_err());
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        assert_eq!(received.load(Ordering::Relaxed), 6);
    }
}
