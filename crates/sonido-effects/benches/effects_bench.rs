//! Criterion benchmarks for sonido effects
//!
//! Run with: cargo bench
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sonido_core::{Effect, EffectExt, Oversampled};
use sonido_effects::{
    Chorus, CleanPreamp, Compressor, Delay, Distortion, Flanger, Gate, LowPassFilter, MultiVibrato,
    ParametricEq, Phaser, Reverb, TapeSaturation, Tremolo, Wah,
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

fn bench_flanger(c: &mut Criterion) {
    let mut effect = Flanger::new(SAMPLE_RATE);
    effect.set_rate(0.5);
    effect.set_depth(0.7);
    effect.set_feedback(0.5);
    effect.set_mix(0.5);
    bench_effect(c, "Flanger", effect);
}

fn bench_phaser(c: &mut Criterion) {
    let mut effect = Phaser::new(SAMPLE_RATE);
    effect.set_rate(1.0);
    effect.set_depth(0.8);
    effect.set_stages(6);
    effect.set_feedback(0.3);
    effect.set_mix(0.5);
    bench_effect(c, "Phaser", effect);
}

fn bench_gate(c: &mut Criterion) {
    let mut effect = Gate::new(SAMPLE_RATE);
    effect.set_threshold_db(-40.0);
    effect.set_attack_ms(1.0);
    effect.set_release_ms(50.0);
    effect.set_hold_ms(10.0);
    bench_effect(c, "Gate", effect);
}

fn bench_tremolo(c: &mut Criterion) {
    let mut effect = Tremolo::new(SAMPLE_RATE);
    effect.set_rate(5.0);
    effect.set_depth(0.8);
    bench_effect(c, "Tremolo", effect);
}

fn bench_wah(c: &mut Criterion) {
    let mut effect = Wah::new(SAMPLE_RATE);
    effect.set_sensitivity(0.7);
    bench_effect(c, "Wah", effect);
}

fn bench_parametric_eq(c: &mut Criterion) {
    let mut effect = ParametricEq::new(SAMPLE_RATE);
    effect.set_low_freq(200.0);
    effect.set_low_gain(3.0);
    effect.set_low_q(1.0);
    effect.set_mid_freq(1000.0);
    effect.set_mid_gain(-2.0);
    effect.set_mid_q(1.5);
    effect.set_high_freq(4000.0);
    effect.set_high_gain(2.0);
    effect.set_high_q(1.0);
    bench_effect(c, "ParametricEq", effect);
}

// --- Stereo benchmarks ---

fn generate_stereo_test_signals(size: usize) -> (Vec<f32>, Vec<f32>) {
    let left: Vec<f32> = (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();
    let right: Vec<f32> = (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            (2.0 * std::f32::consts::PI * 440.0 * t + std::f32::consts::FRAC_PI_3).sin() * 0.5
        })
        .collect();
    (left, right)
}

fn bench_stereo_effect<E: Effect>(c: &mut Criterion, name: &str, mut effect: E) {
    let mut group = c.benchmark_group(name);

    for &block_size in BLOCK_SIZES {
        let (left_in, right_in) = generate_stereo_test_signals(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, &size| {
                let mut left_out = vec![0.0; size];
                let mut right_out = vec![0.0; size];
                b.iter(|| {
                    effect.process_block_stereo(
                        black_box(&left_in),
                        black_box(&right_in),
                        &mut left_out,
                        &mut right_out,
                    );
                    black_box((left_out[0], right_out[0]))
                })
            },
        );
    }

    group.finish();
}

fn bench_stereo_chorus(c: &mut Criterion) {
    let mut effect = Chorus::new(SAMPLE_RATE);
    effect.set_rate(2.0);
    effect.set_depth(0.7);
    effect.set_mix(0.5);
    bench_stereo_effect(c, "Chorus_Stereo", effect);
}

fn bench_stereo_reverb(c: &mut Criterion) {
    let mut effect = Reverb::new(SAMPLE_RATE);
    effect.set_room_size(0.7);
    effect.set_decay(0.8);
    effect.set_damping(0.3);
    effect.set_predelay_ms(15.0);
    effect.set_mix(0.5);
    bench_stereo_effect(c, "Reverb_Stereo", effect);
}

fn bench_stereo_phaser(c: &mut Criterion) {
    let mut effect = Phaser::new(SAMPLE_RATE);
    effect.set_rate(1.0);
    effect.set_depth(0.8);
    effect.set_stages(6);
    effect.set_feedback(0.3);
    effect.set_mix(0.5);
    bench_stereo_effect(c, "Phaser_Stereo", effect);
}

fn bench_stereo_flanger(c: &mut Criterion) {
    let mut effect = Flanger::new(SAMPLE_RATE);
    effect.set_rate(0.5);
    effect.set_depth(0.7);
    effect.set_feedback(0.5);
    effect.set_mix(0.5);
    bench_stereo_effect(c, "Flanger_Stereo", effect);
}

fn bench_stereo_delay(c: &mut Criterion) {
    let mut effect = Delay::new(SAMPLE_RATE);
    effect.set_delay_time_ms(375.0);
    effect.set_feedback(0.5);
    effect.set_mix(0.3);
    effect.set_ping_pong(true);
    bench_stereo_effect(c, "Delay_Stereo_PingPong", effect);
}

// --- Oversampling benchmarks ---

fn bench_oversampling(c: &mut Criterion) {
    // Inner effect created at base rate — Oversampled::new() handles the Nx rate internally
    let dist_2x = Oversampled::<2, Distortion>::new(Distortion::new(SAMPLE_RATE), SAMPLE_RATE);
    bench_effect(c, "Oversampled_2x_Distortion", dist_2x);

    let dist_4x = Oversampled::<4, Distortion>::new(Distortion::new(SAMPLE_RATE), SAMPLE_RATE);
    bench_effect(c, "Oversampled_4x_Distortion", dist_4x);

    let dist_8x = Oversampled::<8, Distortion>::new(Distortion::new(SAMPLE_RATE), SAMPLE_RATE);
    bench_effect(c, "Oversampled_8x_Distortion", dist_8x);
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
    bench_flanger,
    bench_phaser,
    bench_gate,
    bench_tremolo,
    bench_wah,
    bench_parametric_eq,
    bench_stereo_chorus,
    bench_stereo_reverb,
    bench_stereo_phaser,
    bench_stereo_flanger,
    bench_stereo_delay,
    bench_oversampling,
    bench_effect_chain,
);

criterion_main!(benches);
