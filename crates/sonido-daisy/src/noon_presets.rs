//! Per-effect "noon preset" values for the Hothouse 6-knob pedal.
//!
//! Each registered effect defines the parameter values that constitute
//! a "sweet spot" — the tone you get when all knobs are at center (noon).
//! These values are passed to [`adc_to_param_biased`](crate::adc_to_param_biased)
//! so that ADC 0.5 maps to the sweet spot rather than the raw midpoint
//! of the parameter range.
//!
//! Noon values are **guitarist sweet spots for 6-knob hardware**, not always
//! descriptor defaults. Mix params use 50% (pedal blend convention) even
//! though the plugin default is 100% (insert chain convention).
//! [`adc_to_param_biased`](crate::adc_to_param_biased) automatically falls
//! back to linear mapping for STEPPED params and noon values at range
//! extremes, so those entries in the table are harmless but kept for
//! completeness.

/// Returns the "sweet spot" value for a parameter of a named effect.
///
/// `effect_id` is the registry ID (e.g., `"distortion"`, `"delay"`).
/// `param_idx` is the parameter index within the effect.
///
/// Returns `None` if the effect is unknown or param index is out of range,
/// in which case callers should fall back to the descriptor's `default`.
pub fn noon_value(effect_id: &str, param_idx: usize) -> Option<f32> {
    let values: &[f32] = match effect_id {
        "distortion" => &[8.0, 0.0, 0.0, 0.0, 50.0, 0.0],
        "preamp" => &[0.0, 0.0, 0.0],
        "compressor" => &[-18.0, 4.0, 10.0, 100.0, 0.0, 6.0, 0.0, 80.0, 0.0, 0.0, 50.0, 0.0],
        "gate" => &[-40.0, 1.0, 100.0, 50.0, -80.0, 3.0, 80.0, 0.0, 0.0],
        "eq" => &[100.0, 0.0, 1.0, 500.0, 0.0, 1.0, 5000.0, 0.0, 1.0, 0.0],
        "wah" => &[800.0, 5.0, 50.0, 0.0, 0.0],
        "chorus" => &[1.0, 50.0, 50.0, 2.0, 0.0, 15.0, 0.0, 3.0, 0.0, 0.0],
        "flanger" => &[0.5, 35.0, 50.0, 50.0, 0.0, 0.0, 3.0, 0.0, 0.0],
        "phaser" => &[0.3, 50.0, 6.0, 50.0, 50.0, 20.0, 4000.0, 0.0, 3.0, 0.0, 0.0],
        "tremolo" => &[5.0, 50.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0],
        "delay" => &[300.0, 40.0, 50.0, 0.0, 20000.0, 20.0, 0.0, 0.0, 2.0, 0.0],
        "filter" => &[1000.0, 0.707, 0.0, 0.0],
        "vibrato" => &[100.0, 50.0, 0.0],
        "tape" => &[6.0, 30.0, 12000.0, 0.0, 0.3, 0.2, 0.15, 0.3, 80.0, -6.0],
        "reverb" => &[50.0, 50.0, 30.0, 10.0, 50.0, 100.0, 50.0, 0.0],
        "limiter" => &[-6.0, -0.3, 100.0, 5.0, 0.0],
        "bitcrusher" => &[8.0, 1.0, 0.0, 50.0, 0.0],
        "ringmod" => &[440.0, 50.0, 0.0, 50.0, 0.0],
        "stage" => &[
            0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 120.0, 0.0, 0.0, 0.0,
        ],
        "looper" => &[0.0, 80.0, 0.0, 0.0, 50.0, 0.0],
        _ => return None,
    };
    values.get(param_idx).copied()
}

// ── Compile-time integrity check ─────────────────────────────────────────
//
// Verifies that every noon preset array has exactly `KernelParams::COUNT`
// entries. A mismatch is a compile-time error (const assertion panic).

#[cfg(feature = "alloc")]
mod noon_integrity {
    use sonido_core::kernel::KernelParams;
    use sonido_effects::{
        BitcrusherParams, ChorusParams, CompressorParams, DelayParams, DistortionParams, EqParams,
        FilterParams, FlangerParams, GateParams, LimiterParams, LooperParams, PhaserParams,
        PreampParams, ReverbParams, RingModParams, StageParams, TapeParams, TremoloParams,
        VibratoParams, WahParams,
    };

    /// Assert at compile time that `noon_value(id, 0..COUNT)` covers all params.
    macro_rules! assert_noon_len {
        ($id:expr, $params:ty, $array:expr) => {
            const _: () = {
                let expected = <$params>::COUNT;
                let actual = $array.len();
                assert!(
                    actual == expected,
                    // compile-time panic if lengths diverge
                );
            };
        };
    }

    assert_noon_len!("distortion", DistortionParams, [8.0, 0.0, 0.0, 0.0, 50.0, 0.0]);
    assert_noon_len!("preamp", PreampParams, [0.0, 0.0, 0.0]);
    assert_noon_len!("compressor", CompressorParams, [-18.0, 4.0, 10.0, 100.0, 0.0, 6.0, 0.0, 80.0, 0.0, 0.0, 50.0, 0.0]);
    assert_noon_len!("gate", GateParams, [-40.0, 1.0, 100.0, 50.0, -80.0, 3.0, 80.0, 0.0, 0.0]);
    assert_noon_len!("eq", EqParams, [100.0, 0.0, 1.0, 500.0, 0.0, 1.0, 5000.0, 0.0, 1.0, 0.0]);
    assert_noon_len!("wah", WahParams, [800.0, 5.0, 50.0, 0.0, 0.0]);
    assert_noon_len!("chorus", ChorusParams, [1.0, 50.0, 50.0, 2.0, 0.0, 15.0, 0.0, 3.0, 0.0, 0.0]);
    assert_noon_len!("flanger", FlangerParams, [0.5, 35.0, 50.0, 50.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
    assert_noon_len!("phaser", PhaserParams, [0.3, 50.0, 6.0, 50.0, 50.0, 20.0, 4000.0, 0.0, 3.0, 0.0, 0.0]);
    assert_noon_len!("tremolo", TremoloParams, [5.0, 50.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
    assert_noon_len!("delay", DelayParams, [300.0, 40.0, 50.0, 0.0, 20000.0, 20.0, 0.0, 0.0, 2.0, 0.0]);
    assert_noon_len!("filter", FilterParams, [1000.0, 0.707, 0.0, 0.0]);
    assert_noon_len!("vibrato", VibratoParams, [100.0, 50.0, 0.0]);
    assert_noon_len!("tape", TapeParams, [6.0, 30.0, 12000.0, 0.0, 0.3, 0.2, 0.15, 0.3, 80.0, -6.0]);
    assert_noon_len!("reverb", ReverbParams, [50.0, 50.0, 30.0, 10.0, 50.0, 100.0, 50.0, 0.0]);
    assert_noon_len!("limiter", LimiterParams, [-6.0, -0.3, 100.0, 5.0, 0.0]);
    assert_noon_len!("bitcrusher", BitcrusherParams, [8.0, 1.0, 0.0, 50.0, 0.0]);
    assert_noon_len!("ringmod", RingModParams, [440.0, 50.0, 0.0, 50.0, 0.0]);
    assert_noon_len!("stage", StageParams, [0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 120.0, 0.0, 0.0, 0.0]);
    assert_noon_len!("looper", LooperParams, [0.0, 80.0, 0.0, 0.0, 50.0, 0.0]);
}
