//! Sonido Limiter — CLAP audio effect plugin.
//!
//! Brickwall lookahead peak limiter with ceiling control.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "limiter",
    clap_id: "com.sonido.limiter",
    name: "Sonido Limiter",
    features: [AUDIO_EFFECT, COMPRESSOR, STEREO],
}
