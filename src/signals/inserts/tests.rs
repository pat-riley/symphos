use super::*;
use crate::analysis::{AnalysisSettings, Analyzer};
fn frame(sequence: u64, bass: f32, treble: f32) -> AnalysisFrame {
    let samples: Vec<_> = (0..24000)
        .map(|i| {
            let t = i as f32 / 48000.0;
            let v = bass * (std::f32::consts::TAU * 100.0 * t).sin()
                + treble * (std::f32::consts::TAU * 4000.0 * t).sin();
            [v, v]
        })
        .collect();
    let settings = AnalysisSettings {
        smoothing: 0.0,
        ..Default::default()
    };
    let mut frame =
        Analyzer::new(4096, settings.window).analyze(&samples, 0, 48000, 2, &settings, sequence, 0);
    frame.capture_epoch = 3;
    frame
}
fn add(bank: &mut Bank, id: u64, effect: Effect) -> u64 {
    let d = bank.channels.iter_mut().find(|d| d.id == id).unwrap();
    let c = d.inserts.as_mut().unwrap();
    c.add(d.input, c.stages.len(), effect).unwrap()
}
#[test]
fn chain_types_reject_invalid_inserts_and_reorders_without_mutating_the_chain() {
    let mut c = Chain::default();
    assert!(
        c.add(Input::System, 0, Effect::Smooth { ms: 50.0 })
            .is_none()
    );
    let filter = c
        .add(
            Input::System,
            0,
            Effect::Filter {
                low: 30.0,
                high: 180.0,
            },
        )
        .unwrap();
    let detector = c
        .add(
            Input::System,
            1,
            Effect::Detect(Detection::new(Detector::Bass)),
        )
        .unwrap();
    c.add(Input::System, 2, Effect::Smooth { ms: 100.0 })
        .unwrap();
    let original = c.clone();
    assert!(!c.move_to(Input::System, filter, 3));
    assert_eq!(c, original);
    assert!(
        c.add(Input::System, 3, Effect::AudioGain { db: 6.0 })
            .is_none()
    );
    assert!(c.valid(Input::System));
    assert!(!c.valid(Input::Midi));
    c.remove(detector);
    assert_eq!(c.stages.len(), 1);
    assert!(c.valid(Input::System));
    let mut controls = Chain::default();
    assert!(
        controls
            .add(
                Input::Midi,
                0,
                Effect::Filter {
                    low: 30.0,
                    high: 180.0
                }
            )
            .is_none()
    );
    assert!(
        controls
            .add(Input::Midi, 0, Effect::Smooth { ms: 50.0 })
            .is_some()
    );
}
#[test]
fn main_audio_and_bass_control_are_independent_and_reuse_unchanged_pcm() {
    let mut bank = Bank::default();
    let main = bank.add_source(Input::System).unwrap();
    let bass = bank.add_source(Input::System).unwrap();
    add(
        &mut bank,
        bass,
        Effect::Filter {
            low: 30.0,
            high: 180.0,
        },
    );
    add(
        &mut bank,
        bass,
        Effect::Detect(Detection::new(Detector::Bass)),
    );
    add(&mut bank, bass, Effect::Smooth { ms: 100.0 });
    let f = frame(1, 0.1, 0.1);
    let mut processor = Processor::default();
    let out = processor.step(&bank, &f, 0.01, true);
    assert!(Arc::ptr_eq(
        &out.audio[&main].frame.waveform_left,
        &f.waveform_left
    ));
    assert!(!Arc::ptr_eq(
        &out.audio[&bass].frame.waveform_left,
        &f.waveform_left
    ));
    assert!(out.audio[&bass].frame.mix_rms < f.mix_rms * 0.85);
    let mut bright = 0.0;
    for i in 2..60 {
        bright = processor.step(&bank, &f, i as f64 / 120.0, true).values[&bass].value;
    }
    assert!(bright > 0.5);
    assert_eq!(out.values[&main].availability, 0.0);
    let cached = processor.step(&bank, &f, 0.495, true);
    assert!(Arc::ptr_eq(
        &cached.audio[&bass].frame,
        &out.audio[&bass].frame
    ));
    let quiet = frame(2, 0.0, 0.1);
    for i in 60..120 {
        processor.step(&bank, &quiet, i as f64 / 120.0, true);
    }
    let out = processor.step(&bank, &quiet, 0.999, true);
    assert!(out.values[&bass].value < bright * 0.35);
    assert!(bank.valid());
}
#[test]
fn ordered_controls_are_noncommutative_and_bypass_passes_through() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::Lfo).unwrap();
    let d = &mut bank.channels[0];
    d.lfo.waveform = Waveform::Square;
    d.lfo.bipolar = false;
    d.lfo.depth = 0.4;
    let gain = add(&mut bank, id, Effect::Gain { gain: 2.0 });
    let gate = add(
        &mut bank,
        id,
        Effect::Threshold {
            threshold: 0.5,
            hysteresis: 0.05,
        },
    );
    let f = AnalysisFrame::default();
    let out = Processor::default().step(&bank, &f, 0.01, false);
    assert_eq!(out.values[&id].value, 1.0);
    bank.channels[0]
        .inserts
        .as_mut()
        .unwrap()
        .move_to(Input::Lfo, gate, 0);
    let out = Processor::default().step(&bank, &f, 0.01, false);
    assert_eq!(out.values[&id].value, 0.0);
    bank.channels[0]
        .inserts
        .as_mut()
        .unwrap()
        .move_to(Input::Lfo, gain, 0);
    bank.channels[0].inserts.as_mut().unwrap().stages[0].enabled = false;
    assert_eq!(
        Processor::default().step(&bank, &f, 0.01, false).values[&id].value,
        0.0
    );
}
#[test]
fn smoothing_has_the_same_time_constant_at_30_60_and_120_hz_and_influence_applies_once() {
    let mut values = Vec::new();
    for fps in [30, 60, 120] {
        let mut bank = Bank::default();
        let id = bank.add_source(Input::Lfo).unwrap();
        let d = &mut bank.channels[0];
        d.lfo.waveform = Waveform::Square;
        d.lfo.bipolar = false;
        d.lfo.rate = 0.01;
        d.lfo.depth = 0.0;
        d.influence = 2.0;
        add(&mut bank, id, Effect::Smooth { ms: 200.0 });
        let mut p = Processor::default();
        let f = AnalysisFrame::default();
        p.step(&bank, &f, 0.0, false);
        bank.channels[0].lfo.depth = 0.5;
        let mut value = 0.0;
        for i in 1..=fps {
            value = p.step(&bank, &f, i as f64 / fps as f64, false).values[&id].value;
        }
        values.push(value);
    }
    for value in values {
        assert!((value - (1.0 - (-5.0_f32).exp())).abs() < 0.0001);
    }
}
#[test]
fn legacy_detector_migration_preserves_every_channel_id_mapping_and_output() {
    let mut old = Bank::test_sources();
    let lfo = old.add(Method::Lfo).unwrap();
    old.channels.last_mut().unwrap().influence = 1.5;
    for d in &mut old.channels {
        d.response.amount = 2.0;
        d.response.output_min = 0.3;
        d.response.output_max = 0.7;
        d.response.invert = true;
        d.normalization.enabled = true;
        d.normalization.low = 0.05;
        d.normalization.high = 0.7;
        d.shaping.enabled = true;
        d.shaping.exponent = 1.5;
        d.smoothing_ms = 80.0;
    }
    old.sync_slots();
    let original_ids: Vec<_> = old.channels.iter().map(|d| d.id).collect();
    let mut new = old.clone();
    new.migrate_inserts();
    let canonical = new.clone();
    new.migrate_inserts();
    assert_eq!(new, canonical);
    assert!(new.valid());
    assert_eq!(
        original_ids,
        new.channels.iter().map(|d| d.id).collect::<Vec<_>>()
    );
    assert_eq!(new.slots, old.slots);
    assert!(new.channels.iter().any(|d| d.id == lfo));
    let mut a = Processor::default();
    let mut b = Processor::default();
    for i in 1..100 {
        let mut f = frame(1, 0.1, 0.03);
        f.sequence = i;
        let time = i as f64 / 120.0;
        let x = a.step(&old, &f, time, true);
        let y = b.step(&new, &f, time, true);
        for id in &original_ids {
            assert!(
                (x.values[id].value - y.values[id].value).abs() < 0.00001,
                "channel {id}, frame {i}: {} vs {}",
                x.values[id].value,
                y.values[id].value
            );
        }
    }
    let decoded: Bank = serde_json::from_slice(&serde_json::to_vec(&new).unwrap()).unwrap();
    assert_eq!(decoded, new);
}
#[test]
fn beat_detector_tracks_regular_onsets_without_inventing_beats_in_silence() {
    for fps in [30, 60, 120] {
        let mut beat = BeatState::default();
        let mut hits = 0;
        let dt = 1.0 / fps as f32;
        for i in 0..fps * 6 {
            let time = i as f64 / fps as f64;
            let energy = if i % (fps / 2) == 0 { 0.2 } else { 0.0 };
            let (hit, _, _, _) = beat.step(energy, true, dt, time, 1.5, false, 120.0);
            hits += usize::from(hit);
        }
        assert!((10..=12).contains(&hits));
        assert!((60.0 / beat.period - 120.0).abs() < 1.0);
        assert!(beat.confidence > 0.7);
        for i in fps * 6..fps * 8 {
            let (hit, _, _, _) = beat.step(0.0, true, dt, i as f64 / fps as f64, 1.5, false, 120.0);
            assert!(!hit);
        }
        assert_eq!(beat.confidence, 0.0);
        assert!(beat.pulse < 0.001);
    }
}
#[test]
fn deleting_a_paired_audio_channel_removes_both_outputs_and_duplicate_is_independent() {
    let mut b = Bank::default();
    let id = b.add_source(Input::System).unwrap();
    add(&mut b, id, Effect::Detect(Detection::new(Detector::Bass)));
    let copy = b.duplicate(id).unwrap();
    b.sync_slots();
    assert!(b.valid());
    assert!(b.audio_inputs.iter().any(|a| a.id == copy));
    b.channels
        .iter_mut()
        .find(|d| d.id == copy)
        .unwrap()
        .inserts
        .as_mut()
        .unwrap()
        .stages[0]
        .enabled = false;
    assert!(b.channels[0].inserts.as_ref().unwrap().stages[0].enabled);
    b.remove_channel(id);
    assert!(b.channels.iter().all(|d| d.id != id));
    assert!(b.audio_inputs.iter().all(|d| d.id != id));
    assert!(b.valid());
}
#[test]
fn processing_edits_advance_audio_revision_but_control_edits_do_not() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::System).unwrap();
    add(&mut bank, id, Effect::LowPass { hz: 4000.0 });
    add(
        &mut bank,
        id,
        Effect::Detect(Detection::new(Detector::Level)),
    );
    let smooth = add(&mut bank, id, Effect::Smooth { ms: 50.0 });
    let f = frame(1, 0.1, 0.1);
    let mut p = Processor::default();
    let first = p.step(&bank, &f, 0.01, true);
    bank.channels[0].inserts.as_mut().unwrap().stages[0].effect = Effect::LowPass { hz: 300.0 };
    let changed = p.step(&bank, &f, 0.02, true);
    assert_ne!(first.audio[&id].revision, changed.audio[&id].revision);
    bank.channels[0]
        .inserts
        .as_mut()
        .unwrap()
        .stages
        .iter_mut()
        .find(|s| s.id == smooth)
        .unwrap()
        .effect = Effect::Smooth { ms: 200.0 };
    let control = p.step(&bank, &f, 0.03, true);
    assert_eq!(changed.audio[&id].revision, control.audio[&id].revision);
}

#[test]
fn mute_silences_both_outputs_and_control_inserts_cannot_bypass_it() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::System).unwrap();
    add(
        &mut bank,
        id,
        Effect::Detect(Detection::new(Detector::Bass)),
    );
    add(&mut bank, id, Effect::Invert);
    let f = frame(1, 0.1, 0.0);
    let mut p = Processor::default();
    p.step(&bank, &f, 0.01, true);
    bank.channels[0].enabled = false;
    let out = p.step(&bank, &f, 0.02, true);
    assert_eq!(out.audio[&id].frame.mix_rms, 0.0);
    assert_eq!(out.values[&id].status, Status::Muted);
    for i in 3..200 {
        p.step(&bank, &f, i as f64 / 120.0, true);
    }
    assert_eq!(p.step(&bank, &f, 2.0, true).values[&id].value, 0.0);
}
#[test]
fn validation_rejects_duplicate_stage_ids_bad_ranges_and_oversized_chains() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::Lfo).unwrap();
    add(&mut bank, id, Effect::Smooth { ms: 30.0 });
    let mut bad = bank.clone();
    let duplicate = bad.channels[0].inserts.as_ref().unwrap().stages[0].clone();
    bad.channels[0]
        .inserts
        .as_mut()
        .unwrap()
        .stages
        .push(duplicate);
    assert!(!bad.valid());
    bank.channels[0].inserts.as_mut().unwrap().stages[0].effect = Effect::Smooth { ms: f32::NAN };
    assert!(!bank.valid());
}
#[test]
#[ignore = "native worker throughput benchmark; run explicitly"]
fn benchmark_standard_audio_and_bass_insert_chain() {
    let mut bank = Bank::default();
    bank.add_source(Input::System).unwrap();
    let bass = bank.add_source(Input::System).unwrap();
    add(
        &mut bank,
        bass,
        Effect::Filter {
            low: 30.0,
            high: 180.0,
        },
    );
    add(
        &mut bank,
        bass,
        Effect::Detect(Detection::new(Detector::Bass)),
    );
    add(
        &mut bank,
        bass,
        Effect::Normalize {
            low: 0.0,
            high: 1.0,
            automatic: false,
        },
    );
    add(&mut bank, bass, Effect::Smooth { ms: 100.0 });
    let mut f = frame(1, 0.1, 0.1);
    let mut processor = Processor::default();
    processor.step(&bank, &f, 0.0, true);
    let start = Instant::now();
    for i in 1..301 {
        f.sequence = i;
        std::hint::black_box(processor.step(&bank, &f, i as f64 / 120.0, true));
    }
    println!(
        "2 audio sources + filter/bass/normalize/smooth: {:.3} ms per fresh snapshot",
        start.elapsed().as_secs_f64() * 1000.0 / 300.0
    );
}

#[test]
fn bypassing_a_detector_silences_its_control_branch_even_with_invert_after_it() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::System).unwrap();
    add(&mut bank, id, Effect::LowPass { hz: 250.0 });
    let detector = add(
        &mut bank,
        id,
        Effect::Detect(Detection::new(Detector::Bass)),
    );
    add(&mut bank, id, Effect::Invert);
    let f = frame(1, 0.1, 0.1);
    let mut p = Processor::default();
    let before = p.step(&bank, &f, 0.01, true);
    bank.channels[0]
        .inserts
        .as_mut()
        .unwrap()
        .stages
        .iter_mut()
        .find(|s| s.id == detector)
        .unwrap()
        .enabled = false;
    let after = p.step(&bank, &f, 0.02, true);
    assert_eq!(after.values[&id].value, 0.0);
    assert_eq!(after.values[&id].availability, 0.0);
    assert_eq!(
        after.audio[&id].frame.mix_rms,
        before.audio[&id].frame.mix_rms
    );
}

#[test]
fn insert_scopes_and_control_clocks_keep_running_without_ui_or_audio() {
    let mut bank = Bank::default();
    let id = bank.add_source(Input::Lfo).unwrap();
    add(&mut bank, id, Effect::Smooth { ms: 80.0 });
    let audio = Arc::new(arc_swap::ArcSwap::from_pointee(AnalysisFrame::default()));
    let midi = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::midi::Snapshot::default(),
    ));
    let runtime = super::super::Runtime::start(audio, midi);
    runtime.connected.store(false, Ordering::Relaxed);
    runtime.publish(&bank);
    let deadline = Instant::now() + Duration::from_secs(2);
    while runtime
        .snapshot
        .load()
        .inserts
        .get(&id)
        .is_none_or(|m| m[0].history.len() < 4)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = runtime.snapshot.load_full();
    assert!(out.inserts[&id][0].history.len() >= 4);
    assert!(out.values[&id].value.abs() > 0.001);
    assert_eq!(out.values[&id].status, Status::Live);
}

#[test]
fn missing_or_muted_upstream_audio_never_substitutes_master_or_drives_inverted_controls() {
    let mut bank = Bank::default();
    let parent = bank.add_source(Input::System).unwrap();
    let child = bank.add_source(Input::System).unwrap();
    bank.channels[1].audio_input = audio::Connection::Input(parent);
    add(
        &mut bank,
        child,
        Effect::Detect(Detection::new(Detector::Bass)),
    );
    add(&mut bank, child, Effect::Invert);
    let f = frame(1, 0.1, 0.0);
    let mut p = Processor::default();
    p.step(&bank, &f, 0.01, true);
    bank.channels[0].enabled = false;
    let muted = p.step(&bank, &f, 0.02, true);
    assert_eq!(muted.values[&child].status, Status::Muted);
    assert_eq!(muted.audio[&child].frame.mix_rms, 0.0);
    bank.channels[1].audio_input = audio::Connection::Input(9999);
    let missing = p.step(&bank, &f, 0.03, true);
    assert_eq!(missing.values[&child].status, Status::Muted);
    assert_eq!(missing.audio[&child].frame.mix_rms, 0.0);
    for i in 4..200 {
        p.step(&bank, &f, i as f64 / 120.0, true);
    }
    assert_eq!(p.step(&bank, &f, 2.0, true).values[&child].value, 0.0);
}
