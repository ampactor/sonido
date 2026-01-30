//! Thread-safe audio<->GUI communication.
//!
//! This module provides lock-free parameter sharing between the GUI thread
//! and the audio thread, ensuring no priority inversion or audio glitches.

use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

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
    pub preamp: AtomicBool,
    pub distortion: AtomicBool,
    pub compressor: AtomicBool,
    pub chorus: AtomicBool,
    pub delay: AtomicBool,
    pub filter: AtomicBool,
    pub multivibrato: AtomicBool,
    pub tape: AtomicBool,
    pub reverb: AtomicBool,
}

impl Default for BypassStates {
    fn default() -> Self {
        Self {
            preamp: AtomicBool::new(false),
            distortion: AtomicBool::new(false),
            compressor: AtomicBool::new(false),
            chorus: AtomicBool::new(false),
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
    // Global
    pub input_gain: AtomicParam,
    pub master_volume: AtomicParam,

    // Preamp
    pub preamp_gain: AtomicParam,

    // Distortion
    pub dist_drive: AtomicParam,
    pub dist_tone: AtomicParam,
    pub dist_level: AtomicParam,
    pub dist_waveshape: AtomicU32, // 0-3 for WaveShape enum

    // Compressor
    pub comp_threshold: AtomicParam,
    pub comp_ratio: AtomicParam,
    pub comp_attack: AtomicParam,
    pub comp_release: AtomicParam,
    pub comp_makeup: AtomicParam,

    // Chorus
    pub chorus_rate: AtomicParam,
    pub chorus_depth: AtomicParam,
    pub chorus_mix: AtomicParam,

    // Delay
    pub delay_time: AtomicParam,
    pub delay_feedback: AtomicParam,
    pub delay_mix: AtomicParam,

    // Filter
    pub filter_cutoff: AtomicParam,
    pub filter_resonance: AtomicParam,

    // MultiVibrato
    pub vibrato_depth: AtomicParam,

    // Tape Saturation
    pub tape_drive: AtomicParam,
    pub tape_saturation: AtomicParam,

    // Reverb
    pub reverb_room_size: AtomicParam,
    pub reverb_decay: AtomicParam,
    pub reverb_damping: AtomicParam,
    pub reverb_predelay: AtomicParam,
    pub reverb_mix: AtomicParam,
    pub reverb_type: AtomicU32, // 0-1 for ReverbType enum

    // Bypass states
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

            // Chorus
            chorus_rate: AtomicParam::new(1.0, 0.1, 10.0),
            chorus_depth: AtomicParam::new(0.5, 0.0, 1.0),
            chorus_mix: AtomicParam::new(0.5, 0.0, 1.0),

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
    pub input_peak: f32,
    pub input_rms: f32,
    pub output_peak: f32,
    pub output_rms: f32,
    pub gain_reduction: f32, // For compressor
}

/// Audio bridge for communication between GUI and audio threads.
pub struct AudioBridge {
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
        // Default order: preamp, dist, comp, chorus, delay, filter, vibrato, tape, reverb
        Self {
            order: Arc::new(parking_lot::RwLock::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])),
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
        assert_eq!(order.get(), vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);

        order.move_effect(0, 2);
        assert_eq!(order.get(), vec![1, 2, 0, 3, 4, 5, 6, 7, 8]);
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
        });

        let data = bridge.receive_metering();
        assert!(data.is_some());
        let data = data.unwrap();
        assert_eq!(data.input_peak, 0.5);
    }
}
