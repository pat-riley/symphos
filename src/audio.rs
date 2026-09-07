use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context as _, Result};
use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::param::format::{MediaSubtype, MediaType};
use pw::spa::param::format_utils;
use pw::spa::pod::Pod;
use rtrb::Producer;

use crate::analysis::{AnalysisRuntime, StereoSample};

const AUDIO_RING_CAPACITY: usize = 262_144;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Input,
    SystemOutput,
}

impl SourceKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::SystemOutput => "System output",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSource {
    pub global_id: u32,
    pub node_name: String,
    pub display_name: String,
    pub kind: SourceKind,
}

#[derive(Clone, Debug)]
pub enum AudioStatus {
    Idle,
    Connecting,
    Paused,
    Streaming,
    Error(String),
}

impl AudioStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Idle => "Select a source",
            Self::Connecting => "Connecting",
            Self::Paused => "Ready — waiting for audio",
            Self::Streaming => "Live",
            Self::Error(message) => message,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AudioEvent {
    Sources(Vec<AudioSource>),
    Status(AudioStatus),
}

enum AudioCommand {
    Select(AudioSource),
    Refresh,
    Stop,
}

pub struct AudioEngine {
    pub analysis: AnalysisRuntime,
    command: pw::channel::Sender<AudioCommand>,
    events: mpsc::Receiver<AudioEvent>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn start() -> Self {
        let (producer, consumer) = rtrb::RingBuffer::<StereoSample>::new(AUDIO_RING_CAPACITY);
        let sample_rate = Arc::new(AtomicU32::new(48_000));
        let channels = Arc::new(AtomicU32::new(2));
        let dropped_samples = Arc::new(AtomicU64::new(0));
        let analysis = AnalysisRuntime::start(
            consumer,
            sample_rate.clone(),
            channels.clone(),
            dropped_samples.clone(),
        );
        let (event_sender, events) = mpsc::channel();
        let (command, command_receiver) = pw::channel::channel();
        let error_sender = event_sender.clone();

        let worker = thread::Builder::new()
            .name("symphos-pipewire".into())
            .spawn(move || {
                if let Err(error) = pipewire_thread(
                    producer,
                    sample_rate,
                    channels,
                    dropped_samples,
                    command_receiver,
                    event_sender,
                ) {
                    let _ = error_sender.send(AudioEvent::Status(AudioStatus::Error(format!(
                        "PipeWire: {error:#}"
                    ))));
                }
            })
            .expect("failed to start PipeWire thread");

        Self {
            analysis,
            command,
            events,
            worker: Some(worker),
        }
    }

    pub fn select_source(&self, source: AudioSource) {
        let _ = self.command.send(AudioCommand::Select(source));
    }

    pub fn refresh_sources(&self) {
        let _ = self.command.send(AudioCommand::Refresh);
    }

    pub fn drain_events(&self) -> impl Iterator<Item = AudioEvent> + '_ {
        self.events.try_iter()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.command.send(AudioCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct CaptureData {
    format: spa::param::audio::AudioInfoRaw,
    producer: Rc<RefCell<Producer<StereoSample>>>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicU64>,
}

struct ActiveCapture {
    // Listener must be dropped before the stream it is registered on.
    _listener: pw::stream::StreamListener<CaptureData>,
    _stream: pw::stream::StreamRc,
}

fn pipewire_thread(
    producer: Producer<StereoSample>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicU64>,
    command_receiver: pw::channel::Receiver<AudioCommand>,
    event_sender: mpsc::Sender<AudioEvent>,
) -> Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None).context("create main loop")?;
    let context = pw::context::ContextRc::new(&mainloop, None).context("create context")?;
    let core = context.connect_rc(None).context("connect to server")?;
    let registry = core.get_registry_rc().context("get registry")?;
    let sources = Rc::new(RefCell::new(Vec::<AudioSource>::new()));
    let active = Rc::new(RefCell::new(None::<ActiveCapture>));
    let producer = Rc::new(RefCell::new(producer));

    let registry_sources = sources.clone();
    let registry_sender = event_sender.clone();
    let remove_sources = sources.clone();
    let remove_sender = event_sender.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |object| {
            if object.type_ != pw::types::ObjectType::Node {
                return;
            }
            let Some(props) = object.props else {
                return;
            };
            let kind = match props.get("media.class") {
                Some("Audio/Source") => SourceKind::Input,
                Some("Audio/Sink") => SourceKind::SystemOutput,
                _ => return,
            };
            let Some(node_name) = props.get("node.name") else {
                return;
            };
            let display_name = props
                .get("node.description")
                .or_else(|| props.get("node.nick"))
                .unwrap_or(node_name)
                .to_owned();
            let source = AudioSource {
                global_id: object.id,
                node_name: node_name.to_owned(),
                display_name,
                kind,
            };
            log::debug!(
                "discovered {} source: {} ({})",
                source.kind.label(),
                source.display_name,
                source.node_name
            );
            let mut list = registry_sources.borrow_mut();
            if let Some(existing) = list.iter_mut().find(|item| item.global_id == object.id) {
                *existing = source;
            } else {
                list.push(source);
            }
            sort_sources(&mut list);
            let _ = registry_sender.send(AudioEvent::Sources(list.clone()));
        })
        .global_remove(move |id| {
            let mut list = remove_sources.borrow_mut();
            list.retain(|source| source.global_id != id);
            let _ = remove_sender.send(AudioEvent::Sources(list.clone()));
        })
        .register();

    let command_loop = mainloop.clone();
    let command_core = core.clone();
    let command_sources = sources.clone();
    let command_active = active.clone();
    let command_producer = producer.clone();
    let command_rate = sample_rate.clone();
    let command_channels = channels.clone();
    let command_dropped = dropped_samples.clone();
    let command_sender = event_sender.clone();
    let _command_receiver =
        command_receiver.attach(mainloop.loop_(), move |command| match command {
            AudioCommand::Select(source) => {
                // Disconnect cleanly before replacing the listener and stream.
                command_active.borrow_mut().take();
                let _ = command_sender.send(AudioEvent::Status(AudioStatus::Connecting));
                match start_capture(
                    command_core.clone(),
                    &source,
                    command_producer.clone(),
                    command_rate.clone(),
                    command_channels.clone(),
                    command_dropped.clone(),
                    command_sender.clone(),
                ) {
                    Ok(capture) => *command_active.borrow_mut() = Some(capture),
                    Err(error) => {
                        let _ = command_sender.send(AudioEvent::Status(AudioStatus::Error(
                            format!("Could not capture {}: {error:#}", source.display_name),
                        )));
                    }
                }
            }
            AudioCommand::Refresh => {
                let _ = command_sender.send(AudioEvent::Sources(command_sources.borrow().clone()));
            }
            AudioCommand::Stop => command_loop.quit(),
        });

    // The initial registry burst is delivered after the loop starts.
    let _ = event_sender.send(AudioEvent::Status(AudioStatus::Idle));
    mainloop.run();
    active.borrow_mut().take();
    Ok(())
}

fn start_capture(
    core: pw::core::CoreRc,
    source: &AudioSource,
    producer: Rc<RefCell<Producer<StereoSample>>>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU32>,
    dropped_samples: Arc<AtomicU64>,
    event_sender: mpsc::Sender<AudioEvent>,
) -> Result<ActiveCapture> {
    log::info!(
        "capturing {} source: {} ({})",
        source.kind.label(),
        source.display_name,
        source.node_name
    );
    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::NODE_NAME => "symphos-capture",
        *pw::keys::NODE_DESCRIPTION => "Symphos audio analyzer",
        *pw::keys::TARGET_OBJECT => source.node_name.clone(),
        *pw::keys::NODE_LATENCY => "256/48000",
    };
    if source.kind == SourceKind::SystemOutput {
        props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");
        props.insert(*pw::keys::STREAM_MONITOR, "true");
    }

    let stream = pw::stream::StreamRc::new(core, "symphos-capture", props)
        .context("create capture stream")?;
    let data = CaptureData {
        format: Default::default(),
        producer,
        sample_rate,
        channels,
        dropped_samples,
    };
    let state_sender = event_sender.clone();
    let listener = stream
        .add_local_listener_with_user_data(data)
        .state_changed(move |_, _, _, new| {
            let status = match new {
                pw::stream::StreamState::Error(message) => AudioStatus::Error(message),
                pw::stream::StreamState::Unconnected => AudioStatus::Idle,
                pw::stream::StreamState::Connecting => AudioStatus::Connecting,
                pw::stream::StreamState::Paused => AudioStatus::Paused,
                pw::stream::StreamState::Streaming => AudioStatus::Streaming,
            };
            log::info!("PipeWire capture state: {}", status.label());
            let _ = state_sender.send(AudioEvent::Status(status));
        })
        .param_changed(|_, data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Ok((media_type, media_subtype)) = format_utils::parse_format(param) else {
                return;
            };
            if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                return;
            }
            if data.format.parse(param).is_ok() {
                data.sample_rate
                    .store(data.format.rate(), Ordering::Relaxed);
                data.channels
                    .store(data.format.channels(), Ordering::Relaxed);
                log::info!(
                    "negotiated audio: {} Hz, {} channels, F32LE",
                    data.format.rate(),
                    data.format.channels()
                );
            }
        })
        .process(|stream, data| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(spa_data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let offset = spa_data.chunk().offset() as usize;
            let size = spa_data.chunk().size() as usize;
            let Some(bytes) = spa_data.data() else {
                return;
            };
            let end = offset.saturating_add(size).min(bytes.len());
            if offset >= end {
                return;
            }
            let channel_count = data.format.channels().max(1) as usize;
            let (floats, _) = bytes[offset..end].as_chunks::<4>();
            let mut producer = data.producer.borrow_mut();
            let mut dropped = 0_u64;
            let mut frame = [0.0_f32; 2];
            let mut channel = 0_usize;
            for bytes in floats {
                let value = f32::from_le_bytes(*bytes);
                if channel == 0 {
                    frame[0] = value;
                    frame[1] = value;
                } else if channel == 1 {
                    frame[1] = value;
                }
                channel += 1;
                if channel == channel_count {
                    if producer.push(frame).is_err() {
                        dropped += 1;
                    }
                    channel = 0;
                }
            }
            if dropped > 0 {
                data.dropped_samples.fetch_add(dropped, Ordering::Relaxed);
            }
        })
        .register()
        .context("register capture callbacks")?;

    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    let object = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let values = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(object),
    )
    .context("serialize audio format")?
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).context("build audio format pod")?];
    stream
        .connect(
            spa::utils::Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut params,
        )
        .context("connect capture stream")?;

    Ok(ActiveCapture {
        _listener: listener,
        _stream: stream,
    })
}

fn sort_sources(sources: &mut [AudioSource]) {
    sources.sort_by(|a, b| {
        let kind_order = |kind| match kind {
            SourceKind::SystemOutput => 0,
            SourceKind::Input => 1,
        };
        kind_order(a.kind).cmp(&kind_order(b.kind)).then_with(|| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        })
    });
}
