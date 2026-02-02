//! Integration tests for sonido-config.
//!
//! These tests verify end-to-end functionality across modules.

use sonido_config::{
    EffectChain, EffectConfig, Preset,
    factory_presets, get_factory_preset,
};
use sonido_core::Effect;
use tempfile::TempDir;

/// Test creating an EffectChain from a programmatic preset and processing audio.
#[test]
fn test_preset_to_chain_processing() {
    // Create a preset with multiple effects
    let preset = Preset::new("Integration Test")
        .with_description("Test preset for integration testing")
        .with_effect(EffectConfig::new("preamp").with_param("gain", "0"))
        .with_effect(EffectConfig::new("distortion").with_param("drive", "0.3"))
        .with_effect(EffectConfig::new("!reverb")); // bypassed

    // Create chain from preset
    let mut chain = EffectChain::from_preset(&preset, 48000.0)
        .expect("should create chain from preset");

    assert_eq!(chain.len(), 3);
    assert!(!chain.is_bypassed(0).unwrap()); // preamp active
    assert!(!chain.is_bypassed(1).unwrap()); // distortion active
    assert!(chain.is_bypassed(2).unwrap());  // reverb bypassed

    // Process some samples
    let input_samples: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
    let mut output_samples = vec![0.0f32; 1024];

    chain.process_block(&input_samples, &mut output_samples);

    // Verify output is valid (finite, not all zeros)
    assert!(output_samples.iter().all(|&s| s.is_finite()));
    assert!(output_samples.iter().any(|&s| s != 0.0));
}

/// Test factory presets can be loaded and converted to chains.
#[test]
fn test_factory_preset_to_chain() {
    let presets = factory_presets();
    assert!(!presets.is_empty(), "should have factory presets");

    for preset in presets {
        let chain = EffectChain::from_preset(&preset, 48000.0);
        assert!(
            chain.is_ok(),
            "factory preset '{}' should create valid chain: {:?}",
            preset.name,
            chain.err()
        );

        let mut chain = chain.unwrap();

        // Process a test signal
        let output = chain.process(0.5);
        assert!(output.is_finite(), "preset '{}' produced non-finite output", preset.name);
    }
}

/// Test getting a specific factory preset and processing with it.
#[test]
fn test_crunch_preset_processing() {
    let preset = get_factory_preset("crunch").expect("crunch preset should exist");

    let mut chain = EffectChain::from_preset(&preset, 48000.0)
        .expect("should create chain from crunch preset");

    // Crunch should have distortion effect
    let types = chain.effect_types();
    assert!(
        types.iter().any(|t| t == "distortion"),
        "crunch preset should have distortion effect"
    );

    // Process a guitar-like signal (sine wave)
    let mut max_output = 0.0f32;
    for i in 0..4800 {
        let input = (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48000.0).sin() * 0.3;
        let output = chain.process(input);
        max_output = max_output.max(output.abs());
    }

    // Distortion should increase the signal level somewhat
    assert!(max_output > 0.0, "should produce output");
    assert!(max_output.is_finite(), "output should be finite");
}

/// Test preset save/load roundtrip.
#[test]
fn test_preset_save_load_roundtrip() {
    let temp_dir = TempDir::new().expect("should create temp dir");
    let preset_path = temp_dir.path().join("test_preset.toml");

    // Create and save a preset
    let original = Preset::new("Roundtrip Test")
        .with_description("Testing save/load")
        .with_effect(EffectConfig::new("distortion").with_param("drive", "0.7"))
        .with_effect(EffectConfig::new("!delay").with_param("time", "300ms"));

    original.save(&preset_path).expect("should save preset");

    // Load it back
    let loaded = Preset::load(&preset_path).expect("should load preset");

    // Verify contents match
    assert_eq!(loaded.name, original.name);
    assert_eq!(loaded.description, original.description);
    assert_eq!(loaded.effects.len(), original.effects.len());

    // Both should create identical chains
    let mut chain1 = EffectChain::from_preset(&original, 48000.0).unwrap();
    let mut chain2 = EffectChain::from_preset(&loaded, 48000.0).unwrap();

    // Process the same signal through both
    for i in 0..100 {
        let input = (i as f32 * 0.1).sin() * 0.5;
        let out1 = chain1.process(input);
        let out2 = chain2.process(input);
        assert!(
            (out1 - out2).abs() < 1e-6,
            "chains should produce identical output"
        );
    }
}

/// Test effect chain bypass toggling.
#[test]
fn test_chain_bypass_toggling() {
    let mut chain = EffectChain::from_effect_types(
        &["distortion", "reverb"],
        48000.0
    ).expect("should create chain");

    // Process with all effects active
    let input = 0.5;
    let _with_effects = chain.process(input);

    // Bypass distortion
    chain.set_bypassed(0, true);
    chain.reset(); // Reset to clear any state

    // Process again - should be different (only reverb)
    let _with_only_reverb = chain.process(input);

    // Bypass reverb too
    chain.set_bypassed(1, true);
    chain.reset();

    // Should now be passthrough
    let passthrough = chain.process(input);
    assert_eq!(passthrough, input, "fully bypassed chain should passthrough");

    // Toggle distortion back on
    chain.toggle_bypass(0);
    assert!(!chain.is_bypassed(0).unwrap());
    assert!(chain.is_bypassed(1).unwrap());
}

/// Test chain creation from effect type strings with bypass prefix.
#[test]
fn test_chain_from_effect_types_with_bypass() {
    let chain = EffectChain::from_effect_types(
        &["preamp", "!distortion", "compressor", "!reverb"],
        44100.0
    ).expect("should create chain");

    assert_eq!(chain.len(), 4);
    assert!(!chain.is_bypassed(0).unwrap()); // preamp active
    assert!(chain.is_bypassed(1).unwrap());  // distortion bypassed
    assert!(!chain.is_bypassed(2).unwrap()); // compressor active
    assert!(chain.is_bypassed(3).unwrap());  // reverb bypassed

    let types = chain.effect_types();
    assert_eq!(types, vec!["preamp", "!distortion", "compressor", "!reverb"]);
}

/// Test that unknown effects produce appropriate errors.
#[test]
fn test_unknown_effect_error() {
    let preset = Preset::new("Bad Preset")
        .with_effect(EffectConfig::new("nonexistent_effect"));

    let result = EffectChain::from_preset(&preset, 48000.0);
    assert!(result.is_err(), "should fail with unknown effect");
}

/// Test ambient preset for time-based effects.
#[test]
fn test_ambient_preset_has_delay_and_reverb() {
    let preset = get_factory_preset("ambient").expect("ambient preset should exist");

    let chain = EffectChain::from_preset(&preset, 48000.0)
        .expect("should create chain from ambient preset");

    let types = chain.effect_types();

    // Ambient should have both delay and reverb active
    let has_delay = types.iter().any(|t| t == "delay");
    let has_reverb = types.iter().any(|t| t == "reverb");

    assert!(has_delay, "ambient should have delay");
    assert!(has_reverb, "ambient should have reverb");
}

/// Test sample rate changes propagate through chain.
#[test]
fn test_chain_sample_rate_change() {
    let mut chain = EffectChain::from_effect_types(
        &["delay", "reverb"],
        48000.0
    ).expect("should create chain");

    assert_eq!(chain.sample_rate(), 48000.0);

    chain.set_sample_rate(96000.0);
    assert_eq!(chain.sample_rate(), 96000.0);

    // Should still process correctly at new rate
    let output = chain.process(0.5);
    assert!(output.is_finite());
}

/// Test chain reset clears internal state.
#[test]
fn test_chain_reset() {
    let mut chain = EffectChain::from_effect_types(
        &["delay"],
        48000.0
    ).expect("should create chain with delay");

    // Feed signal to fill delay buffer
    for _ in 0..10000 {
        chain.process(0.5);
    }

    // Reset the chain
    chain.reset();

    // After reset, first sample should have minimal delay contribution
    // (depends on delay implementation, but should be much quieter)
    let first_after_reset = chain.process(0.0);
    assert!(first_after_reset.abs() < 0.1, "reset should clear delay buffer");
}
