//! Tempo sync demo: TempoManager, note divisions, and tempo-synced timing.
//!
//! Run with: cargo run -p sonido-core --example tempo_sync_demo

use sonido_core::{Lfo, LfoWaveform, NoteDivision, TempoManager};

fn main() {
    let sample_rate = 48000.0;

    // --- TempoManager basics ---
    println!("=== TempoManager at 120 BPM ===\n");

    let mut tempo = TempoManager::new(sample_rate, 120.0);
    println!("BPM: {}", tempo.bpm());
    println!("Transport: {:?}", tempo.transport());

    // --- Delay times for all note divisions ---
    println!("\n--- Delay Times at 120 BPM ---\n");
    println!(
        "{:<22} {:>8} {:>10} {:>12}",
        "Division", "Beats", "ms", "Samples"
    );
    println!("{:-<22} {:->8} {:->10} {:->12}", "", "", "", "");

    let divisions = [
        ("Whole", NoteDivision::Whole),
        ("Half", NoteDivision::Half),
        ("Dotted Half", NoteDivision::DottedHalf),
        ("Quarter", NoteDivision::Quarter),
        ("Dotted Quarter", NoteDivision::DottedQuarter),
        ("Eighth", NoteDivision::Eighth),
        ("Dotted Eighth", NoteDivision::DottedEighth),
        ("Triplet Quarter", NoteDivision::TripletQuarter),
        ("Sixteenth", NoteDivision::Sixteenth),
        ("Triplet Eighth", NoteDivision::TripletEighth),
        ("Thirty-Second", NoteDivision::ThirtySecond),
        ("Triplet Sixteenth", NoteDivision::TripletSixteenth),
    ];

    for (name, div) in &divisions {
        println!(
            "{:<22} {:>8.3} {:>10.1} {:>12.0}",
            name,
            div.beats(),
            tempo.division_to_ms(*div),
            tempo.division_to_samples(*div)
        );
    }

    // --- LFO rates for note divisions ---
    println!("\n--- LFO Rates at 120 BPM ---\n");
    println!("{:<22} {:>10}", "Division", "LFO Hz");
    println!("{:-<22} {:->10}", "", "");

    for (name, div) in &divisions {
        println!("{:<22} {:>10.3}", name, tempo.division_to_hz(*div));
    }

    // --- Transport and beat tracking ---
    println!("\n=== Transport and Beat Position ===\n");

    tempo.play();
    println!("Transport started: {:?}", tempo.transport());

    // Advance through 2 bars (8 beats at 120 BPM)
    let samples_per_beat = (sample_rate * 60.0 / 120.0) as usize; // 24000

    println!("\nAdvancing through 2 bars (8 beats):\n");
    println!(
        "{:>8} {:>8} {:>8} {:>8} {:>8}",
        "Beat", "Phase", "Bar", "BarPh", "OnBeat"
    );
    println!("{:->8} {:->8} {:->8} {:->8} {:->8}", "", "", "", "", "");

    for _beat in 0..8 {
        // Advance to beat boundary
        for _ in 0..samples_per_beat {
            tempo.advance();
        }
        println!(
            "{:>8.2} {:>8.3} {:>8.2} {:>8.3} {:>8}",
            tempo.beat_position(),
            tempo.beat_phase(),
            tempo.bar_position(),
            tempo.bar_phase(),
            if tempo.is_on_beat(10) { "YES" } else { "no" }
        );
    }

    // --- BPM changes ---
    println!("\n=== BPM Changes ===\n");

    let bpms = [60.0, 90.0, 120.0, 140.0, 180.0];
    println!(
        "{:>6} {:>12} {:>12} {:>12}",
        "BPM", "Quarter ms", "Eighth ms", "DotEighth ms"
    );
    println!("{:->6} {:->12} {:->12} {:->12}", "", "", "", "");

    for bpm in &bpms {
        tempo.set_bpm(*bpm);
        println!(
            "{:>6.0} {:>12.1} {:>12.1} {:>12.1}",
            bpm,
            tempo.division_to_ms(NoteDivision::Quarter),
            tempo.division_to_ms(NoteDivision::Eighth),
            tempo.division_to_ms(NoteDivision::DottedEighth),
        );
    }

    // --- Tempo-synced LFO ---
    println!("\n=== Tempo-Synced LFO ===\n");

    tempo.set_bpm(120.0);

    let mut lfo = Lfo::new(sample_rate, 1.0);
    lfo.set_waveform(LfoWaveform::Sine);
    lfo.sync_to_tempo(120.0, NoteDivision::Quarter);

    println!(
        "LFO synced to quarter notes at 120 BPM: {:.3} Hz",
        lfo.frequency()
    );

    // Show one cycle of the synced LFO
    let lfo_period_samples = (sample_rate / lfo.frequency()) as usize;
    println!(
        "Period: {} samples ({:.1} ms)\n",
        lfo_period_samples,
        1000.0 / lfo.frequency()
    );

    println!("Phase    | Value");
    println!("---------+------");
    for i in 0..lfo_period_samples {
        let val = lfo.advance();
        // Print 8 evenly spaced samples across the period
        if i % (lfo_period_samples / 8) == 0 {
            let phase = i as f32 / lfo_period_samples as f32;
            println!("{:>7.3}  | {:>6.3}", phase, val);
        }
    }

    // Sync to different divisions
    println!("\nLFO sync to various divisions at 120 BPM:");
    let sync_divisions = [
        ("Whole note", NoteDivision::Whole),
        ("Half note", NoteDivision::Half),
        ("Quarter note", NoteDivision::Quarter),
        ("Eighth note", NoteDivision::Eighth),
        ("Dotted eighth", NoteDivision::DottedEighth),
        ("Sixteenth note", NoteDivision::Sixteenth),
    ];

    for (name, div) in &sync_divisions {
        lfo.sync_to_tempo(120.0, *div);
        println!("  {:<20} -> {:.3} Hz", name, lfo.frequency());
    }

    // --- Stop transport ---
    tempo.stop();
    let pos_before = tempo.beat_position();
    for _ in 0..1000 {
        tempo.advance();
    }
    let pos_after = tempo.beat_position();

    println!("\n=== Transport Stop ===");
    println!(
        "Stopped transport. Position before: {:.2}, after 1000 samples: {:.2}",
        pos_before, pos_after
    );
    println!(
        "Position unchanged: {}",
        (pos_before - pos_after).abs() < 0.001
    );

    println!("\nTempo sync demo complete.");
}
