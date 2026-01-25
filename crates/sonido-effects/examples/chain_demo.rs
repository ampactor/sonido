//! Demonstration of effect chaining in Sonido
//!
//! This example shows both static dispatch (zero-cost) and dynamic dispatch
//! (runtime flexibility) approaches to chaining effects.
//!
//! Run with: cargo run --example chain_demo

use sonido_core::{Effect, EffectExt};
use sonido_effects::{
    Distortion, WaveShape, Compressor, Chorus, Delay,
    LowPassFilter, TapeSaturation, CleanPreamp,
};

const SAMPLE_RATE: f32 = 48000.0;

fn main() {
    println!("Sonido Effect Chain Demo");
    println!("========================\n");

    // Generate a test signal (440 Hz sine wave)
    let test_signal: Vec<f32> = (0..4800)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();

    // Example 1: Static dispatch chain (zero-cost abstraction)
    println!("1. Static Dispatch Chain (Compile-time, Zero-Cost)");
    println!("-------------------------------------------------");

    let preamp = {
        let mut p = CleanPreamp::new();
        p.set_gain_db(6.0);
        p
    };

    let distortion = {
        let mut d = Distortion::new(SAMPLE_RATE);
        d.set_drive_db(12.0);
        d.set_waveshape(WaveShape::SoftClip);
        d.set_tone_hz(4000.0);
        d.set_level_db(-6.0);
        d
    };

    let tape = {
        let mut t = TapeSaturation::new(SAMPLE_RATE);
        t.set_drive(1.5);
        t.set_saturation(0.4);
        t
    };

    let chorus = {
        let mut c = Chorus::new(SAMPLE_RATE);
        c.set_rate(1.2);
        c.set_depth(0.5);
        c.set_mix(0.3);
        c
    };

    let delay = {
        let mut d = Delay::new(SAMPLE_RATE);
        d.set_delay_time_ms(375.0);
        d.set_feedback(0.4);
        d.set_mix(0.25);
        d
    };

    // Chain using the EffectExt trait - all resolved at compile time
    let mut static_chain = preamp
        .chain(distortion)
        .chain(tape)
        .chain(chorus)
        .chain(delay);

    let mut output = vec![0.0; test_signal.len()];
    static_chain.process_block(&test_signal, &mut output);

    // Calculate RMS of output
    let rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
    println!("Chain: Preamp -> Distortion -> Tape -> Chorus -> Delay");
    println!("Output RMS: {:.4}", rms);
    println!("Peak: {:.4}", output.iter().map(|x| x.abs()).fold(0.0_f32, f32::max));
    println!("Latency: {} samples\n", static_chain.latency_samples());

    // Example 2: Dynamic dispatch chain (runtime flexibility)
    println!("2. Dynamic Dispatch Chain (Runtime Flexibility)");
    println!("-----------------------------------------------");

    let mut dynamic_chain: Vec<Box<dyn Effect>> = vec![
        Box::new({
            let mut f = LowPassFilter::new(SAMPLE_RATE);
            f.set_cutoff_hz(2000.0);
            f.set_q(1.0);
            f
        }),
        Box::new({
            let mut c = Compressor::new(SAMPLE_RATE);
            c.set_threshold_db(-18.0);
            c.set_ratio(4.0);
            c.set_attack_ms(5.0);
            c.set_release_ms(100.0);
            c
        }),
        Box::new({
            let mut d = Delay::new(SAMPLE_RATE);
            d.set_delay_time_ms(250.0);
            d.set_feedback(0.3);
            d.set_mix(0.2);
            d
        }),
    ];

    // Process with dynamic dispatch
    let mut output2 = test_signal.clone();
    for sample in output2.iter_mut() {
        for effect in dynamic_chain.iter_mut() {
            *sample = effect.process(*sample);
        }
    }

    let rms2: f32 = (output2.iter().map(|x| x * x).sum::<f32>() / output2.len() as f32).sqrt();
    println!("Chain: LowPass -> Compressor -> Delay");
    println!("Output RMS: {:.4}", rms2);
    println!("Peak: {:.4}\n", output2.iter().map(|x| x.abs()).fold(0.0_f32, f32::max));

    // Example 3: Individual effect demonstration
    println!("3. Individual Effects");
    println!("--------------------");

    let effects_info = [
        ("CleanPreamp", "High-headroom, zero-latency preamp"),
        ("Distortion", "4 waveshapes: soft/hard clip, tanh, foldback"),
        ("TapeSaturation", "Asymmetric saturation with HF rolloff"),
        ("Compressor", "Soft-knee dynamics with envelope follower"),
        ("Chorus", "Dual-voice modulated delay"),
        ("Delay", "Feedback delay with smooth parameter changes"),
        ("LowPassFilter", "Biquad-based resonant filter"),
    ];

    for (name, desc) in effects_info {
        println!("  {} - {}", name, desc);
    }

    println!("\n4. Tape MultiVibrato (Oxide Original)");
    println!("-------------------------------------");
    println!("  10 simultaneous subtle vibratos at different frequencies");
    println!("  Simulates authentic tape wow and flutter");
    println!("  Latency: 128 samples (~2.7ms at 48kHz)");

    println!("\nDemo complete!");
}
