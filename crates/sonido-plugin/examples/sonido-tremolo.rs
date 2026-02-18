//! Sonido Tremolo — CLAP audio effect plugin.
//!
//! Amplitude modulation with multiple waveforms.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "tremolo",
    clap_id: "com.sonido.tremolo",
    name: "Sonido Tremolo",
    features: [AUDIO_EFFECT, TREMOLO, STEREO],
}
