//! Preset and configuration demo: effect configs, factory presets, and chains.
//!
//! Run with: cargo run -p sonido-config --example preset_demo

use sonido_config::{
    EffectChain, EffectConfig, Preset, get_factory_preset,
    factory_preset_names, is_factory_preset, parse_param_value,
};
use sonido_core::Effect;

fn main() {
    // --- EffectConfig basics ---
    println!("=== Effect Configuration ===\n");

    let distortion = EffectConfig::new("distortion")
        .with_param("drive", "0.7")
        .with_param("tone", "5kHz")
        .with_param("level", "-6dB");

    println!("Effect: {}", distortion.effect_type);
    println!("Bypassed: {}", distortion.bypassed);
    println!("Params:");
    for (key, value) in &distortion.params {
        let parsed = parse_param_value(value);
        println!("  {}: {} -> parsed: {:?}", key, value, parsed);
    }

    // Bypassed effect
    let reverb = EffectConfig::new("!reverb")
        .with_param("room_size", "80%")
        .with_param("damping", "30%");

    println!("\nEffect: {} (display: {})", reverb.effect_type, reverb.display_type());
    println!("Bypassed: {}", reverb.bypassed);

    // --- Parameter value parsing ---
    println!("\n=== Parameter Value Parsing ===\n");

    let test_values = [
        ("0.5", "plain number"),
        ("50%", "percentage"),
        ("-6dB", "decibels"),
        ("100ms", "milliseconds"),
        ("1.5s", "seconds"),
        ("440Hz", "hertz"),
        ("1.2kHz", "kilohertz"),
    ];

    println!("{:<12} {:<18} {:>10}", "Input", "Type", "Parsed");
    println!("{:-<12} {:-<18} {:->10}", "", "", "");

    for (input, desc) in &test_values {
        let parsed = parse_param_value(input).unwrap();
        println!("{:<12} {:<18} {:>10.4}", input, desc, parsed);
    }

    // --- Preset creation ---
    println!("\n=== Preset Creation ===\n");

    let preset = Preset::new("My Guitar Tone")
        .with_description("Warm crunch with ambient reverb")
        .with_sample_rate(48000)
        .with_effect(
            EffectConfig::new("distortion")
                .with_param("drive", "0.6")
                .with_param("tone", "0.5"),
        )
        .with_effect(
            EffectConfig::new("compressor")
                .with_param("threshold", "-20")
                .with_param("ratio", "4"),
        )
        .with_effect(
            EffectConfig::new("reverb")
                .with_param("room_size", "0.7")
                .with_param("damping", "0.4"),
        );

    println!("Preset: {}", preset.name);
    println!("Description: {}", preset.description.as_deref().unwrap_or("none"));
    println!("Sample rate: {}", preset.sample_rate);
    println!("Effects ({}):", preset.len());

    for (i, effect) in preset.iter().enumerate() {
        println!("  {}: {} {}", i, effect.display_type(),
            if effect.bypassed { "(bypassed)" } else { "" });
    }

    // TOML serialization
    println!("\n--- Serialized TOML ---");
    let toml = preset.to_toml().unwrap();
    println!("{}", toml);

    // --- Factory presets ---
    println!("=== Factory Presets ===\n");

    let names = factory_preset_names();
    println!("Available factory presets: {}", names.len());
    for name in &names {
        let preset = get_factory_preset(name).unwrap();
        let active_effects: Vec<_> = preset.effects.iter()
            .filter(|e| !e.bypassed)
            .map(|e| e.effect_type.as_str())
            .collect();
        println!(
            "  {:<15} - {} [{}]",
            name,
            preset.description.as_deref().unwrap_or(""),
            active_effects.join(", ")
        );
    }

    // Check if a name is a factory preset
    println!("\nIs 'crunch' a factory preset? {}", is_factory_preset("crunch"));
    println!("Is 'my_custom' a factory preset? {}", is_factory_preset("my_custom"));

    // --- Effect chain ---
    println!("\n=== Effect Chain ===\n");

    // Build a chain from a preset
    let ambient = get_factory_preset("ambient").unwrap();
    let mut chain = EffectChain::from_preset(&ambient, 48000.0).unwrap();

    println!("Chain from 'ambient' preset:");
    println!("  Effects: {}", chain.len());
    println!("  Types: {:?}", chain.effect_types());

    // Process a test signal through the chain
    let mut output_samples = Vec::new();
    for i in 0..100 {
        let input = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin() * 0.5;
        let output = chain.process(input);
        output_samples.push(output);
    }

    println!("  Processed 100 samples through chain");
    println!(
        "  Input peak: 0.500, Output peak: {:.4}",
        output_samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
    );

    // Build a chain from effect types
    let mut chain2 = EffectChain::from_effect_types(
        &["distortion", "!reverb", "compressor"],
        48000.0,
    )
    .unwrap();

    println!("\nChain from type list:");
    println!("  Types: {:?}", chain2.effect_types());

    // Toggle bypass
    chain2.toggle_bypass(1); // Un-bypass reverb
    println!("  After un-bypassing reverb: {:?}", chain2.effect_types());

    chain2.set_bypassed(0, true); // Bypass distortion
    println!("  After bypassing distortion: {:?}", chain2.effect_types());

    println!("\nPreset demo complete.");
}
