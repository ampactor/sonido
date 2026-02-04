//! Modulation demo: mod matrix, LFO, and modulation routing.
//!
//! Run with: cargo run -p sonido-synth --example modulation_demo

use sonido_synth::{
    Lfo, LfoWaveform, ModulationMatrix, ModulationRoute, ModulationSource, ModulationValues,
    ModDestination, ModSourceId, Oscillator, OscillatorWaveform, AdsrEnvelope,
};

fn main() {
    let sample_rate = 48000.0;

    // --- LFO as ModulationSource ---
    println!("=== LFO Modulation Source ===\n");

    let mut lfo = Lfo::new(sample_rate, 2.0); // 2 Hz LFO
    lfo.set_waveform(LfoWaveform::Sine);

    println!("LFO: 2 Hz sine, bipolar={}", lfo.is_bipolar());
    println!("\nSample | Bipolar | Unipolar");
    println!("-------+---------+---------");

    for i in 0..20 {
        let bipolar = lfo.mod_advance();
        // Use a separate LFO for unipolar to keep them in sync for display
        if i < 10 {
            println!("{:>6} | {:>7.4} |", i, bipolar);
        }
    }

    // Show unipolar conversion
    let mut lfo_uni = Lfo::new(sample_rate, 2.0);
    lfo_uni.set_waveform(LfoWaveform::Sine);
    println!("\nUnipolar conversion (first 10 samples):");
    for i in 0..10 {
        let uni = lfo_uni.mod_advance_unipolar();
        println!("  Sample {:>2}: {:.4}", i, uni);
    }

    // --- Different LFO waveforms ---
    println!("\n=== LFO Waveforms at 1 Hz (sampled at phase 0, 0.25, 0.5, 0.75) ===\n");

    let waveforms = [
        ("Sine", LfoWaveform::Sine),
        ("Triangle", LfoWaveform::Triangle),
        ("Saw", LfoWaveform::Saw),
        ("Square", LfoWaveform::Square),
    ];

    for (name, wf) in &waveforms {
        let mut l = Lfo::new(sample_rate, 1.0);
        l.set_waveform(*wf);

        // Sample at key phase points
        let quarter = (sample_rate / 4.0) as usize;
        let mut vals = Vec::new();
        for i in 0..(quarter * 4) {
            let v = l.advance();
            if i == 0 || i == quarter || i == quarter * 2 || i == quarter * 3 {
                vals.push(v);
            }
        }
        println!(
            "{:<10} 0deg={:>7.3}  90deg={:>7.3}  180deg={:>7.3}  270deg={:>7.3}",
            name, vals[0], vals[1], vals[2], vals[3]
        );
    }

    // --- Modulation Matrix setup ---
    println!("\n=== Modulation Matrix ===\n");

    let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

    // Route LFO1 -> filter cutoff (vibrato-like)
    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::FilterCutoff,
        0.5,
    ));

    // Route filter envelope -> filter cutoff (envelope sweep)
    matrix.add_route(ModulationRoute::unipolar(
        ModSourceId::FilterEnv,
        ModDestination::FilterCutoff,
        0.8,
    ));

    // Route LFO2 -> oscillator pitch (vibrato)
    matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo2,
        ModDestination::Osc1Pitch,
        0.1,
    ));

    // Route velocity -> amplitude
    matrix.add_route(ModulationRoute::unipolar(
        ModSourceId::Velocity,
        ModDestination::Amplitude,
        1.0,
    ));

    println!(
        "Matrix: {} routes (capacity {})",
        matrix.route_count(),
        matrix.capacity()
    );

    for i in 0..matrix.route_count() {
        if let Some(route) = matrix.get_route(i) {
            println!(
                "  Route {}: {:?} -> {:?}, amount={:.2}, bipolar={}",
                i, route.source, route.destination, route.amount, route.bipolar
            );
        }
    }

    // --- Compute modulation values per sample ---
    println!("\n=== Modulation Matrix Processing ===\n");

    let mut lfo1 = Lfo::new(sample_rate, 3.0); // 3 Hz for filter
    lfo1.set_waveform(LfoWaveform::Sine);

    let mut lfo2 = Lfo::new(sample_rate, 5.0); // 5 Hz vibrato
    lfo2.set_waveform(LfoWaveform::Triangle);

    let mut filter_env = AdsrEnvelope::new(sample_rate);
    filter_env.set_attack_ms(50.0);
    filter_env.set_decay_ms(200.0);
    filter_env.set_sustain(0.3);
    filter_env.set_release_ms(300.0);
    filter_env.gate_on();

    let base_cutoff = 1000.0; // Hz
    let mod_range = 2000.0; // Hz modulation range

    println!("Base cutoff: {} Hz, mod range: +/- {} Hz", base_cutoff, mod_range);
    println!("\nSample | LFO1   | FilterEnv | Cutoff Mod | Effective Cutoff");
    println!("-------+--------+-----------+------------+-----------------");

    for i in 0..20 {
        let mut values = ModulationValues::new();
        values.lfo1 = lfo1.advance();
        values.lfo2 = lfo2.advance();
        values.filter_env = filter_env.advance();
        values.velocity = 0.8;

        // Get total modulation for filter cutoff
        let cutoff_mod = matrix.get_modulation(ModDestination::FilterCutoff, &values);
        let effective_cutoff = (base_cutoff + cutoff_mod * mod_range).clamp(20.0, 20000.0);

        if i % 2 == 0 {
            println!(
                "{:>6} | {:>6.3} | {:>9.4} | {:>10.4} | {:>15.1} Hz",
                i, values.lfo1, values.filter_env, cutoff_mod, effective_cutoff
            );
        }
    }

    // --- LFO modulating oscillator pitch ---
    println!("\n=== LFO Vibrato on Oscillator ===\n");

    let mut osc = Oscillator::new(sample_rate);
    osc.set_frequency(440.0);
    osc.set_waveform(OscillatorWaveform::Sine);

    let mut vibrato_lfo = Lfo::new(sample_rate, 5.0); // 5 Hz vibrato
    vibrato_lfo.set_waveform(LfoWaveform::Sine);

    let vibrato_depth_hz = 5.0; // +/- 5 Hz vibrato

    println!("Carrier: 440 Hz sine, Vibrato: 5 Hz sine, depth: +/- {} Hz", vibrato_depth_hz);
    println!("\nSample | LFO Val | Freq (Hz) | Output");
    println!("-------+---------+-----------+-------");

    for i in 0..20 {
        let lfo_val = vibrato_lfo.mod_advance();
        let modulated_freq = 440.0 + lfo_val * vibrato_depth_hz;
        osc.set_frequency(modulated_freq);
        let out = osc.advance();

        if i % 2 == 0 {
            println!(
                "{:>6} | {:>7.4} | {:>9.2} | {:>6.4}",
                i, lfo_val, modulated_freq, out
            );
        }
    }

    // --- Route management ---
    println!("\n=== Route Management ===\n");

    let mut mgmt_matrix: ModulationMatrix<4> = ModulationMatrix::new();
    mgmt_matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo1,
        ModDestination::Osc1Pitch,
        0.3,
    ));
    mgmt_matrix.add_route(ModulationRoute::new(
        ModSourceId::Lfo2,
        ModDestination::FilterCutoff,
        0.7,
    ));
    mgmt_matrix.add_route(ModulationRoute::new(
        ModSourceId::ModWheel,
        ModDestination::Amplitude,
        0.5,
    ));

    println!("Initial routes: {}", mgmt_matrix.route_count());

    // Remove middle route
    let removed = mgmt_matrix.remove_route(1);
    if let Some(r) = removed {
        println!("Removed route: {:?} -> {:?}", r.source, r.destination);
    }
    println!("Routes after removal: {}", mgmt_matrix.route_count());

    // Modify amount on remaining route
    if let Some(route) = mgmt_matrix.get_route_mut(0) {
        route.amount = 0.9;
        println!("Updated route 0 amount to {:.1}", route.amount);
    }

    // Clear all
    mgmt_matrix.clear();
    println!("After clear: {} routes", mgmt_matrix.route_count());

    println!("\nModulation demo complete.");
}
