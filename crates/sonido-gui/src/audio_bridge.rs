//! Lock-free audio↔GUI communication.
//!
//! Provides metering data transport, transport state (running flag), and
//! standalone input/master gain controls. Per-effect parameter sharing is
//! handled by [`AtomicParamBridge`](super::atomic_param_bridge) — this module
//! only owns the two global gain knobs that live outside the effect chain.

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
///
/// Owns the two global gain controls (input gain, master volume) that sit
/// outside the per-effect parameter system, plus metering transport and
/// the running flag.
pub struct AudioBridge {
    input_gain: Arc<AtomicParam>,
    master_volume: Arc<AtomicParam>,
    running: Arc<AtomicBool>,
    metering_tx: Sender<MeteringData>,
    metering_rx: Receiver<MeteringData>,
}

impl AudioBridge {
    /// Create a new audio bridge.
    pub fn new() -> Self {
        let (metering_tx, metering_rx) = bounded(4);
        Self {
            input_gain: Arc::new(AtomicParam::new(1.0, 0.0, 4.0)),
            master_volume: Arc::new(AtomicParam::new(1.0, 0.0, 4.0)),
            running: Arc::new(AtomicBool::new(false)),
            metering_tx,
            metering_rx,
        }
    }

    /// Get the input gain control.
    pub fn input_gain(&self) -> Arc<AtomicParam> {
        Arc::clone(&self.input_gain)
    }

    /// Get the master volume control.
    pub fn master_volume(&self) -> Arc<AtomicParam> {
        Arc::clone(&self.master_volume)
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

    #[test]
    fn test_audio_bridge_gain_accessors() {
        let bridge = AudioBridge::new();

        let input = bridge.input_gain();
        assert_eq!(input.get(), 1.0);
        assert_eq!(input.min(), 0.0);
        assert_eq!(input.max(), 4.0);

        let master = bridge.master_volume();
        assert_eq!(master.get(), 1.0);
        assert_eq!(master.min(), 0.0);
        assert_eq!(master.max(), 4.0);

        // Verify Arc identity — same underlying allocation
        input.set(2.0);
        assert_eq!(bridge.input_gain().get(), 2.0);

        master.set(0.5);
        assert_eq!(bridge.master_volume().get(), 0.5);
    }
}
