//! Platform capture adapters feeding the shared analyzer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    // macOS currently exposes system playback only; Linux retains input sources.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
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

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::AudioEngine as NativeAudioEngine;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::AudioEngine as NativeAudioEngine;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Symphos currently supports Linux and macOS; Windows capture is planned.");

#[cfg(feature = "development")]
mod demo;

pub enum AudioEngine {
    Native(NativeAudioEngine),
    #[cfg(feature = "development")]
    Demo(demo::DemoEngine),
}
impl AudioEngine {
    pub fn start() -> Self {
        #[cfg(feature = "development")]
        if crate::development::options().is_some_and(|options| options.demo) {
            return Self::Demo(demo::DemoEngine::start());
        }
        Self::Native(NativeAudioEngine::start())
    }
    #[cfg(all(feature = "development", test))]
    pub fn development_mode(demo: bool) -> Self {
        if demo {
            Self::Demo(self::demo::DemoEngine::start())
        } else {
            Self::Native(NativeAudioEngine::start())
        }
    }
    pub fn analysis(&self) -> &crate::analysis::AnalysisRuntime {
        match self {
            Self::Native(engine) => &engine.analysis,
            #[cfg(feature = "development")]
            Self::Demo(engine) => &engine.analysis,
        }
    }
    pub fn refresh_sources(&self) {
        match self {
            Self::Native(engine) => engine.refresh_sources(),
            #[cfg(feature = "development")]
            Self::Demo(engine) => engine.refresh_sources(),
        }
    }
    pub fn select_source(&self, source: AudioSource) {
        match self {
            Self::Native(engine) => engine.select_source(source),
            #[cfg(feature = "development")]
            Self::Demo(engine) => engine.select_source(),
        }
    }
    pub fn drain_events(&mut self) -> std::vec::IntoIter<AudioEvent> {
        match self {
            Self::Native(engine) => engine.drain_events().collect::<Vec<_>>().into_iter(),
            #[cfg(feature = "development")]
            Self::Demo(engine) => engine.drain_events(),
        }
    }
}
