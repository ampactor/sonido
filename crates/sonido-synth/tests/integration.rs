//! Integration tests for sonido-synth crate.
//!
//! Tests cover voice management, oscillators, envelopes, modulation matrix,
//! and polyphonic output summing.

use sonido_synth::{
    AdsrEnvelope, EnvelopeState, ModDestination, ModSourceId, ModulationMatrix, ModulationRoute,
    ModulationValues, Oscillator, OscillatorWaveform, Voice, VoiceAllocationMode, VoiceManager,
    midi_to_freq,
};

const SR: f32 = 48000.0;

// ---------------------------------------------------------------------------
// 1. Voice allocation and stealing
// ---------------------------------------------------------------------------

#[test]
fn voice_allocation_fills_all_slots() {
    let mut mgr: VoiceManager<4> = VoiceManager::new(SR);

    mgr.note_on(60, 100);
    mgr.note_on(64, 100);
    mgr.note_on(67, 100);
    mgr.note_on(72, 100);

    assert_eq!(mgr.active_voice_count(), 4);
}

#[test]
fn voice_stealing_oldest_replaces_first_note() {
    let mut mgr: VoiceManager<4> = VoiceManager::new(SR);
    mgr.set_allocation_mode(VoiceAllocationMode::OldestNote);

    // Fill all 4 slots
    mgr.note_on(60, 100);
    mgr.note_on(64, 100);
    mgr.note_on(67, 100);
    mgr.note_on(72, 100);

    // 5th note should steal the oldest (note 60)
    mgr.note_on(76, 100);

    assert_eq!(
        mgr.active_voice_count(),
        4,
        "count stays at polyphony limit"
    );

    let has_60 = mgr.voices().iter().any(|v| v.is_active() && v.note() == 60);
    assert!(!has_60, "oldest note (60) should have been stolen");

    let has_76 = mgr.voices().iter().any(|v| v.is_active() && v.note() == 76);
    assert!(has_76, "new note (76) should be present");
}

#[test]
fn voice_stealing_lowest_replaces_lowest_pitch() {
    let mut mgr: VoiceManager<3> = VoiceManager::new(SR);
    mgr.set_allocation_mode(VoiceAllocationMode::LowestNote);

    mgr.note_on(64, 100);
    mgr.note_on(67, 100);
    mgr.note_on(72, 100);

    // Steal lowest pitch (64)
    mgr.note_on(80, 100);

    let has_64 = mgr.voices().iter().any(|v| v.is_active() && v.note() == 64);
    assert!(!has_64, "lowest note (64) should have been stolen");
}

#[test]
fn voice_stealing_highest_replaces_highest_pitch() {
    let mut mgr: VoiceManager<3> = VoiceManager::new(SR);
    mgr.set_allocation_mode(VoiceAllocationMode::HighestNote);

    mgr.note_on(60, 100);
    mgr.note_on(64, 100);
    mgr.note_on(72, 100);

    // Steal highest pitch (72)
    mgr.note_on(50, 100);

    let has_72 = mgr.voices().iter().any(|v| v.is_active() && v.note() == 72);
    assert!(!has_72, "highest note (72) should have been stolen");
}

#[test]
fn note_off_releases_voice_then_becomes_inactive() {
    let mut mgr: VoiceManager<4> = VoiceManager::new(SR);

    // Use very short envelope so release completes quickly
    for voice in mgr.voices_mut() {
        voice.amp_env.set_attack_ms(0.1);
        voice.amp_env.set_decay_ms(0.1);
        voice.amp_env.set_sustain(0.5);
        voice.amp_env.set_release_ms(1.0);
    }

    mgr.note_on(60, 100);
    assert_eq!(mgr.active_voice_count(), 1);

    // Run through attack/decay so the voice is sounding
    for _ in 0..500 {
        mgr.process();
    }
    assert_eq!(mgr.active_voice_count(), 1);

    // Release
    mgr.note_off(60);

    // Run enough samples for the 1ms release to complete (~48 samples + margin)
    for _ in 0..5000 {
        mgr.process();
    }

    assert_eq!(
        mgr.active_voice_count(),
        0,
        "voice should be inactive after release completes"
    );
}

#[test]
fn free_voice_reused_before_stealing() {
    let mut mgr: VoiceManager<4> = VoiceManager::new(SR);

    // Short envelope for quick release
    for voice in mgr.voices_mut() {
        voice.amp_env.set_attack_ms(0.1);
        voice.amp_env.set_decay_ms(0.1);
        voice.amp_env.set_sustain(0.5);
        voice.amp_env.set_release_ms(0.5);
    }

    mgr.note_on(60, 100);
    mgr.note_on(64, 100);

    // Advance through attack
    for _ in 0..200 {
        mgr.process();
    }

    // Release note 60
    mgr.note_off(60);

    // Run through release
    for _ in 0..5000 {
        mgr.process();
    }

    assert_eq!(mgr.active_voice_count(), 1, "only note 64 remains");

    // New note should reuse the freed slot, not steal from 64
    mgr.note_on(72, 100);
    assert_eq!(mgr.active_voice_count(), 2);

    let has_64 = mgr.voices().iter().any(|v| v.is_active() && v.note() == 64);
    assert!(has_64, "note 64 should still be playing");
}

// ---------------------------------------------------------------------------
// 2. Polyphonic output summing
// ---------------------------------------------------------------------------

#[test]
fn polyphonic_output_is_sum_of_individual_voices() {
    // We create two standalone voices and one VoiceManager<2>, play the same
    // notes, and verify the manager output equals the sum of the individuals.
    let note_a: u8 = 69; // A4
    let note_e: u8 = 76; // E5
    let velocity: u8 = 100;
    let num_samples = 512;

    // Standalone voices
    let mut voice_a = Voice::new(SR);
    let mut voice_b = Voice::new(SR);
    voice_a.note_on(note_a, velocity);
    voice_b.note_on(note_e, velocity);

    let mut standalone_sum = Vec::with_capacity(num_samples);
    for _ in 0..num_samples {
        standalone_sum.push(voice_a.process() + voice_b.process());
    }

    // VoiceManager
    let mut mgr: VoiceManager<2> = VoiceManager::new(SR);
    mgr.note_on(note_a, velocity);
    mgr.note_on(note_e, velocity);

    let mut mgr_out = Vec::with_capacity(num_samples);
    for _ in 0..num_samples {
        mgr_out.push(mgr.process());
    }

    // Compare sample-by-sample
    for (i, (a, b)) in standalone_sum.iter().zip(mgr_out.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-5,
            "sample {i}: standalone={a}, manager={b}, diff={}",
            (a - b).abs()
        );
    }
}

#[test]
fn polyphonic_output_nonzero_for_chord() {
    let mut mgr: VoiceManager<8> = VoiceManager::new(SR);

    // C major triad
    mgr.note_on(60, 100);
    mgr.note_on(64, 100);
    mgr.note_on(67, 100);

    let mut energy = 0.0_f64;
    for _ in 0..2048 {
        let s = mgr.process();
        energy += (s as f64) * (s as f64);
    }

    assert!(energy > 0.0, "chord should produce nonzero energy");
}

// ---------------------------------------------------------------------------
// 3. Modulation matrix routing
// ---------------------------------------------------------------------------

#[test]
fn mod_matrix_bipolar_route_scales_source() {
    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::FilterCutoff,
        0.5,
    ));

    let mut values = ModulationValues::new();
    values.lfo1 = 0.8;

    let result = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    // bipolar: source * amount = 0.8 * 0.5 = 0.4
    assert!((result - 0.4).abs() < 1e-5, "expected 0.4, got {result}");
}

#[test]
fn mod_matrix_unipolar_route_maps_correctly() {
    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    matrix.add_route(ModulationRoute::unipolar(
        ModSourceId::FilterEnv,
        ModDestination::FilterCutoff,
        1.0,
    ));

    // Unipolar mapping: unipolar = (source + 1) * 0.5, scaled by amount
    // For source = 1.0: unipolar = (1.0 + 1.0) * 0.5 = 1.0, * 1.0 = 1.0
    let mut values = ModulationValues::new();
    values.filter_env = 1.0;
    let result = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    assert!(
        (result - 1.0).abs() < 1e-5,
        "source=1.0, amount=1.0 -> expected 1.0, got {result}"
    );

    // For source = -1.0: unipolar = (-1.0 + 1.0) * 0.5 = 0.0, * 1.0 = 0.0
    values.filter_env = -1.0;
    let result = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    assert!(
        result.abs() < 1e-5,
        "source=-1.0, amount=1.0 -> expected 0.0, got {result}"
    );

    // For source = 0.0: unipolar = (0.0 + 1.0) * 0.5 = 0.5, * 1.0 = 0.5
    values.filter_env = 0.0;
    let result = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    assert!(
        (result - 0.5).abs() < 1e-5,
        "source=0.0, amount=1.0 -> expected 0.5, got {result}"
    );
}

#[test]
fn mod_matrix_multiple_routes_to_same_destination_sum() {
    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::FilterCutoff,
        0.5,
    ));
    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo2,
        ModDestination::FilterCutoff,
        0.3,
    ));

    let mut values = ModulationValues::new();
    values.lfo1 = 1.0;
    values.lfo2 = 1.0;

    let result = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    // 1.0 * 0.5 + 1.0 * 0.3 = 0.8
    assert!((result - 0.8).abs() < 1e-5, "expected 0.8, got {result}");
}

#[test]
fn mod_matrix_negative_amount_inverts() {
    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::Osc1Pitch,
        -0.5,
    ));

    let mut values = ModulationValues::new();
    values.lfo1 = 1.0;

    let result = matrix.get_modulation(ModDestination::Osc1Pitch, &values);
    assert!(
        (result - (-0.5)).abs() < 1e-5,
        "expected -0.5, got {result}"
    );
}

#[test]
fn mod_matrix_different_destinations_are_independent() {
    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::FilterCutoff,
        0.7,
    ));
    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo2,
        ModDestination::Osc1Pitch,
        0.4,
    ));

    let mut values = ModulationValues::new();
    values.lfo1 = 1.0;
    values.lfo2 = 1.0;

    let cutoff = matrix.get_modulation(ModDestination::FilterCutoff, &values);
    let pitch = matrix.get_modulation(ModDestination::Osc1Pitch, &values);

    assert!(
        (cutoff - 0.7).abs() < 1e-5,
        "FilterCutoff: expected 0.7, got {cutoff}"
    );
    assert!(
        (pitch - 0.4).abs() < 1e-5,
        "Osc1Pitch: expected 0.4, got {pitch}"
    );
}

#[test]
fn mod_matrix_capacity_enforced() {
    let mut matrix: ModulationMatrix<2> = ModulationMatrix::new();

    assert!(matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::FilterCutoff,
        0.5,
    )));
    assert!(matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo2,
        ModDestination::Osc1Pitch,
        0.5,
    )));
    assert!(
        !matrix.add_route(ModulationRoute::new(
            ModSourceId::AmpEnv,
            ModDestination::Amplitude,
            0.5,
        )),
        "should reject when full"
    );
    assert_eq!(matrix.route_count(), 2);
}

// ---------------------------------------------------------------------------
// 4. Oscillator waveforms
// ---------------------------------------------------------------------------

/// Count positive-going zero crossings over a given number of samples.
fn count_zero_crossings(osc: &mut Oscillator, samples: usize) -> i32 {
    let mut crossings = 0i32;
    let mut prev = 0.0_f32;
    for _ in 0..samples {
        let s = osc.advance();
        if prev <= 0.0 && s > 0.0 {
            crossings += 1;
        }
        prev = s;
    }
    crossings
}

/// Verify output is bounded within [-bound, bound] and never NaN/Inf.
fn assert_bounded(osc: &mut Oscillator, samples: usize, bound: f32, label: &str) {
    for i in 0..samples {
        let s = osc.advance();
        assert!(s.is_finite(), "{label}: sample {i} is not finite ({s})");
        assert!(
            s.abs() <= bound,
            "{label}: sample {i} = {s} exceeds +/-{bound}"
        );
    }
}

/// Verify output is not all zeros.
fn assert_nontrivial(osc: &mut Oscillator, samples: usize, label: &str) {
    let mut energy = 0.0_f64;
    for _ in 0..samples {
        let s = osc.advance() as f64;
        energy += s * s;
    }
    assert!(energy > 0.0, "{label}: output is silence");
}

#[test]
fn oscillator_sine_frequency_and_bounds() {
    let mut osc = Oscillator::new(SR);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Sine);

    let crossings = count_zero_crossings(&mut osc, SR as usize);
    assert!(
        (crossings - 440).abs() <= 2,
        "sine 440 Hz: expected ~440 crossings, got {crossings}"
    );

    osc.reset();
    assert_bounded(&mut osc, 10000, 1.01, "sine");
}

#[test]
fn oscillator_saw_frequency_and_bounds() {
    let mut osc = Oscillator::new(SR);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Saw);

    let crossings = count_zero_crossings(&mut osc, SR as usize);
    assert!(
        (crossings - 440).abs() <= 2,
        "saw 440 Hz: expected ~440 crossings, got {crossings}"
    );

    osc.reset();
    // PolyBLEP can slightly overshoot
    assert_bounded(&mut osc, 10000, 1.5, "saw");
}

#[test]
fn oscillator_square_frequency_and_bounds() {
    let mut osc = Oscillator::new(SR);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Square);

    let crossings = count_zero_crossings(&mut osc, SR as usize);
    assert!(
        (crossings - 440).abs() <= 2,
        "square 440 Hz: expected ~440 crossings, got {crossings}"
    );

    osc.reset();
    assert_bounded(&mut osc, 10000, 1.5, "square");
}

#[test]
fn oscillator_triangle_nontrivial_and_bounded() {
    let mut osc = Oscillator::new(SR);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Triangle);

    assert_nontrivial(&mut osc, 10000, "triangle");

    osc.reset();
    // Triangle is a leaky-integrated square; overshoot can exceed 1.5 briefly
    assert_bounded(&mut osc, 10000, 2.0, "triangle");
}

#[test]
fn oscillator_pulse_nontrivial_and_bounded() {
    let mut osc = Oscillator::new(SR);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Pulse(0.25));

    assert_nontrivial(&mut osc, 10000, "pulse 25%");

    osc.reset();
    assert_bounded(&mut osc, 10000, 1.5, "pulse 25%");
}

#[test]
fn oscillator_noise_nontrivial_and_bounded() {
    let mut osc = Oscillator::new(SR);
    osc.set_waveform(OscillatorWaveform::Noise);

    assert_nontrivial(&mut osc, 10000, "noise");

    osc.reset();
    assert_bounded(&mut osc, 10000, 1.01, "noise");
}

#[test]
fn oscillator_frequency_varies_correctly() {
    for &freq in &[100.0, 440.0, 1000.0, 5000.0] {
        let mut osc = Oscillator::new(SR);
        osc.set_frequency(freq);
        osc.set_waveform(OscillatorWaveform::Sine);

        let crossings = count_zero_crossings(&mut osc, SR as usize);
        let tolerance = if freq > 2000.0 { 5 } else { 2 };
        assert!(
            (crossings - freq as i32).abs() <= tolerance,
            "sine {freq} Hz: expected ~{} crossings, got {crossings}",
            freq as i32
        );
    }
}

#[test]
fn oscillator_all_waveforms_produce_output_at_low_frequency() {
    let waveforms = [
        OscillatorWaveform::Sine,
        OscillatorWaveform::Saw,
        OscillatorWaveform::Square,
        OscillatorWaveform::Triangle,
        OscillatorWaveform::Pulse(0.5),
        OscillatorWaveform::Noise,
    ];

    for wf in &waveforms {
        let mut osc = Oscillator::new(SR);
        osc.set_frequency(100.0);
        osc.set_waveform(*wf);
        assert_nontrivial(&mut osc, 4800, &format!("{wf:?} at 100 Hz"));
    }
}

// ---------------------------------------------------------------------------
// 5. ADSR envelope stages and timing
// ---------------------------------------------------------------------------

#[test]
fn adsr_attack_reaches_peak() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(5.0);
    env.set_decay_ms(50.0);
    env.set_sustain(0.7);
    env.set_release_ms(100.0);

    env.gate_on();

    // Run for well past the 5ms attack (5ms * 48 = 240 samples, go 5x)
    let mut peak = 0.0_f32;
    for _ in 0..1200 {
        let level = env.advance();
        if level > peak {
            peak = level;
        }
    }

    assert!(
        peak >= 0.99,
        "attack should reach near 1.0, peak was {peak}"
    );
}

#[test]
fn adsr_decay_settles_to_sustain() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(1.0);
    env.set_decay_ms(10.0);
    env.set_sustain(0.6);
    env.set_release_ms(50.0);

    env.gate_on();

    // Run through attack and decay (generous margin)
    for _ in 0..10000 {
        env.advance();
    }

    assert_eq!(env.state(), EnvelopeState::Sustain);
    assert!(
        (env.level() - 0.6).abs() < 0.01,
        "sustain level should be 0.6, got {}",
        env.level()
    );
}

#[test]
fn adsr_release_decays_to_zero() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(0.5);
    env.set_decay_ms(1.0);
    env.set_sustain(0.8);
    env.set_release_ms(20.0);

    env.gate_on();

    // Reach sustain
    for _ in 0..5000 {
        env.advance();
    }
    assert_eq!(env.state(), EnvelopeState::Sustain);

    // Release
    env.gate_off();
    assert_eq!(env.state(), EnvelopeState::Release);

    // Run through release (20ms * 48 = 960 samples; ~10x time constants = 9600)
    for _ in 0..20000 {
        env.advance();
    }

    assert_eq!(env.state(), EnvelopeState::Idle);
    assert!(
        env.level() < 0.001,
        "level should be near zero after release, got {}",
        env.level()
    );
}

#[test]
fn adsr_full_cycle_state_transitions() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(2.0);
    env.set_decay_ms(5.0);
    env.set_sustain(0.5);
    env.set_release_ms(10.0);

    assert_eq!(env.state(), EnvelopeState::Idle);

    env.gate_on();
    assert_eq!(env.state(), EnvelopeState::Attack);

    // Advance to Decay
    for _ in 0..2000 {
        env.advance();
        if env.state() == EnvelopeState::Decay {
            break;
        }
    }
    assert_eq!(env.state(), EnvelopeState::Decay);

    // Advance to Sustain
    for _ in 0..10000 {
        env.advance();
        if env.state() == EnvelopeState::Sustain {
            break;
        }
    }
    assert_eq!(env.state(), EnvelopeState::Sustain);

    env.gate_off();
    assert_eq!(env.state(), EnvelopeState::Release);

    // Advance to Idle
    for _ in 0..30000 {
        env.advance();
        if env.state() == EnvelopeState::Idle {
            break;
        }
    }
    assert_eq!(env.state(), EnvelopeState::Idle);
}

#[test]
fn adsr_timing_attack_duration_approximately_correct() {
    // With exponential attack toward overshoot target of 1.2,
    // the envelope hits 1.0 before the full time constant.
    // We verify it transitions to Decay within a reasonable range.
    let attack_ms = 10.0;
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(attack_ms);
    env.set_decay_ms(200.0);
    env.set_sustain(0.8);

    env.gate_on();

    let mut sample_count = 0u32;
    for _ in 0..10000 {
        env.advance();
        sample_count += 1;
        if env.state() == EnvelopeState::Decay {
            break;
        }
    }

    let actual_ms = sample_count as f32 / SR * 1000.0;
    // Exponential with overshoot: should reach peak somewhat before the nominal time
    // but within a factor of 2.
    assert!(
        actual_ms < attack_ms * 2.0,
        "attack took {actual_ms}ms, expected less than {}ms",
        attack_ms * 2.0
    );
    assert!(
        actual_ms > 0.5,
        "attack completed suspiciously fast: {actual_ms}ms"
    );
}

#[test]
fn adsr_level_monotonically_increases_during_attack() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(10.0);
    env.set_decay_ms(200.0);
    env.set_sustain(0.7);

    env.gate_on();

    let mut prev = 0.0_f32;
    for _ in 0..500 {
        let level = env.advance();
        if env.state() != EnvelopeState::Attack {
            break;
        }
        assert!(
            level >= prev - 1e-6,
            "attack should be monotonically increasing: prev={prev}, current={level}"
        );
        prev = level;
    }
}

#[test]
fn adsr_level_monotonically_decreases_during_release() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(0.5);
    env.set_decay_ms(1.0);
    env.set_sustain(0.7);
    env.set_release_ms(50.0);

    env.gate_on();

    // Reach sustain
    for _ in 0..5000 {
        env.advance();
    }

    env.gate_off();

    let mut prev = env.level();
    for _ in 0..5000 {
        let level = env.advance();
        if env.state() == EnvelopeState::Idle {
            break;
        }
        assert!(
            level <= prev + 1e-6,
            "release should be monotonically decreasing: prev={prev}, current={level}"
        );
        prev = level;
    }
}

#[test]
fn adsr_retrigger_preserves_level() {
    let mut env = AdsrEnvelope::new(SR);
    env.set_attack_ms(10.0);
    env.set_decay_ms(50.0);
    env.set_sustain(0.5);

    env.gate_on();

    // Advance partway through attack
    for _ in 0..200 {
        env.advance();
    }

    let level_before = env.level();
    assert!(level_before > 0.0, "should have some level during attack");

    // Retrigger
    env.gate_on();

    // Level should be preserved (smooth retrigger)
    let level_after = env.level();
    assert!(
        (level_after - level_before).abs() < 1e-6,
        "retrigger should preserve level: before={level_before}, after={level_after}"
    );
}

// ---------------------------------------------------------------------------
// 6. Integration: voice with oscillator + envelope combined behavior
// ---------------------------------------------------------------------------

#[test]
fn voice_output_scales_with_velocity() {
    let vel_low: u8 = 32;
    let vel_high: u8 = 127;
    let note: u8 = 69;
    let num_samples = 1024;

    let mut voice_low = Voice::new(SR);
    voice_low.note_on(note, vel_low);
    let energy_low: f64 = (0..num_samples)
        .map(|_| {
            let s = voice_low.process() as f64;
            s * s
        })
        .sum();

    let mut voice_high = Voice::new(SR);
    voice_high.note_on(note, vel_high);
    let energy_high: f64 = (0..num_samples)
        .map(|_| {
            let s = voice_high.process() as f64;
            s * s
        })
        .sum();

    assert!(
        energy_high > energy_low,
        "higher velocity should produce more energy: high={energy_high}, low={energy_low}"
    );
}

#[test]
fn voice_midi_to_freq_round_trip() {
    // Verify that note_on sets the oscillator to the correct MIDI frequency
    let note: u8 = 69; // A4 = 440 Hz
    let expected_freq = midi_to_freq(note);

    assert!(
        (expected_freq - 440.0).abs() < 0.01,
        "A4 should be 440 Hz, got {expected_freq}"
    );

    let mut voice = Voice::new(SR);
    voice.note_on(note, 100);

    // The oscillator frequency should match
    assert!(
        (voice.osc1.frequency() - expected_freq).abs() < 0.01,
        "osc1 freq should be {expected_freq}, got {}",
        voice.osc1.frequency()
    );
}
