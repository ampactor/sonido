//! Lock-free audio↔GUI communication.
//!
//! Provides metering data transport, transport state (running flag), and
//! standalone input/master gain controls. Per-effect parameter sharing is
//! handled by [`AtomicParamBridge`](super::atomic_param_bridge) — this module
//! only owns the two global gain knobs that live outside the effect chain.

use crate::chain_manager::ChainCommand;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
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
/// outside the per-effect parameter system, plus metering transport,
/// the running flag, and a command channel for dynamic chain mutations.
pub struct AudioBridge {
    input_gain: Arc<AtomicParam>,
    master_volume: Arc<AtomicParam>,
    running: Arc<AtomicBool>,
    metering_tx: Sender<MeteringData>,
    metering_rx: Receiver<MeteringData>,
    command_tx: Sender<ChainCommand>,
    command_rx: Receiver<ChainCommand>,
}

impl AudioBridge {
    /// Create a new audio bridge.
    pub fn new() -> Self {
        let (metering_tx, metering_rx) = bounded(4);
        let (command_tx, command_rx) = unbounded();
        Self {
            input_gain: Arc::new(AtomicParam::new(1.0, 0.0, 4.0)),
            master_volume: Arc::new(AtomicParam::new(1.0, 0.0, 4.0)),
            running: Arc::new(AtomicBool::new(false)),
            metering_tx,
            metering_rx,
            command_tx,
            command_rx,
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

    /// Send a chain mutation command to the audio thread (non-blocking).
    ///
    /// Commands are drained by the audio thread at the start of each buffer
    /// via the receiver obtained from [`command_receiver`](Self::command_receiver).
    pub fn send_command(&self, cmd: ChainCommand) {
        let _ = self.command_tx.send(cmd);
    }

    /// Get a clone of the command receiver for the audio thread.
    ///
    /// `crossbeam` receivers are cheaply cloneable. In practice only one
    /// audio thread should drain the channel.
    pub fn command_receiver(&self) -> Receiver<ChainCommand> {
        self.command_rx.clone()
    }
}

impl Default for AudioBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Effect chain order (indices into a fixed array of effects).
///
/// Thread-safe via `RwLock`. Supports dynamic add/remove to match
/// `ChainManager` and `AtomicParamBridge` mutations.
#[derive(Debug, Clone)]
pub struct EffectOrder {
    order: Arc<parking_lot::RwLock<Vec<usize>>>,
}

impl Default for EffectOrder {
    fn default() -> Self {
        Self::new(15)
    }
}

impl EffectOrder {
    /// Create an order with `len` sequential indices (`0..len`).
    pub fn new(len: usize) -> Self {
        Self {
            order: Arc::new(parking_lot::RwLock::new((0..len).collect())),
        }
    }

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

    /// Append a new index at the end, returning the appended value.
    ///
    /// The appended value equals the current length before insertion,
    /// matching the index that `ChainManager::add_effect` assigns.
    pub fn push(&self) -> usize {
        let mut order = self.order.write();
        let idx = order.len();
        order.push(idx);
        idx
    }

    /// Remove `slot` from the order and remap the swapped-in last index.
    ///
    /// Mirrors the swap-remove semantics of `ChainManager::remove_effect`
    /// and `AtomicParamBridge::remove_slot`: after their swap-remove, the
    /// element that was at the last position is now at `slot`. This method
    /// removes `slot` from the order vector and replaces any occurrence of
    /// the old last index with `slot`.
    pub fn swap_remove(&self, slot: usize) {
        let mut order = self.order.write();
        let old_last = order.len().saturating_sub(1);
        order.retain(|&i| i != slot);
        if slot != old_last {
            for idx in &mut *order {
                if *idx == old_last {
                    *idx = slot;
                }
            }
        }
    }

    /// Returns the number of entries in the order.
    pub fn len(&self) -> usize {
        self.order.read().len()
    }

    /// Returns `true` if the order is empty.
    pub fn is_empty(&self) -> bool {
        self.order.read().is_empty()
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
    fn test_effect_order_new() {
        let order = EffectOrder::new(3);
        assert_eq!(order.get(), vec![0, 1, 2]);
        assert_eq!(order.len(), 3);
        assert!(!order.is_empty());

        let empty = EffectOrder::new(0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_effect_order_push() {
        let order = EffectOrder::new(2);
        assert_eq!(order.get(), vec![0, 1]);

        let idx = order.push();
        assert_eq!(idx, 2);
        assert_eq!(order.get(), vec![0, 1, 2]);
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_effect_order_swap_remove_last() {
        // Remove last element — no remap needed
        let order = EffectOrder::new(3);
        order.swap_remove(2);
        assert_eq!(order.get(), vec![0, 1]);
    }

    #[test]
    fn test_effect_order_swap_remove_middle() {
        // Remove slot 0 from [0, 1, 2] → remove 0, remap 2→0 → [1, 0]
        let order = EffectOrder::new(3);
        order.swap_remove(0);
        assert_eq!(order.get(), vec![1, 0]);
    }

    #[test]
    fn test_command_channel() {
        use sonido_registry::EffectRegistry;

        let bridge = AudioBridge::new();
        let rx = bridge.command_receiver();

        let registry = EffectRegistry::new();
        let effect = registry.create("distortion", 48000.0).unwrap();
        bridge.send_command(ChainCommand::Add {
            id: "distortion",
            effect,
        });
        bridge.send_command(ChainCommand::Remove { slot: 0 });

        // Drain the channel
        let cmd1 = rx.try_recv().unwrap();
        assert!(matches!(
            cmd1,
            ChainCommand::Add {
                id: "distortion",
                ..
            }
        ));

        let cmd2 = rx.try_recv().unwrap();
        assert!(matches!(cmd2, ChainCommand::Remove { slot: 0 }));

        assert!(rx.try_recv().is_err());
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
