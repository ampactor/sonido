//! Thread-safe audio<->GUI communication.
//!
//! This module provides lock-free parameter sharing between the GUI thread
//! and the audio thread, ensuring no priority inversion or audio glitches.

use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// A thread-safe atomic parameter using bit-cast f32.
///
/// GUI thread writes, audio thread reads. No locks, no allocations.
#[derive(Debug)]
pub struct AtomicParam {
    value: AtomicU32,
    min: f32,
    max: f32,
    default: f32,
}

impl AtomicParam {
    /// Create a new atomic parameter with range and default.
    pub fn new(default: f32, min: f32, max: f32) -> Self {
        Self {
            value: AtomicU32::new(default.to_bits()),
            min,
            max,
            default,
        }
    }

    /// Set the parameter value (GUI thread).
    #[inline]
    pub fn set(&self, v: f32) {
        let clamped = v.clamp(self.min, self.max);
        self.value.store(clamped.to_bits(), Ordering::Release);
    }

    /// Get the parameter value (audio thread).
    #[inline]
    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Acquire))
    }

    /// Get the minimum value.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Get the maximum value.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Get the default value.
    pub fn default(&self) -> f32 {
        self.default
    }

    /// Reset to default value.
    pub fn reset(&self) {
        self.set(self.default);
    }
}

impl Clone for AtomicParam {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU32::new(self.value.load(Ordering::Acquire)),
            min: self.min,
            max: self.max,
            default: self.default,
        }
    }
}

/// Effect bypass states (atomic bools for lock-free access).
#[derive(Debug)]
pub struct BypassStates {
    /// Preamp bypass state.
    pub preamp: AtomicBool,
    /// Distortion bypass state.
    pub distortion: AtomicBool,
    /// Compressor bypass state.
    pub compressor: AtomicBool,
    /// Gate bypass state.
    pub gate: AtomicBool,
    /// Parametric EQ bypass state.
    pub eq: AtomicBool,
    /// Wah bypass state.
    pub wah: AtomicBool,
    /// Chorus bypass state.
    pub chorus: AtomicBool,
    /// Flanger bypass state.
    pub flanger: AtomicBool,
    /// Phaser bypass state.
    pub phaser: AtomicBool,
    /// Tremolo bypass state.
    pub tremolo: AtomicBool,
    /// Delay bypass state.
    pub delay: AtomicBool,
    /// Filter bypass state.
    pub filter: AtomicBool,
    /// Multi-vibrato bypass state.
    pub multivibrato: AtomicBool,
    /// Tape saturation bypass state.
    pub tape: AtomicBool,
    /// Reverb bypass state.
    pub reverb: AtomicBool,
}

impl Default for BypassStates {
    fn default() -> Self {
        Self {
            preamp: AtomicBool::new(false),
            distortion: AtomicBool::new(false),
            compressor: AtomicBool::new(false),
            gate: AtomicBool::new(true), // Gate bypassed by default
            eq: AtomicBool::new(true),   // EQ bypassed by default
            wah: AtomicBool::new(true),  // Wah bypassed by default
            chorus: AtomicBool::new(false),
            flanger: AtomicBool::new(true), // Flanger bypassed by default
            phaser: AtomicBool::new(true),  // Phaser bypassed by default
            tremolo: AtomicBool::new(true), // Tremolo bypassed by default
            delay: AtomicBool::new(false),
            filter: AtomicBool::new(false),
            multivibrato: AtomicBool::new(false),
            tape: AtomicBool::new(false),
            reverb: AtomicBool::new(false),
        }
    }
}

/// Shared parameters between GUI and audio threads.
///
/// All parameters use atomic operations for lock-free access.
#[derive(Debug)]
pub struct SharedParams {
    /// Input gain in dB.
    pub input_gain: AtomicParam,
    /// Master output volume in dB.
    pub master_volume: AtomicParam,

    /// Preamp gain in dB.
    pub preamp_gain: AtomicParam,

    /// Distortion drive amount in dB.
    pub dist_drive: AtomicParam,
    /// Distortion tone control.
    pub dist_tone: AtomicParam,
    /// Distortion output level.
    pub dist_level: AtomicParam,
    /// Distortion waveshape type (0–3, maps to `WaveShape` enum).
    pub dist_waveshape: AtomicU32,

    /// Compressor threshold in dB.
    pub comp_threshold: AtomicParam,
    /// Compressor ratio.
    pub comp_ratio: AtomicParam,
    /// Compressor attack time in ms.
    pub comp_attack: AtomicParam,
    /// Compressor release time in ms.
    pub comp_release: AtomicParam,
    /// Compressor makeup gain in dB.
    pub comp_makeup: AtomicParam,

    /// Gate threshold in dB.
    pub gate_threshold: AtomicParam,
    /// Gate attack time in ms.
    pub gate_attack: AtomicParam,
    /// Gate release time in ms.
    pub gate_release: AtomicParam,
    /// Gate hold time in ms.
    pub gate_hold: AtomicParam,

    /// Low-band EQ center frequency in Hz.
    pub eq_low_freq: AtomicParam,
    /// Low-band EQ gain in dB.
    pub eq_low_gain: AtomicParam,
    /// Low-band EQ Q factor.
    pub eq_low_q: AtomicParam,
    /// Mid-band EQ center frequency in Hz.
    pub eq_mid_freq: AtomicParam,
    /// Mid-band EQ gain in dB.
    pub eq_mid_gain: AtomicParam,
    /// Mid-band EQ Q factor.
    pub eq_mid_q: AtomicParam,
    /// High-band EQ center frequency in Hz.
    pub eq_high_freq: AtomicParam,
    /// High-band EQ gain in dB.
    pub eq_high_gain: AtomicParam,
    /// High-band EQ Q factor.
    pub eq_high_q: AtomicParam,

    /// Wah filter frequency in Hz.
    pub wah_frequency: AtomicParam,
    /// Wah filter resonance.
    pub wah_resonance: AtomicParam,
    /// Wah envelope sensitivity.
    pub wah_sensitivity: AtomicParam,
    /// Wah mode (0 = auto, 1 = manual).
    pub wah_mode: AtomicU32,

    /// Chorus LFO rate in Hz.
    pub chorus_rate: AtomicParam,
    /// Chorus modulation depth.
    pub chorus_depth: AtomicParam,
    /// Chorus wet/dry mix.
    pub chorus_mix: AtomicParam,

    /// Flanger LFO rate in Hz.
    pub flanger_rate: AtomicParam,
    /// Flanger modulation depth.
    pub flanger_depth: AtomicParam,
    /// Flanger feedback amount.
    pub flanger_feedback: AtomicParam,
    /// Flanger wet/dry mix.
    pub flanger_mix: AtomicParam,

    /// Phaser LFO rate in Hz.
    pub phaser_rate: AtomicParam,
    /// Phaser modulation depth.
    pub phaser_depth: AtomicParam,
    /// Phaser feedback amount.
    pub phaser_feedback: AtomicParam,
    /// Phaser wet/dry mix.
    pub phaser_mix: AtomicParam,
    /// Number of phaser allpass stages (2–12).
    pub phaser_stages: AtomicU32,

    /// Tremolo LFO rate in Hz.
    pub tremolo_rate: AtomicParam,
    /// Tremolo modulation depth.
    pub tremolo_depth: AtomicParam,
    /// Tremolo waveform (0 = sine, 1 = triangle, 2 = square, 3 = S&H).
    pub tremolo_waveform: AtomicU32,

    /// Delay time in ms.
    pub delay_time: AtomicParam,
    /// Delay feedback amount.
    pub delay_feedback: AtomicParam,
    /// Delay wet/dry mix.
    pub delay_mix: AtomicParam,

    /// Filter cutoff frequency in Hz.
    pub filter_cutoff: AtomicParam,
    /// Filter resonance.
    pub filter_resonance: AtomicParam,

    /// Vibrato modulation depth.
    pub vibrato_depth: AtomicParam,

    /// Tape saturation drive amount.
    pub tape_drive: AtomicParam,
    /// Tape saturation amount.
    pub tape_saturation: AtomicParam,

    /// Reverb room size.
    pub reverb_room_size: AtomicParam,
    /// Reverb decay time.
    pub reverb_decay: AtomicParam,
    /// Reverb high-frequency damping.
    pub reverb_damping: AtomicParam,
    /// Reverb pre-delay in ms.
    pub reverb_predelay: AtomicParam,
    /// Reverb wet/dry mix.
    pub reverb_mix: AtomicParam,
    /// Reverb algorithm type (0–1, maps to `ReverbType` enum).
    pub reverb_type: AtomicU32,

    /// Effect bypass states.
    pub bypass: BypassStates,
}

impl Default for SharedParams {
    fn default() -> Self {
        Self {
            // Global: -20 to +20 dB
            input_gain: AtomicParam::new(0.0, -20.0, 20.0),
            master_volume: AtomicParam::new(0.0, -40.0, 6.0),

            // Preamp: -20 to +20 dB
            preamp_gain: AtomicParam::new(0.0, -20.0, 20.0),

            // Distortion
            dist_drive: AtomicParam::new(0.0, 0.0, 40.0),
            dist_tone: AtomicParam::new(8000.0, 500.0, 10000.0),
            dist_level: AtomicParam::new(0.0, -20.0, 0.0),
            dist_waveshape: AtomicU32::new(0),

            // Compressor
            comp_threshold: AtomicParam::new(-20.0, -40.0, 0.0),
            comp_ratio: AtomicParam::new(4.0, 1.0, 20.0),
            comp_attack: AtomicParam::new(10.0, 0.1, 100.0),
            comp_release: AtomicParam::new(100.0, 10.0, 1000.0),
            comp_makeup: AtomicParam::new(0.0, 0.0, 20.0),

            // Gate
            gate_threshold: AtomicParam::new(-40.0, -80.0, 0.0),
            gate_attack: AtomicParam::new(1.0, 0.1, 50.0),
            gate_release: AtomicParam::new(100.0, 10.0, 1000.0),
            gate_hold: AtomicParam::new(50.0, 0.0, 500.0),

            // Parametric EQ
            eq_low_freq: AtomicParam::new(100.0, 20.0, 500.0),
            eq_low_gain: AtomicParam::new(0.0, -12.0, 12.0),
            eq_low_q: AtomicParam::new(1.0, 0.5, 5.0),
            eq_mid_freq: AtomicParam::new(1000.0, 200.0, 5000.0),
            eq_mid_gain: AtomicParam::new(0.0, -12.0, 12.0),
            eq_mid_q: AtomicParam::new(1.0, 0.5, 5.0),
            eq_high_freq: AtomicParam::new(5000.0, 1000.0, 15000.0),
            eq_high_gain: AtomicParam::new(0.0, -12.0, 12.0),
            eq_high_q: AtomicParam::new(1.0, 0.5, 5.0),

            // Wah
            wah_frequency: AtomicParam::new(800.0, 200.0, 2000.0),
            wah_resonance: AtomicParam::new(5.0, 1.0, 10.0),
            wah_sensitivity: AtomicParam::new(0.5, 0.0, 1.0),
            wah_mode: AtomicU32::new(0), // Auto mode

            // Chorus
            chorus_rate: AtomicParam::new(1.0, 0.1, 10.0),
            chorus_depth: AtomicParam::new(0.5, 0.0, 1.0),
            chorus_mix: AtomicParam::new(0.5, 0.0, 1.0),

            // Flanger
            flanger_rate: AtomicParam::new(0.5, 0.05, 5.0),
            flanger_depth: AtomicParam::new(0.5, 0.0, 1.0),
            flanger_feedback: AtomicParam::new(0.5, 0.0, 0.95),
            flanger_mix: AtomicParam::new(0.5, 0.0, 1.0),

            // Phaser
            phaser_rate: AtomicParam::new(0.3, 0.05, 5.0),
            phaser_depth: AtomicParam::new(0.5, 0.0, 1.0),
            phaser_feedback: AtomicParam::new(0.5, 0.0, 0.95),
            phaser_mix: AtomicParam::new(0.5, 0.0, 1.0),
            phaser_stages: AtomicU32::new(6),

            // Tremolo
            tremolo_rate: AtomicParam::new(5.0, 0.5, 20.0),
            tremolo_depth: AtomicParam::new(0.5, 0.0, 1.0),
            tremolo_waveform: AtomicU32::new(0), // Sine

            // Delay
            delay_time: AtomicParam::new(300.0, 1.0, 2000.0),
            delay_feedback: AtomicParam::new(0.4, 0.0, 0.95),
            delay_mix: AtomicParam::new(0.5, 0.0, 1.0),

            // Filter
            filter_cutoff: AtomicParam::new(5000.0, 20.0, 20000.0),
            filter_resonance: AtomicParam::new(0.7, 0.1, 10.0),

            // MultiVibrato
            vibrato_depth: AtomicParam::new(0.5, 0.0, 1.0),

            // Tape Saturation
            tape_drive: AtomicParam::new(6.0, 0.0, 24.0),
            tape_saturation: AtomicParam::new(0.5, 0.0, 1.0),

            // Reverb
            reverb_room_size: AtomicParam::new(0.5, 0.0, 1.0),
            reverb_decay: AtomicParam::new(0.5, 0.0, 1.0),
            reverb_damping: AtomicParam::new(0.5, 0.0, 1.0),
            reverb_predelay: AtomicParam::new(10.0, 0.0, 100.0),
            reverb_mix: AtomicParam::new(0.3, 0.0, 1.0),
            reverb_type: AtomicU32::new(0),

            bypass: BypassStates::default(),
        }
    }
}

/// Metering data sent from audio thread to GUI.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeteringData {
    /// Input signal peak level (linear).
    pub input_peak: f32,
    /// Input signal RMS level (linear).
    pub input_rms: f32,
    /// Output signal peak level (linear).
    pub output_peak: f32,
    /// Output signal RMS level (linear).
    pub output_rms: f32,
    /// Compressor gain reduction in dB.
    pub gain_reduction: f32,
    /// Audio thread CPU usage (0.0 to 100.0).
    pub cpu_usage: f32,
}

/// Audio bridge for communication between GUI and audio threads.
pub struct AudioBridge {
    /// Shared parameters accessible by both GUI and audio threads.
    pub params: Arc<SharedParams>,
    metering_tx: Sender<MeteringData>,
    metering_rx: Receiver<MeteringData>,
    running: Arc<AtomicBool>,
}

impl AudioBridge {
    /// Create a new audio bridge.
    pub fn new() -> Self {
        let (tx, rx) = bounded(4); // Small buffer, drop old data if full
        Self {
            params: Arc::new(SharedParams::default()),
            metering_tx: tx,
            metering_rx: rx,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get shared parameters reference.
    pub fn params(&self) -> Arc<SharedParams> {
        Arc::clone(&self.params)
    }

    /// Get the running flag.
    pub fn running(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Send metering data from audio thread (non-blocking).
    pub fn send_metering(&self, data: MeteringData) {
        // Try to send, drop if buffer is full (OK for metering)
        let _ = self.metering_tx.try_send(data);
    }

    /// Get metering sender for audio thread.
    pub fn metering_sender(&self) -> Sender<MeteringData> {
        self.metering_tx.clone()
    }

    /// Receive latest metering data (GUI thread).
    pub fn receive_metering(&self) -> Option<MeteringData> {
        // Get the most recent metering data, discarding older ones
        let mut latest = None;
        while let Ok(data) = self.metering_rx.try_recv() {
            latest = Some(data);
        }
        latest
    }

    /// Set the running state.
    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    /// Check if audio is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for AudioBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Effect chain order (indices into a fixed array of effects).
#[derive(Debug, Clone)]
pub struct EffectOrder {
    order: Arc<parking_lot::RwLock<Vec<usize>>>,
}

impl Default for EffectOrder {
    fn default() -> Self {
        // Default order with all effects
        Self {
            order: Arc::new(parking_lot::RwLock::new(vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
            ])),
        }
    }
}

impl EffectOrder {
    /// Get current effect order.
    pub fn get(&self) -> Vec<usize> {
        self.order.read().clone()
    }

    /// Set effect order.
    pub fn set(&self, order: Vec<usize>) {
        *self.order.write() = order;
    }

    /// Move effect from one position to another.
    pub fn move_effect(&self, from: usize, to: usize) {
        let mut order = self.order.write();
        if from < order.len() && to < order.len() && from != to {
            let effect = order.remove(from);
            order.insert(to, effect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_param_clamping() {
        let param = AtomicParam::new(0.5, 0.0, 1.0);

        param.set(1.5);
        assert_eq!(param.get(), 1.0);

        param.set(-0.5);
        assert_eq!(param.get(), 0.0);

        param.set(0.75);
        assert_eq!(param.get(), 0.75);
    }

    #[test]
    fn test_atomic_param_reset() {
        let param = AtomicParam::new(0.5, 0.0, 1.0);
        param.set(0.8);
        assert_eq!(param.get(), 0.8);

        param.reset();
        assert_eq!(param.get(), 0.5);
    }

    #[test]
    fn test_effect_order() {
        let order = EffectOrder::default();
        assert_eq!(
            order.get(),
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
        );

        order.move_effect(0, 2);
        assert_eq!(
            order.get(),
            vec![1, 2, 0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
        );
    }

    #[test]
    fn test_audio_bridge_metering() {
        let bridge = AudioBridge::new();

        bridge.send_metering(MeteringData {
            input_peak: 0.5,
            input_rms: 0.3,
            output_peak: 0.6,
            output_rms: 0.4,
            gain_reduction: 3.0,
            cpu_usage: 12.5,
        });

        let data = bridge.receive_metering();
        assert!(data.is_some());
        let data = data.unwrap();
        assert_eq!(data.input_peak, 0.5);
    }
}
