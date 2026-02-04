//! Synthesis demo: oscillators, envelopes, and FM synthesis.
//!
//! Run with: cargo run -p sonido-synth --example synthesis_demo

use sonido_synth::{AdsrEnvelope, Oscillator, OscillatorWaveform};

fn main() {
    let sample_rate = 48000.0;

    // --- Oscillator waveforms ---
    println!("=== Oscillator Waveforms (440 Hz, first 20 samples) ===\n");

    let waveforms = [
        ("Sine", OscillatorWaveform::Sine),
        ("Saw", OscillatorWaveform::Saw),
        ("Square", OscillatorWaveform::Square),
        ("Triangle", OscillatorWaveform::Triangle),
    ];

    for (name, wf) in &waveforms {
        let mut osc = Oscillator::new(sample_rate);
        osc.set_frequency(440.0);
        osc.set_waveform(*wf);

        let samples: Vec<f32> = (0..20).map(|_| osc.advance()).collect();
        println!(
            "{:<10} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} ...",
            name, samples[0], samples[1], samples[2], samples[3], samples[4]
        );
    }

    // --- ADSR envelope shaping a note ---
    println!("\n=== ADSR Envelope Shaping a 440 Hz Sine ===\n");

    let mut osc = Oscillator::new(sample_rate);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Sine);

    let mut env = AdsrEnvelope::new(sample_rate);
    env.set_attack_ms(10.0);
    env.set_decay_ms(50.0);
    env.set_sustain(0.6);
    env.set_release_ms(100.0);

    // Gate on: play note for ~20ms then release
    env.gate_on();
    let note_on_samples = (20.0 * sample_rate / 1000.0) as usize; // 20ms
    let release_samples = (150.0 * sample_rate / 1000.0) as usize; // 150ms for release tail

    println!("Phase      | Sample# | Env Level | Osc Out | Shaped Out");
    println!("-----------+---------+-----------+---------+-----------");

    // Print at key moments during attack+sustain
    for i in 0..note_on_samples {
        let level = env.advance();
        let raw = osc.advance();
        let shaped = raw * level;

        // Print every 100 samples and at transitions
        if i % 100 == 0 {
            println!(
                "{:<10} | {:>7} | {:>9.4} | {:>7.4} | {:>10.4}",
                format!("{:?}", env.state()),
                i,
                level,
                raw,
                shaped
            );
        }
    }

    // Gate off: release phase
    env.gate_off();
    println!("--- gate off ---");

    for i in 0..release_samples {
        let level = env.advance();
        let raw = osc.advance();
        let shaped = raw * level;

        if i % 500 == 0 {
            println!(
                "{:<10} | {:>7} | {:>9.4} | {:>7.4} | {:>10.4}",
                format!("{:?}", env.state()),
                note_on_samples + i,
                level,
                raw,
                shaped
            );
        }
    }

    println!("Final state: {:?}, level: {:.6}", env.state(), env.level());

    // --- FM Synthesis ---
    println!("\n=== FM Synthesis (carrier 440 Hz, modulator 880 Hz) ===\n");

    let mut carrier = Oscillator::new(sample_rate);
    carrier.set_frequency(440.0);
    carrier.set_waveform(OscillatorWaveform::Sine);

    let mut modulator = Oscillator::new(sample_rate);
    modulator.set_frequency(880.0); // 2:1 ratio
    modulator.set_waveform(OscillatorWaveform::Sine);

    let mod_index = 2.0; // modulation depth in radians

    println!("Sample | Mod Out | PM (rad) | Carrier Out");
    println!("-------+---------+----------+------------");

    for i in 0..20 {
        let mod_out = modulator.advance();
        let pm = mod_out * mod_index; // phase modulation in radians
        let carrier_out = carrier.advance_with_pm(pm);

        if i % 2 == 0 {
            println!(
                "{:>6} | {:>7.4} | {:>8.4} | {:>11.4}",
                i, mod_out, pm, carrier_out
            );
        }
    }

    // Compare FM vs clean sine
    println!("\n=== FM vs Clean Sine (100 samples, showing difference) ===\n");

    let mut clean = Oscillator::new(sample_rate);
    clean.set_frequency(440.0);
    clean.set_waveform(OscillatorWaveform::Sine);

    let mut fm_carrier = Oscillator::new(sample_rate);
    fm_carrier.set_frequency(440.0);
    fm_carrier.set_waveform(OscillatorWaveform::Sine);

    let mut fm_mod = Oscillator::new(sample_rate);
    fm_mod.set_frequency(880.0);
    fm_mod.set_waveform(OscillatorWaveform::Sine);

    let mut max_diff: f32 = 0.0;
    for _ in 0..100 {
        let clean_out = clean.advance();
        let m = fm_mod.advance();
        let fm_out = fm_carrier.advance_with_pm(m * 3.0);
        max_diff = max_diff.max((clean_out - fm_out).abs());
    }

    println!(
        "Max difference between clean sine and FM sine over 100 samples: {:.4}",
        max_diff
    );
    println!("(FM synthesis adds sidebands that make the waveform more complex)");

    // --- Pulse width modulation ---
    println!("\n=== Pulse Width Modulation ===\n");

    for duty in [0.1, 0.25, 0.5, 0.75, 0.9] {
        let mut osc = Oscillator::new(sample_rate);
        osc.set_frequency(440.0);
        osc.set_waveform(OscillatorWaveform::Pulse(duty));

        let samples: Vec<f32> = (0..1000).map(|_| osc.advance()).collect();
        let rms: f32 = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        println!("Duty {:.0}%: RMS = {:.4}", duty * 100.0, rms);
    }

    println!("\nSynthesis demo complete.");
}
