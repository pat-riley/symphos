//! MIDI inputs are machine preferences; bindings live in scene signal channels.
//! Held controller values do not time out just because a device is quiet.
use crate::help::HoverHelp;
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
pub mod mapping;
pub mod ui;

pub const VIRTUAL_PORT: i32 = i32::MIN;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: BTreeSet<i32>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: [VIRTUAL_PORT].into(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Message {
    #[default]
    ControlChange,
    NoteVelocity,
    NoteGate,
    PitchBend,
    ChannelPressure,
    PolyPressure,
}
impl Message {
    pub const ALL: [Self; 6] = [
        Self::ControlChange,
        Self::NoteVelocity,
        Self::NoteGate,
        Self::PitchBend,
        Self::ChannelPressure,
        Self::PolyPressure,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::ControlChange => "Control change (CC)",
            Self::NoteVelocity => "Note velocity",
            Self::NoteGate => "Note gate",
            Self::PitchBend => "Pitch bend",
            Self::ChannelPressure => "Channel pressure",
            Self::PolyPressure => "Note pressure",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::ControlChange => {
                "Absolute faders, knobs and switches: CC values 0–127 become 0–1 and hold until the next message. Relative encoders must be configured to send absolute CC on the controller."
            }
            Self::NoteVelocity => {
                "Pad or keyboard strike strength, 0–1 while the note is held; note-off returns it to zero. With Any note, the strongest held note wins. Sustain pedal is honored."
            }
            Self::NoteGate => {
                "One while a matching note is held, zero when released. With Any note, remains one until all matching notes are released. Sustain pedal is honored."
            }
            Self::PitchBend => {
                "14-bit pitch wheel position mapped to −1…+1, with exact zero at the center. Normalize in Output for unipolar motion."
            }
            Self::ChannelPressure => {
                "Channel aftertouch mapped to 0–1. Holds the latest value until changed or disconnected."
            }
            Self::PolyPressure => {
                "Per-note aftertouch mapped to 0–1. Choose a particular note or the strongest pressure across held notes."
            }
        }
    }
    fn numbered(self) -> bool {
        matches!(
            self,
            Self::ControlChange | Self::NoteVelocity | Self::NoteGate | Self::PolyPressure
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Binding {
    pub port: Option<i32>,
    /// None = all channels; otherwise 1–16.
    pub channel: Option<u8>,
    pub message: Message,
    /// None = any note. CC always uses a specific controller number.
    pub number: Option<u8>,
}
impl Default for Binding {
    fn default() -> Self {
        Self {
            port: None,
            channel: None,
            message: Message::ControlChange,
            number: Some(1),
        }
    }
}
impl Binding {
    fn matches(&self, port: i32, channel: u8, message: Message, number: u8) -> bool {
        self.port.is_none_or(|id| id == port)
            && self.channel.is_none_or(|c| c == channel)
            && self.message == message
            && (!message.numbered() || self.number.is_none_or(|n| n == number))
    }
    pub fn matches_event(&self, event: &Event) -> bool {
        let message = if self.message == Message::NoteGate && event.message == Message::NoteVelocity
        {
            Message::NoteGate
        } else {
            event.message
        };
        self.matches(
            event.port,
            event.channel,
            message,
            event.number.unwrap_or(0),
        )
    }
    pub fn valid(&self) -> bool {
        self.channel.is_none_or(|c| (1..=16).contains(&c))
            && self.number.is_none_or(|n| n <= 127)
            && (self.message != Message::ControlChange || self.number.is_some())
    }
    pub fn sanitize(&mut self) {
        self.channel = self.channel.map(|c| c.clamp(1, 16));
        self.number = self.number.map(|n| n.min(127));
        if self.message == Message::ControlChange && self.number.is_none() {
            self.number = Some(1);
        }
    }
}
#[derive(Clone, Debug)]
pub struct Port {
    pub id: i32,
    pub name: String,
}
#[derive(Clone, Debug)]
pub struct Event {
    pub serial: u64,
    pub port: i32,
    pub channel: u8,
    pub message: Message,
    pub number: Option<u8>,
    pub value: u16,
}
impl Event {
    pub fn label(&self) -> String {
        format!(
            "Ch {} · {}{} · {}",
            self.channel,
            self.message.label(),
            self.number.map_or(String::new(), |n| format!(" {n}")),
            self.value
        )
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Value {
    value: f32,
    serial: u64,
}
type Key = (i32, u8, Message, u8);
#[derive(Clone, Default, Debug)]
pub struct Snapshot {
    pub ports: Vec<Port>,
    pub enabled: BTreeSet<i32>,
    pub events: VecDeque<Event>,
    pub serial: u64,
    pub error: Option<String>,
    pub dropped: usize,
    values: BTreeMap<Key, Value>,
}
impl Snapshot {
    pub fn available(&self, binding: &Binding) -> bool {
        self.ports
            .iter()
            .any(|p| self.enabled.contains(&p.id) && binding.port.is_none_or(|id| id == p.id))
    }
    pub fn changed_since(&self, binding: &Binding, serial: u64) -> bool {
        self.values
            .iter()
            .any(|(&(port, ch, message, number), value)| {
                value.serial > serial
                    && self.enabled.contains(&port)
                    && binding.matches(port, ch, message, number)
            })
    }
    pub fn value(&self, binding: &Binding) -> f32 {
        let matches = self
            .values
            .iter()
            .filter(|((port, ch, message, number), _)| {
                self.enabled.contains(port) && binding.matches(*port, *ch, *message, *number)
            });
        if matches!(
            binding.message,
            Message::NoteVelocity | Message::NoteGate | Message::PolyPressure
        ) {
            matches.map(|(_, v)| v.value).fold(0.0, f32::max)
        } else {
            matches
                .max_by_key(|(_, v)| v.serial)
                .map_or(0.0, |(_, v)| v.value)
        }
    }
    /// Per-note input levels for the channel preview. Keep simultaneous notes
    /// separate instead of reducing them to the strongest scalar used by macros.
    pub fn note_levels(&self, binding: &Binding) -> [f32; 128] {
        let mut notes = [0.0_f32; 128];
        for (&(port, channel, message, number), value) in &self.values {
            if matches!(
                message,
                Message::NoteVelocity | Message::NoteGate | Message::PolyPressure
            ) && self.enabled.contains(&port)
                && binding.matches(port, channel, message, number)
            {
                notes[usize::from(number)] = notes[usize::from(number)].max(value.value);
            }
        }
        notes
    }
    pub fn port_name(&self, id: i32) -> String {
        self.ports
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| format!("Disconnected input ({id})"), |p| p.name.clone())
    }
}
#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
struct Parser {
    status: u8,
    data: [u8; 2],
    len: usize,
    sysex: bool,
}
#[cfg(any(target_os = "macos", test))]
impl Parser {
    fn byte(&mut self, byte: u8) -> Option<(u8, [u8; 2])> {
        if byte >= 0xf8 {
            return None;
        } // realtime may interrupt any message
        if byte >= 0x80 {
            self.len = 0;
            if byte >= 0xf0 {
                self.status = 0;
                self.sysex = byte == 0xf0;
            } else {
                self.status = byte;
                self.sysex = false;
            }
            return None;
        }
        if self.sysex || self.status == 0 {
            return None;
        }
        self.data[self.len] = byte;
        self.len += 1;
        let count = if matches!(self.status & 0xf0, 0xc0 | 0xd0) {
            1
        } else {
            2
        };
        if self.len < count {
            return None;
        }
        self.len = 0;
        Some((self.status, self.data))
    }
}
#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
struct Engine {
    snapshot: Snapshot,
    parsers: BTreeMap<i32, Parser>,
    /// Physical keys and sustain are separate to handle polyphony and pedal release.
    keys: BTreeSet<(i32, u8, u8)>,
    sustain: BTreeSet<(i32, u8)>,
}
#[cfg(any(target_os = "macos", test))]
impl Engine {
    fn ports(&mut self, ports: Vec<Port>, prefs: &Preferences) {
        let active: BTreeSet<_> = ports
            .iter()
            .filter(|p| prefs.enabled.contains(&p.id))
            .map(|p| p.id)
            .collect();
        self.snapshot
            .values
            .retain(|(p, _, _, _), _| active.contains(p));
        self.parsers.retain(|p, _| active.contains(p));
        self.keys.retain(|(p, _, _)| active.contains(p));
        self.sustain.retain(|(p, _)| active.contains(p));
        self.snapshot.events.retain(|e| active.contains(&e.port));
        self.snapshot.ports = ports;
        self.snapshot.enabled = active;
    }
    fn set(&mut self, port: i32, ch: u8, message: Message, number: u8, value: f32) {
        self.snapshot.values.insert(
            (port, ch, message, number),
            Value {
                value,
                serial: self.snapshot.serial,
            },
        );
    }
    fn release(&mut self, port: i32, ch: u8, note: u8) {
        for message in [
            Message::NoteVelocity,
            Message::NoteGate,
            Message::PolyPressure,
        ] {
            self.set(port, ch, message, note, 0.0);
        }
    }
    fn bytes(&mut self, port: i32, bytes: &[u8]) {
        if !self.snapshot.enabled.contains(&port) {
            return;
        }
        for &byte in bytes {
            let Some((status, data)) = self.parsers.entry(port).or_default().byte(byte) else {
                continue;
            };
            let ch = (status & 15) + 1;
            let kind = status & 0xf0;
            let (number, value) = (data[0], data[1]);
            self.snapshot.serial = self.snapshot.serial.wrapping_add(1).max(1);
            let event = match kind {
                0x90 if value > 0 => {
                    self.keys.insert((port, ch, number));
                    self.set(
                        port,
                        ch,
                        Message::NoteVelocity,
                        number,
                        f32::from(value) / 127.0,
                    );
                    self.set(port, ch, Message::NoteGate, number, 1.0);
                    Some((Message::NoteVelocity, Some(number), u16::from(value)))
                }
                0x80 | 0x90 => {
                    self.keys.remove(&(port, ch, number));
                    if !self.sustain.contains(&(port, ch)) {
                        self.release(port, ch, number);
                    }
                    None
                }
                0xb0 => {
                    self.set(
                        port,
                        ch,
                        Message::ControlChange,
                        number,
                        f32::from(value) / 127.0,
                    );
                    if number == 64 {
                        if value >= 64 {
                            self.sustain.insert((port, ch));
                        } else {
                            self.sustain.remove(&(port, ch));
                            for note in 0..128 {
                                if !self.keys.contains(&(port, ch, note)) {
                                    self.release(port, ch, note);
                                }
                            }
                        }
                    }
                    if number == 120 || number == 123 {
                        self.keys.retain(|&(p, c, _)| p != port || c != ch);
                        self.sustain.remove(&(port, ch));
                        for note in 0..128 {
                            self.release(port, ch, note);
                        }
                    }
                    if number == 121 {
                        self.snapshot.values.retain(|&(p, c, m, _), _| {
                            p != port
                                || c != ch
                                || m == Message::NoteVelocity
                                || m == Message::NoteGate
                        });
                        self.sustain.remove(&(port, ch));
                        for note in 0..128 {
                            if !self.keys.contains(&(port, ch, note)) {
                                self.release(port, ch, note);
                            }
                        }
                    }
                    (number < 120).then_some((
                        Message::ControlChange,
                        Some(number),
                        u16::from(value),
                    ))
                }
                0xe0 => {
                    let raw = u16::from(number) | (u16::from(value) << 7);
                    let signed = f32::from(raw) - 8192.0;
                    self.set(
                        port,
                        ch,
                        Message::PitchBend,
                        0,
                        signed / if signed < 0.0 { 8192.0 } else { 8191.0 },
                    );
                    Some((Message::PitchBend, None, raw))
                }
                0xd0 => {
                    self.set(
                        port,
                        ch,
                        Message::ChannelPressure,
                        0,
                        f32::from(number) / 127.0,
                    );
                    Some((Message::ChannelPressure, None, u16::from(number)))
                }
                0xa0 => {
                    self.set(
                        port,
                        ch,
                        Message::PolyPressure,
                        number,
                        f32::from(value) / 127.0,
                    );
                    Some((Message::PolyPressure, Some(number), u16::from(value)))
                }
                _ => None,
            };
            if let Some((message, number, value)) = event {
                if self.snapshot.events.len() == 64 {
                    self.snapshot.events.pop_front();
                }
                self.snapshot.events.push_back(Event {
                    serial: self.snapshot.serial,
                    port,
                    channel: ch,
                    message,
                    number,
                    value,
                });
            }
        }
    }
    fn panic(&mut self) {
        self.snapshot.values.clear();
        self.parsers.clear();
        self.keys.clear();
        self.sustain.clear();
        self.snapshot.events.clear();
    }
}

pub struct Runtime {
    pub snapshot: Arc<ArcSwap<Snapshot>>,
    preferences: Arc<ArcSwap<Preferences>>,
    pub preferences_value: Preferences,
    pub save_error: Option<String>,
    panic: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Runtime {
    pub fn start() -> Self {
        let path = crate::workspace::directory().join("midi-inputs.json");
        let preferences_value: Preferences = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let preferences = Arc::new(ArcSwap::from_pointee(preferences_value));
        let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let panic = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (config, output, reset, quit) = (
            preferences.clone(),
            snapshot.clone(),
            panic.clone(),
            stop.clone(),
        );
        let worker = std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                let mut backend = match native::Backend::new(config.clone()) {
                    Ok(b) => b,
                    Err(error) => {
                        output.store(Arc::new(Snapshot {
                            error: Some(error),
                            ..Default::default()
                        }));
                        return;
                    }
                };
                let mut engine = Engine::default();
                let mut last_scan = Instant::now() - Duration::from_secs(1);
                let mut previous = Preferences {
                    enabled: BTreeSet::new(),
                };
                while !quit.load(Ordering::Relaxed) {
                    let serial = engine.snapshot.serial;
                    let dropped = engine.snapshot.dropped;
                    let mut changed = false;
                    let prefs = config.load();
                    if last_scan.elapsed() >= Duration::from_millis(500) || **prefs != previous {
                        // Remove queued data from formerly enabled inputs before a
                        // preference change can turn it into a fresh held value.
                        backend.drain(&mut engine);
                        engine.ports(backend.ports(), &prefs);
                        changed = true;
                        previous = (**prefs).clone();
                        last_scan = Instant::now();
                    }
                    backend.drain(&mut engine);
                    if reset.swap(false, Ordering::Relaxed) {
                        engine.panic();
                        changed = true;
                    }
                    if changed
                        || serial != engine.snapshot.serial
                        || dropped != engine.snapshot.dropped
                    {
                        output.store(Arc::new(engine.snapshot.clone()));
                    }
                    std::thread::sleep(Duration::from_millis(4));
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (config, reset, quit);
                output.store(Arc::new(Snapshot {
                    error: Some("MIDI input is currently supported on macOS.".into()),
                    ..Default::default()
                }));
            }
        });
        Self {
            preferences_value: (**preferences.load()).clone(),
            preferences,
            snapshot,
            save_error: None,
            panic,
            stop,
            worker: Some(worker),
        }
    }
    fn save_preferences(&mut self) {
        self.preferences
            .store(Arc::new(self.preferences_value.clone()));
        let dir = crate::workspace::directory();
        let result = (|| -> anyhow::Result<()> {
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("midi-inputs.json");
            let temp = dir.join("midi-inputs.json.tmp");
            std::fs::write(&temp, serde_json::to_vec_pretty(&self.preferences_value)?)?;
            std::fs::rename(temp, path)?;
            Ok(())
        })();
        self.save_error = result.err().map(|e| e.to_string());
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
pub(crate) fn test_snapshot(bytes: &[u8]) -> Snapshot {
    let mut engine = Engine::default();
    engine.ports(
        vec![Port {
            id: VIRTUAL_PORT,
            name: "Test MIDI".into(),
        }],
        &Preferences::default(),
    );
    engine.bytes(VIRTUAL_PORT, bytes);
    engine.snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> Engine {
        let mut engine = Engine::default();
        engine.ports(
            vec![
                Port {
                    id: 1,
                    name: "Keys".into(),
                },
                Port {
                    id: 2,
                    name: "Faders".into(),
                },
            ],
            &Preferences {
                enabled: [1, 2].into(),
            },
        );
        engine
    }
    fn binding(message: Message, number: Option<u8>) -> Binding {
        Binding {
            message,
            number,
            ..Default::default()
        }
    }
    #[test]
    fn parser_handles_packet_splits_running_status_realtime_and_sysex() {
        let mut engine = engine();
        engine.bytes(1, &[0xb0, 7]);
        assert!(engine.snapshot.values.is_empty());
        engine.bytes(1, &[0xf8, 127, 7, 64, 0xf0, 1, 2, 3, 0xf8, 0xf7, 7, 0]);
        assert_eq!(
            engine
                .snapshot
                .value(&binding(Message::ControlChange, Some(7))),
            64.0 / 127.0
        );
        assert_eq!(engine.snapshot.events.len(), 2);
        engine.bytes(1, &[0xb0, 7, 0]);
        assert_eq!(
            engine
                .snapshot
                .value(&binding(Message::ControlChange, Some(7))),
            0.0
        );
        // Incomplete system-common messages must not reuse channel running status.
        engine.bytes(1, &[0xf2, 4, 5, 7, 127]);
        assert_eq!(engine.snapshot.events.len(), 3);
    }
    #[test]
    fn notes_are_polyphonic_zero_velocity_is_off_and_sustain_releases_cleanly() {
        let mut engine = engine();
        let gate = binding(Message::NoteGate, None);
        let velocity = binding(Message::NoteVelocity, None);
        engine.bytes(1, &[0x90, 60, 127, 64, 64]);
        assert_eq!(engine.snapshot.value(&velocity), 1.0);
        engine.bytes(1, &[0x90, 60, 0]);
        assert_eq!(engine.snapshot.value(&gate), 1.0);
        assert_eq!(engine.snapshot.value(&velocity), 64.0 / 127.0);
        engine.bytes(1, &[0xb0, 64, 127, 0x80, 64, 0]);
        assert_eq!(engine.snapshot.value(&gate), 1.0);
        engine.bytes(1, &[0xb0, 64, 0]);
        assert_eq!(engine.snapshot.value(&gate), 0.0);
        engine.bytes(1, &[0x90, 60, 90, 0xb0, 123, 0]);
        assert_eq!(engine.snapshot.value(&gate), 0.0);
        engine.bytes(1, &[0x90, 60, 90, 0xb0, 120, 0]);
        assert_eq!(engine.snapshot.value(&gate), 0.0);
    }
    #[test]
    fn channel_and_port_filters_bend_center_and_pressure_are_correct() {
        let mut engine = engine();
        engine.bytes(1, &[0xb0, 1, 32, 0xb1, 1, 64]);
        engine.bytes(2, &[0xb0, 1, 127]);
        let mut b = Binding::default();
        assert_eq!(engine.snapshot.value(&b), 1.0);
        b.port = Some(1);
        b.channel = Some(1);
        assert_eq!(engine.snapshot.value(&b), 32.0 / 127.0);
        b.channel = Some(2);
        assert_eq!(engine.snapshot.value(&b), 64.0 / 127.0);
        let bend = binding(Message::PitchBend, None);
        for (bytes, value) in [
            ([0xe0, 0, 0], -1.0),
            ([0xe0, 0, 64], 0.0),
            ([0xe0, 127, 127], 1.0),
        ] {
            engine.bytes(1, &bytes);
            assert_eq!(engine.snapshot.value(&bend), value);
        }
        engine.bytes(1, &[0xd0, 127, 0x90, 60, 80, 0xa0, 60, 100]);
        assert_eq!(
            engine
                .snapshot
                .value(&binding(Message::ChannelPressure, None)),
            1.0
        );
        assert_eq!(
            engine
                .snapshot
                .value(&binding(Message::PolyPressure, Some(60))),
            100.0 / 127.0
        );
        engine.bytes(1, &[0x80, 60, 0]);
        assert_eq!(
            engine
                .snapshot
                .value(&binding(Message::PolyPressure, Some(60))),
            0.0
        );
    }
    #[test]
    fn disabled_disconnected_and_reset_inputs_cannot_leave_stuck_values() {
        let mut engine = engine();
        engine.bytes(1, &[0xb0, 1, 127, 0x90, 60, 127]);
        engine.bytes(2, &[0xb0, 1, 64]);
        engine.ports(
            vec![Port {
                id: 2,
                name: "Faders".into(),
            }],
            &Preferences {
                enabled: [1, 2].into(),
            },
        );
        assert_eq!(engine.snapshot.value(&Binding::default()), 64.0 / 127.0);
        assert_eq!(
            engine.snapshot.value(&binding(Message::NoteGate, None)),
            0.0
        );
        engine.bytes(1, &[0xb0, 1, 127]);
        assert_eq!(engine.snapshot.value(&Binding::default()), 64.0 / 127.0);
        engine.ports(
            vec![Port {
                id: 1,
                name: "Keys".into(),
            }],
            &Preferences {
                enabled: [1].into(),
            },
        );
        assert_eq!(engine.snapshot.value(&Binding::default()), 0.0);
        engine.bytes(1, &[0xb0, 1, 127]);
        engine.panic();
        assert_eq!(engine.snapshot.value(&Binding::default()), 0.0);
        engine.bytes(1, &[0xb0, 1, 64]);
        assert_eq!(engine.snapshot.value(&Binding::default()), 64.0 / 127.0);
    }
    #[test]
    fn midi_drives_existing_signal_mapping_without_audio_and_fades_on_disconnect() {
        use crate::{
            analysis::AnalysisFrame,
            modulation::Route,
            signals::{Bank, Method, Processor, Status},
        };
        let mut engine = engine();
        let mut bank = Bank::default();
        let id = bank.add(Method::Midi).unwrap();
        bank.channels[0].midi.port = Some(1);
        bank.channels[0].smoothing_ms = 200.0;
        let mut processor = Processor::default();
        let frame = AnalysisFrame::default();
        engine.bytes(1, &[0xb0, 1, 0]);
        processor.step_with_midi(&bank, &frame, 0.01, false, &engine.snapshot);
        engine.bytes(1, &[0xb0, 1, 127]);
        let moved = processor.step_with_midi(&bank, &frame, 0.11, false, &engine.snapshot);
        assert!((0.3..0.5).contains(&moved.values[&id].value));
        let held = processor.step_with_midi(&bank, &frame, 100.0, false, &engine.snapshot);
        assert_eq!(held.values[&id].status, Status::Live); // no idle timeout
        let route = Route {
            amount: 0.5,
            output_min: 0.2,
            output_max: 0.8,
            ..Default::default()
        };
        assert!(held.values[&id].mapped(&route) > 0.2);
        engine.ports(Vec::new(), &Preferences::default());
        let mut faded = held;
        for i in 1..=30 {
            faded = processor.step_with_midi(
                &bank,
                &frame,
                100.0 + f64::from(i) * 0.1,
                false,
                &engine.snapshot,
            );
        }
        assert_eq!(faded.values[&id].status, Status::Stale);
        assert_eq!(faded.values[&id].mapped(&route), 0.0);
        engine.ports(
            vec![Port {
                id: 1,
                name: "Keys".into(),
            }],
            &Preferences {
                enabled: [1].into(),
            },
        );
        engine.bytes(1, &[0xb0, 1, 64]);
        let resumed = processor.step_with_midi(&bank, &frame, 104.0, false, &engine.snapshot);
        assert_eq!(resumed.values[&id].value, 64.0 / 127.0); // reconnect skips lag
        bank.channels[0].enabled = false;
        assert_eq!(
            processor
                .step_with_midi(&bank, &frame, 104.1, false, &engine.snapshot)
                .values[&id]
                .status,
            Status::Muted
        );
    }
    #[test]
    fn scene_binding_roundtrips_but_device_preferences_are_separate() {
        let mut bank = crate::signals::Bank::default();
        bank.add(crate::signals::Method::Midi);
        bank.channels[0].midi = Binding {
            port: Some(17),
            channel: Some(4),
            message: Message::NoteGate,
            number: Some(36),
        };
        let json = serde_json::to_value(&bank).unwrap();
        assert!(!json.to_string().contains("preferences"));
        let restored: crate::signals::Bank = serde_json::from_value(json).unwrap();
        assert_eq!(restored, bank);
        assert!(restored.valid());
        bank.channels[0].midi.channel = Some(17);
        assert!(!bank.valid());
        bank.channels[0].sanitize();
        assert!(bank.valid());
        let mut legacy = serde_json::to_value(crate::signals::Definition::recipe(
            1,
            crate::signals::Method::Intensity,
        ))
        .unwrap();
        legacy.as_object_mut().unwrap().remove("midi");
        let restored: crate::signals::Definition = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.midi, Binding::default());
    }
    #[test]
    fn learn_captures_only_new_messages_and_preserves_gate_choice() {
        let mut engine = engine();
        engine.bytes(1, &[0xb0, 1, 127]);
        let mut learn = Some(engine.snapshot.serial);
        engine.bytes(2, &[0x99, 36, 100]);
        let mut binding = binding(Message::NoteGate, None);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            super::ui::binding_editor(ui, &mut binding, &engine.snapshot, &mut learn)
        });
        out.textures_delta.clear();
        assert_eq!(
            binding,
            Binding {
                port: Some(2),
                channel: Some(10),
                message: Message::NoteGate,
                number: Some(36)
            }
        );
        assert_eq!(learn, None);
    }
}
