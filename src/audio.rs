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
pub use linux::AudioEngine;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::AudioEngine;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("Symphos currently supports Linux and macOS; Windows capture is planned.");
