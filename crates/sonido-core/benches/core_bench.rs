//! Criterion benchmarks for sonido-core DSP primitives
//!
//! Run with: cargo bench -p sonido-core
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sonido_core::{
    AllpassFilter, Biquad, CombFilter, DcBlocker, Effect, EnvelopeFollower, InterpolatedDelay, Lfo,
    LfoWaveform, OnePole, SmoothedParam, StateVariableFilter, lowpass_coefficients,
};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK_SIZES: &[usize] = &[64, 128, 256, 512, 1024];

fn generate_test_signal(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect()
}

fn bench_biquad(c: &mut Criterion) {
    let mut group = c.benchmark_group("Biquad");

    let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(1000.0, 0.707, SAMPLE_RATE);

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::new("process", block_size),
            &block_size,
            |b, _| {
                let mut biquad = Biquad::new();
                biquad.set_coefficients(b0, b1, b2, a0, a1, a2);
                b.iter(|| {
                    for &sample in &input {
                        black_box(biquad.process(black_box(sample)));
                    }
                });
            },
        );
    }

    // Coefficient calculation cost
    group.bench_function("coefficient_calc", |b| {
        b.iter(|| {
            black_box(lowpass_coefficients(
                black_box(1000.0),
                black_box(0.707),
                black_box(SAMPLE_RATE),
            ))
        });
    });

    group.finish();
}

fn bench_svf(c: &mut Criterion) {
    let mut group = c.benchmark_group("SVF");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::new("process", block_size),
            &block_size,
            |b, _| {
                let mut svf = StateVariableFilter::new(SAMPLE_RATE);
                svf.set_cutoff(1000.0);
                svf.set_resonance(1.0);
                b.iter(|| {
                    for &sample in &input {
                        black_box(svf.process(black_box(sample)));
                    }
                });
            },
        );
    }

    // set_cutoff recalculation cost
    group.bench_function("set_cutoff_recalc", |b| {
        let mut svf = StateVariableFilter::new(SAMPLE_RATE);
        b.iter(|| {
            svf.set_cutoff(black_box(1000.0));
        });
    });

    group.finish();
}

fn bench_comb(c: &mut Criterion) {
    let mut group = c.benchmark_group("CombFilter");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut comb = CombFilter::new(1557);
                comb.set_feedback(0.84);
                comb.set_damp(0.2);
                b.iter(|| {
                    for &sample in &input {
                        black_box(comb.process(black_box(sample)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_allpass(c: &mut Criterion) {
    let mut group = c.benchmark_group("AllpassFilter");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut allpass = AllpassFilter::new(556);
                allpass.set_feedback(0.5);
                b.iter(|| {
                    for &sample in &input {
                        black_box(allpass.process(black_box(sample)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_delay(c: &mut Criterion) {
    let mut group = c.benchmark_group("InterpolatedDelay");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut delay = InterpolatedDelay::new(48000);
                b.iter(|| {
                    for &sample in &input {
                        let out = delay.read(black_box(1000.5));
                        delay.write(black_box(sample));
                        black_box(out);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_lfo(c: &mut Criterion) {
    let mut group = c.benchmark_group("LFO");

    let waveforms = [
        ("Sine", LfoWaveform::Sine),
        ("Triangle", LfoWaveform::Triangle),
        ("Saw", LfoWaveform::Saw),
        ("Square", LfoWaveform::Square),
        ("SampleAndHold", LfoWaveform::SampleAndHold),
    ];

    for (name, waveform) in &waveforms {
        for &block_size in BLOCK_SIZES {
            group.bench_with_input(
                BenchmarkId::new(*name, block_size),
                &block_size,
                |b, &size| {
                    let mut lfo = Lfo::new(SAMPLE_RATE, 2.0);
                    lfo.set_waveform(*waveform);
                    b.iter(|| {
                        for _ in 0..size {
                            black_box(lfo.advance());
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_smoothed_param(c: &mut Criterion) {
    let mut group = c.benchmark_group("SmoothedParam");

    for &block_size in BLOCK_SIZES {
        // Ramping: set a new target each block
        group.bench_with_input(
            BenchmarkId::new("ramping", block_size),
            &block_size,
            |b, &size| {
                let mut param = SmoothedParam::standard(1.0, SAMPLE_RATE);
                b.iter(|| {
                    param.set_target(black_box(0.5));
                    for _ in 0..size {
                        black_box(param.advance());
                    }
                });
            },
        );

        // Settled: already at target
        group.bench_with_input(
            BenchmarkId::new("settled", block_size),
            &block_size,
            |b, &size| {
                let mut param = SmoothedParam::standard(1.0, SAMPLE_RATE);
                // Advance enough to settle
                for _ in 0..48000 {
                    param.advance();
                }
                b.iter(|| {
                    for _ in 0..size {
                        black_box(param.advance());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_one_pole(c: &mut Criterion) {
    let mut group = c.benchmark_group("OnePole");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut filter = OnePole::new(SAMPLE_RATE, 1000.0);
                b.iter(|| {
                    for &sample in &input {
                        black_box(filter.process(black_box(sample)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_dc_blocker(c: &mut Criterion) {
    let mut group = c.benchmark_group("DcBlocker");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut blocker = DcBlocker::new(SAMPLE_RATE);
                b.iter(|| {
                    for &sample in &input {
                        black_box(blocker.process(black_box(sample)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_envelope_follower(c: &mut Criterion) {
    let mut group = c.benchmark_group("EnvelopeFollower");

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut follower = EnvelopeFollower::new(SAMPLE_RATE);
                b.iter(|| {
                    for &sample in &input {
                        black_box(follower.process(black_box(sample)));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_biquad,
    bench_svf,
    bench_comb,
    bench_allpass,
    bench_delay,
    bench_lfo,
    bench_smoothed_param,
    bench_one_pole,
    bench_dc_blocker,
    bench_envelope_follower,
);

criterion_main!(benches);
