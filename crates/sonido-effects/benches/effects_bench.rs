//! Criterion benchmarks for sonido effects
//!
//! Run with: cargo bench
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sonido_core::{Effect, EffectExt};
use sonido_effects::{
    Chorus, CleanPreamp, Compressor, Delay, Distortion, LowPassFilter, MultiVibrato, Reverb,
    TapeSaturation,
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

fn bench_effect<E: Effect>(c: &mut Criterion, name: &str, mut effect: E) {
    let mut group = c.benchmark_group(name);

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut output = vec![0.0; block_size];
                b.iter(|| {
                    effect.process_block(black_box(&input), &mut output);
                    black_box(output[0])
                })
            },
        );
    }

    group.finish();
}

fn bench_distortion(c: &mut Criterion) {
    let mut effect = Distortion::new(SAMPLE_RATE);
    effect.set_drive_db(20.0);
    effect.set_tone_hz(4000.0);
    effect.set_level_db(-6.0);
    bench_effect(c, "Distortion", effect);
}

fn bench_compressor(c: &mut Criterion) {
    let mut effect = Compressor::new(SAMPLE_RATE);
    effect.set_threshold_db(-20.0);
    effect.set_ratio(4.0);
    effect.set_attack_ms(5.0);
    effect.set_release_ms(50.0);
    bench_effect(c, "Compressor", effect);
}

fn bench_chorus(c: &mut Criterion) {
    let mut effect = Chorus::new(SAMPLE_RATE);
    effect.set_rate(2.0);
    effect.set_depth(0.7);
    effect.set_mix(0.5);
    bench_effect(c, "Chorus", effect);
}

fn bench_delay(c: &mut Criterion) {
    let mut effect = Delay::new(SAMPLE_RATE);
    effect.set_delay_time_ms(375.0);
    effect.set_feedback(0.5);
    effect.set_mix(0.3);
    bench_effect(c, "Delay", effect);
}

fn bench_lowpass(c: &mut Criterion) {
    let mut effect = LowPassFilter::new(SAMPLE_RATE);
    effect.set_cutoff_hz(1000.0);
    effect.set_q(0.707);
    bench_effect(c, "LowPassFilter", effect);
}

fn bench_multi_vibrato(c: &mut Criterion) {
    let mut effect = MultiVibrato::new(SAMPLE_RATE);
    effect.set_mix(1.0);
    effect.set_depth(1.0);
    bench_effect(c, "MultiVibrato", effect);
}

fn bench_tape_saturation(c: &mut Criterion) {
    let mut effect = TapeSaturation::new(SAMPLE_RATE);
    effect.set_drive(2.0);
    effect.set_saturation(0.6);
    bench_effect(c, "TapeSaturation", effect);
}

fn bench_clean_preamp(c: &mut Criterion) {
    let mut effect = CleanPreamp::new(SAMPLE_RATE);
    effect.set_gain_db(12.0);
    effect.set_output_db(-6.0);
    bench_effect(c, "CleanPreamp", effect);
}

fn bench_reverb(c: &mut Criterion) {
    let mut effect = Reverb::new(SAMPLE_RATE);
    effect.set_room_size(0.7);
    effect.set_decay(0.8);
    effect.set_damping(0.3);
    effect.set_predelay_ms(15.0);
    effect.set_mix(0.5);
    bench_effect(c, "Reverb", effect);
}

fn bench_effect_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("EffectChain");

    // Typical guitar chain: preamp -> distortion -> chorus -> delay
    let preamp = {
        let mut p = CleanPreamp::new(SAMPLE_RATE);
        p.set_gain_db(6.0);
        p
    };
    let distortion = {
        let mut d = Distortion::new(SAMPLE_RATE);
        d.set_drive_db(12.0);
        d.set_level_db(-6.0);
        d
    };
    let chorus = {
        let mut c = Chorus::new(SAMPLE_RATE);
        c.set_rate(1.5);
        c.set_depth(0.5);
        c.set_mix(0.3);
        c
    };
    let delay = {
        let mut d = Delay::new(SAMPLE_RATE);
        d.set_delay_time_ms(300.0);
        d.set_feedback(0.4);
        d.set_mix(0.25);
        d
    };

    let mut chain = preamp.chain(distortion).chain(chorus).chain(delay);

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut output = vec![0.0; block_size];
                b.iter(|| {
                    chain.process_block(black_box(&input), &mut output);
                    black_box(output[0])
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_distortion,
    bench_compressor,
    bench_chorus,
    bench_delay,
    bench_lowpass,
    bench_multi_vibrato,
    bench_tape_saturation,
    bench_clean_preamp,
    bench_reverb,
    bench_effect_chain,
);

criterion_main!(benches);
