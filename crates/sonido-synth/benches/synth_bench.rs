//! Criterion benchmarks for sonido-synth components
//!
//! Run with: cargo bench -p sonido-synth

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sonido_synth::{
    AdsrEnvelope, MonophonicSynth, Oscillator, OscillatorWaveform, PolyphonicSynth,
    VoiceAllocationMode,
};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK_SIZES: &[usize] = &[64, 128, 256, 512, 1024];

// ============================================================================
// Oscillator benchmarks
// ============================================================================

fn bench_oscillator_waveforms(c: &mut Criterion) {
    let mut group = c.benchmark_group("Oscillator");

    let waveforms = [
        ("Sine", OscillatorWaveform::Sine),
        ("Saw", OscillatorWaveform::Saw),
        ("Square", OscillatorWaveform::Square),
        ("Triangle", OscillatorWaveform::Triangle),
        ("Pulse50", OscillatorWaveform::Pulse(0.5)),
        ("Pulse25", OscillatorWaveform::Pulse(0.25)),
        ("Noise", OscillatorWaveform::Noise),
    ];

    for (name, waveform) in &waveforms {
        for &block_size in BLOCK_SIZES {
            let mut osc = Oscillator::new(SAMPLE_RATE);
            osc.set_frequency(440.0);
            osc.set_waveform(*waveform);

            group.bench_with_input(
                BenchmarkId::new(*name, block_size),
                &block_size,
                |b, &size| {
                    b.iter(|| {
                        let mut sum = 0.0f32;
                        for _ in 0..size {
                            sum += osc.advance();
                        }
                        black_box(sum)
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_oscillator_phase_modulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Oscillator_PM");

    for &block_size in BLOCK_SIZES {
        let mut carrier = Oscillator::new(SAMPLE_RATE);
        carrier.set_frequency(440.0);
        carrier.set_waveform(OscillatorWaveform::Sine);

        let mut modulator = Oscillator::new(SAMPLE_RATE);
        modulator.set_frequency(220.0);
        modulator.set_waveform(OscillatorWaveform::Sine);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        let mod_val = modulator.advance();
                        sum += carrier.advance_with_pm(mod_val * 2.0);
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Envelope benchmarks
// ============================================================================

fn bench_envelope_adsr(c: &mut Criterion) {
    let mut group = c.benchmark_group("AdsrEnvelope");

    for &block_size in BLOCK_SIZES {
        let mut env = AdsrEnvelope::new(SAMPLE_RATE);
        env.set_attack_ms(10.0);
        env.set_decay_ms(50.0);
        env.set_sustain(0.7);
        env.set_release_ms(200.0);
        env.gate_on();

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += env.advance();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_envelope_full_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("AdsrEnvelope_FullCycle");

    // Benchmark a complete attack-decay-sustain-release cycle
    group.bench_function("1sec_cycle", |b| {
        let mut env = AdsrEnvelope::new(SAMPLE_RATE);
        env.set_attack_ms(50.0);
        env.set_decay_ms(100.0);
        env.set_sustain(0.6);
        env.set_release_ms(300.0);

        b.iter(|| {
            env.reset();
            env.gate_on();

            let mut sum = 0.0f32;
            // Attack + decay + sustain
            for _ in 0..24000 {
                sum += env.advance();
            }
            // Release
            env.gate_off();
            for _ in 0..24000 {
                sum += env.advance();
            }
            black_box(sum)
        })
    });

    group.finish();
}

// ============================================================================
// Monophonic synth benchmarks
// ============================================================================

fn bench_monophonic_synth(c: &mut Criterion) {
    let mut group = c.benchmark_group("MonophonicSynth");

    for &block_size in BLOCK_SIZES {
        let mut synth = MonophonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_detune(7.0);
        synth.set_filter_cutoff(2000.0);
        synth.set_filter_resonance(2.0);
        synth.note_on(60, 100);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_monophonic_synth_modulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("MonophonicSynth_Modulation");

    for &block_size in BLOCK_SIZES {
        let mut synth = MonophonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_waveform(OscillatorWaveform::Square);
        synth.set_osc2_detune(5.0);
        synth.set_osc_mix(0.5);
        synth.set_filter_cutoff(1500.0);
        synth.set_filter_resonance(3.0);
        synth.set_filter_env_amount(2000.0);
        synth.set_lfo1_rate(5.0);
        synth.set_lfo1_to_pitch(0.5);
        synth.set_lfo1_to_filter(500.0);
        synth.note_on(60, 100);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_monophonic_synth_glide(c: &mut Criterion) {
    let mut group = c.benchmark_group("MonophonicSynth_Glide");

    let block_size = 256;
    let mut synth = MonophonicSynth::new(SAMPLE_RATE);
    synth.set_osc1_waveform(OscillatorWaveform::Saw);
    synth.set_glide_time(100.0);
    synth.note_on(48, 100);

    // Process some samples to establish the first note
    for _ in 0..1000 {
        synth.process();
    }

    group.bench_function("glide_transition", |b| {
        b.iter(|| {
            synth.note_on(60, 100);
            let mut sum = 0.0f32;
            for _ in 0..block_size {
                sum += synth.process();
            }
            synth.note_on(48, 100);
            for _ in 0..block_size {
                sum += synth.process();
            }
            black_box(sum)
        })
    });

    group.finish();
}

// ============================================================================
// Polyphonic synth benchmarks
// ============================================================================

fn bench_polyphonic_synth_4_voices(c: &mut Criterion) {
    let mut group = c.benchmark_group("PolyphonicSynth_4Voice");

    for &block_size in BLOCK_SIZES {
        let mut synth: PolyphonicSynth<4> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_filter_cutoff(2000.0);

        // Play a chord (all 4 voices active)
        synth.note_on(60, 100); // C
        synth.note_on(64, 100); // E
        synth.note_on(67, 100); // G
        synth.note_on(72, 100); // C octave

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_polyphonic_synth_8_voices(c: &mut Criterion) {
    let mut group = c.benchmark_group("PolyphonicSynth_8Voice");

    for &block_size in BLOCK_SIZES {
        let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_detune(7.0);
        synth.set_filter_cutoff(2000.0);
        synth.set_filter_resonance(2.0);

        // Play all 8 voices
        for note in [48, 52, 55, 60, 64, 67, 72, 76] {
            synth.note_on(note, 100);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_polyphonic_synth_16_voices(c: &mut Criterion) {
    let mut group = c.benchmark_group("PolyphonicSynth_16Voice");

    for &block_size in BLOCK_SIZES {
        let mut synth: PolyphonicSynth<16> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_osc2_waveform(OscillatorWaveform::Square);
        synth.set_osc2_detune(5.0);
        synth.set_osc_mix(0.3);
        synth.set_filter_cutoff(3000.0);
        synth.set_filter_resonance(1.5);

        // Play all 16 voices
        for i in 0..16 {
            synth.note_on(36 + i * 3, 100);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

fn bench_polyphonic_synth_voice_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("PolyphonicSynth_VoiceAllocation");

    // Test voice stealing performance
    group.bench_function("voice_stealing_8voice", |b| {
        let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_allocation_mode(VoiceAllocationMode::OldestNote);

        b.iter(|| {
            // Play more notes than voices, forcing stealing
            for i in 0..12 {
                synth.note_on(48 + i * 2, 100);
                for _ in 0..64 {
                    black_box(synth.process());
                }
            }
            synth.all_notes_off();
        })
    });

    group.finish();
}

fn bench_polyphonic_synth_with_lfo(c: &mut Criterion) {
    let mut group = c.benchmark_group("PolyphonicSynth_LFO");

    for &block_size in BLOCK_SIZES {
        let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.set_filter_cutoff(2000.0);
        synth.set_lfo1_rate(5.0);
        synth.set_lfo1_to_pitch(0.5);
        synth.set_lfo1_to_filter(500.0);

        // Play 4 voices
        synth.note_on(60, 100);
        synth.note_on(64, 100);
        synth.note_on(67, 100);
        synth.note_on(72, 100);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for _ in 0..size {
                        sum += synth.process();
                    }
                    black_box(sum)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Voice scaling benchmark
// ============================================================================

fn bench_voice_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("VoiceScaling");
    let block_size = 256;

    // 1 voice
    {
        let mut synth: PolyphonicSynth<1> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.note_on(60, 100);

        group.bench_function("1_voice", |b| {
            b.iter(|| {
                let mut sum = 0.0f32;
                for _ in 0..block_size {
                    sum += synth.process();
                }
                black_box(sum)
            })
        });
    }

    // 2 voices
    {
        let mut synth: PolyphonicSynth<2> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        synth.note_on(60, 100);
        synth.note_on(64, 100);

        group.bench_function("2_voices", |b| {
            b.iter(|| {
                let mut sum = 0.0f32;
                for _ in 0..block_size {
                    sum += synth.process();
                }
                black_box(sum)
            })
        });
    }

    // 4 voices
    {
        let mut synth: PolyphonicSynth<4> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        for note in [60, 64, 67, 72] {
            synth.note_on(note, 100);
        }

        group.bench_function("4_voices", |b| {
            b.iter(|| {
                let mut sum = 0.0f32;
                for _ in 0..block_size {
                    sum += synth.process();
                }
                black_box(sum)
            })
        });
    }

    // 8 voices
    {
        let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(SAMPLE_RATE);
        synth.set_osc1_waveform(OscillatorWaveform::Saw);
        for note in [48, 52, 55, 60, 64, 67, 72, 76] {
            synth.note_on(note, 100);
        }

        group.bench_function("8_voices", |b| {
            b.iter(|| {
                let mut sum = 0.0f32;
                for _ in 0..block_size {
                    sum += synth.process();
                }
                black_box(sum)
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_oscillator_waveforms,
    bench_oscillator_phase_modulation,
    bench_envelope_adsr,
    bench_envelope_full_cycle,
    bench_monophonic_synth,
    bench_monophonic_synth_modulation,
    bench_monophonic_synth_glide,
    bench_polyphonic_synth_4_voices,
    bench_polyphonic_synth_8_voices,
    bench_polyphonic_synth_16_voices,
    bench_polyphonic_synth_voice_allocation,
    bench_polyphonic_synth_with_lfo,
    bench_voice_scaling,
);

criterion_main!(benches);
