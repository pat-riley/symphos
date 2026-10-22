use super::*;
use crate::modules::{ModuleKind, ModulePane};
fn source() -> AnalysisFrame {
    let samples: Vec<_> = (0..24000)
        .map(|i| {
            let phase = std::f32::consts::TAU * i as f32 / 48000.0;
            let value = 0.1 * (100.0 * phase).sin() + 0.1 * (4000.0 * phase).sin();
            [value, value]
        })
        .collect();
    let settings = AnalysisSettings {
        smoothing: 0.0,
        ..Default::default()
    };
    let mut frame =
        Analyzer::new(4096, settings.window).analyze(&samples, 0, 48000, 2, &settings, 1, 0);
    frame.capture_epoch = 7;
    frame
}
#[test]
fn low_rate_capture_retains_a_full_fft_window_for_processed_inputs() {
    let samples: Vec<_> = (0..16384)
        .map(|i| [0.1 * (std::f32::consts::TAU * 440.0 * i as f32 / 8000.0).sin(); 2])
        .collect();
    let settings = AnalysisSettings {
        fft_size: 16384,
        smoothing: 0.0,
        ..Default::default()
    };
    let frame =
        Analyzer::new(16384, settings.window).analyze(&samples, 0, 8000, 2, &settings, 1, 0);
    assert_eq!(frame.waveform_left.len(), 16384);
    let bus = InputBus {
        id: 1,
        gain_db: 6.0206,
        ..Default::default()
    };
    let out = Inputs::default().process(&[bus], &frame);
    assert!((out[&1].frame.mix_rms / frame.mix_rms - 2.0).abs() < 0.001);
}
fn magnitude(frame: &AnalysisFrame, hz: f32) -> f32 {
    frame
        .bins
        .iter()
        .filter(|b| (b.frequency_hz - hz).abs() < 30.0)
        .map(|b| b.magnitude)
        .fold(0.0, f32::max)
}
#[test]
fn shared_input_filters_waveform_spectrum_and_metrics_without_changing_master() {
    let source = source();
    let mut bus = InputBus {
        id: 1,
        high_pass: true,
        low_hz: 1000.0,
        ..Default::default()
    };
    let mut inputs = Inputs::default();
    let output = inputs.process(&[bus.clone()], &source);
    let filtered = &output[&1].frame;
    assert!(magnitude(filtered, 100.0) < magnitude(&source, 100.0) * 0.02);
    assert!(magnitude(filtered, 4000.0) > magnitude(&source, 4000.0) * 0.9);
    assert!(filtered.mix_rms < source.mix_rms * 0.8);
    assert!(filtered.waveform_left != source.waveform_left);
    let cached = inputs.process(&[bus.clone()], &source);
    assert!(Arc::ptr_eq(&output[&1], &cached[&1]));
    bus.name = "Renamed".into();
    let renamed = inputs.process(&[bus.clone()], &source);
    assert!(Arc::ptr_eq(&cached[&1], &renamed[&1]));
    let revision = renamed[&1].revision;
    bus.high_pass = false;
    bus.gain_db = 6.0206;
    let gained = inputs.process(&[bus], &source);
    assert!(gained[&1].revision != revision);
    assert!((gained[&1].frame.mix_rms / source.mix_rms - 2.0).abs() < 0.001);
    assert!((magnitude(&gained[&1].frame, 100.0) / magnitude(&source, 100.0) - 2.0).abs() < 0.001);
    assert!((source.mix_rms - 0.1).abs() < 0.01);
}
#[test]
fn bypass_is_shared_mute_gate_and_side_cancellation_are_silent() {
    let source = source();
    let mut inputs = Inputs::default();
    let mut bus = InputBus {
        id: 1,
        ..Default::default()
    };
    let first = inputs.process(&[bus.clone()], &source);
    assert!(Arc::ptr_eq(
        &first[&1].frame.waveform_left,
        &source.waveform_left
    ));
    for mode in 0..3 {
        bus.enabled = mode != 0;
        bus.gate = mode == 1;
        bus.gate_db = -1.0;
        bus.channels = if mode == 2 {
            Channels::Side
        } else {
            Channels::Stereo
        };
        let out = inputs.process(&[bus.clone()], &source);
        assert!(out[&1].frame.mix_rms < 1e-7);
        assert!(out[&1].frame.waveform_left.iter().all(|v| v.abs() < 1e-7));
        assert!(out[&1].frame.bins.iter().all(|b| b.magnitude < 1e-7));
    }
}
#[test]
fn connections_preserve_modulation_and_deletion_never_falls_back_to_master() {
    let mut bank = Bank::default();
    let id = bank.add_audio_input().unwrap();
    let mut pane = ModulePane::new(ModuleKind::SpectralBloom);
    pane.audio_input = Connection::Input(id);
    let mut processor = Processor::default();
    let mut source = source();
    source.signals = Some(Arc::new(processor.step(&bank, &source, 0.02, true)));
    pane.ingest_input(&[], &source);
    let output = pane.routed_frame(&source);
    assert!(output.mix_rms > 0.0);
    assert!(Arc::ptr_eq(
        output.signals.as_ref().unwrap(),
        source.signals.as_ref().unwrap()
    ));
    let saved = pane.snapshot();
    let encoded = serde_json::to_value(&saved).unwrap();
    let restored = serde_json::from_value(encoded).unwrap();
    pane.restore(&restored);
    assert_eq!(pane.audio_input, Connection::Input(id));
    bank.audio_inputs.clear();
    pane.reconcile_signals(&bank);
    assert_eq!(pane.audio_input, Connection::Off);
    assert_eq!(pane.routed_frame(&source).mix_rms, 0.0);
    assert!(bank.channels.is_empty());
    assert_eq!(bank.add_audio_input(), Some(id + 1));
    pane.reconcile_signals(&bank);
    assert_eq!(pane.audio_input, Connection::Off);
}
#[test]
fn old_scenes_have_no_new_sources_and_routing_can_drive_a_detector() {
    let old: Bank = serde_json::from_str(r#"{"channels":[],"slots":[],"next_id":1}"#).unwrap();
    assert!(old.audio_inputs.is_empty());
    assert!(old.valid());
    let mut bank = old;
    let id = bank.add_audio_input().unwrap();
    let channel = bank.add(Method::Intensity).unwrap();
    bank.channels[0].audio_input = Connection::Input(id);
    let mut processor = Processor::default();
    let source = source();
    let live = processor.step(&bank, &source, 0.02, true);
    assert!(live.values[&channel].level > 0.0);
    bank.audio_inputs[0].enabled = false;
    let muted = processor.step(&bank, &source, 0.04, true);
    assert_eq!(muted.values[&channel].level, 0.0);
    let saved = serde_json::to_vec(&bank).unwrap();
    let restored: Bank = serde_json::from_slice(&saved).unwrap();
    assert_eq!(bank, restored);
    assert!(restored.valid());
}
#[test]
#[ignore = "manual input-processing benchmark"]
fn benchmark_input_processing() {
    let mut source = source();
    let bus = InputBus {
        id: 1,
        high_pass: true,
        low_hz: 120.0,
        low_pass: true,
        high_hz: 6000.0,
        ..Default::default()
    };
    let mut inputs = Inputs::default();
    let start = Instant::now();
    for i in 1..=300 {
        source.sequence = i;
        std::hint::black_box(inputs.process(std::slice::from_ref(&bus), &source));
    }
    eprintln!(
        "Filtered stereo input + FFT + measurements, 48kHz / 4096 FFT: {:.3} ms per analysis snapshot",
        start.elapsed().as_secs_f64() * 1000.0 / 300.0
    );
    let start = Instant::now();
    for _ in 0..10000 {
        std::hint::black_box(inputs.process(std::slice::from_ref(&bus), &source));
    }
    eprintln!(
        "Cached input, unchanged capture: {:.3} us",
        start.elapsed().as_secs_f64() * 1e6 / 10000.0
    );
}
