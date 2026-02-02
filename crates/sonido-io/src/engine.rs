//! Effect chain processing engine.

use sonido_core::Effect;
use crate::StereoSamples;

/// Processing engine that runs an effect chain.
///
/// The engine uses `Send` bounds so it can be used in audio callbacks
/// on different threads.
pub struct ProcessingEngine {
    effects: Vec<Box<dyn Effect + Send>>,
    sample_rate: f32,
}

impl ProcessingEngine {
    /// Create a new processing engine.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            effects: Vec::new(),
            sample_rate,
        }
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Set the sample rate for all effects.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for effect in &mut self.effects {
            effect.set_sample_rate(sample_rate);
        }
    }

    /// Add an effect to the chain.
    pub fn add_effect(&mut self, mut effect: Box<dyn Effect + Send>) {
        effect.set_sample_rate(self.sample_rate);
        self.effects.push(effect);
    }

    /// Clear all effects from the chain.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Get the number of effects in the chain.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get the total latency in samples.
    pub fn latency_samples(&self) -> usize {
        self.effects.iter().map(|e| e.latency_samples()).sum()
    }

    /// Reset all effects.
    pub fn reset(&mut self) {
        for effect in &mut self.effects {
            effect.reset();
        }
    }

    /// Process a single sample through the chain.
    pub fn process(&mut self, input: f32) -> f32 {
        let mut sample = input;
        for effect in &mut self.effects {
            sample = effect.process(sample);
        }
        sample
    }

    /// Process a block of samples through the chain.
    ///
    /// Output buffer must be at least as large as input.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert!(output.len() >= input.len());

        if self.effects.is_empty() {
            output[..input.len()].copy_from_slice(input);
            return;
        }

        // First effect: input -> output
        self.effects[0].process_block(input, output);

        // Remaining effects: output -> output (in-place)
        for effect in &mut self.effects[1..] {
            effect.process_block_inplace(&mut output[..input.len()]);
        }
    }

    /// Process a block of samples in-place.
    pub fn process_block_inplace(&mut self, buffer: &mut [f32]) {
        for effect in &mut self.effects {
            effect.process_block_inplace(buffer);
        }
    }

    /// Process an entire file's worth of samples.
    ///
    /// Returns a new vector with processed samples.
    pub fn process_file(&mut self, input: &[f32], block_size: usize) -> Vec<f32> {
        let mut output = vec![0.0; input.len()];

        for (in_chunk, out_chunk) in input.chunks(block_size).zip(output.chunks_mut(block_size)) {
            // Handle last chunk which may be smaller
            let len = in_chunk.len();
            self.process_block(in_chunk, &mut out_chunk[..len]);
        }

        output
    }

    // --- Stereo processing methods ---

    /// Process a single stereo sample through the chain.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mut l = left;
        let mut r = right;
        for effect in &mut self.effects {
            (l, r) = effect.process_stereo(l, r);
        }
        (l, r)
    }

    /// Process a block of stereo samples through the chain.
    ///
    /// Output buffers must be at least as large as input buffers.
    pub fn process_block_stereo(
        &mut self,
        left_in: &[f32],
        right_in: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        debug_assert_eq!(left_in.len(), right_in.len());
        debug_assert!(left_out.len() >= left_in.len());
        debug_assert!(right_out.len() >= right_in.len());

        let len = left_in.len();

        if self.effects.is_empty() {
            left_out[..len].copy_from_slice(left_in);
            right_out[..len].copy_from_slice(right_in);
            return;
        }

        // First effect: input -> output
        self.effects[0].process_block_stereo(left_in, right_in, left_out, right_out);

        // Remaining effects: output -> output (in-place)
        for effect in &mut self.effects[1..] {
            effect.process_block_stereo_inplace(&mut left_out[..len], &mut right_out[..len]);
        }
    }

    /// Process a block of stereo samples in-place.
    pub fn process_block_stereo_inplace(&mut self, left: &mut [f32], right: &mut [f32]) {
        for effect in &mut self.effects {
            effect.process_block_stereo_inplace(left, right);
        }
    }

    /// Process an entire stereo file's worth of samples.
    ///
    /// Returns new StereoSamples with processed audio.
    pub fn process_file_stereo(&mut self, input: &StereoSamples, block_size: usize) -> StereoSamples {
        let len = input.len();
        let mut left_out = vec![0.0; len];
        let mut right_out = vec![0.0; len];

        for i in (0..len).step_by(block_size) {
            let chunk_len = (block_size).min(len - i);
            let end = i + chunk_len;

            self.process_block_stereo(
                &input.left[i..end],
                &input.right[i..end],
                &mut left_out[i..end],
                &mut right_out[i..end],
            );
        }

        StereoSamples::new(left_out, right_out)
    }
}

impl Default for ProcessingEngine {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test effect that multiplies by a factor
    struct Gain {
        factor: f32,
    }

    impl Effect for Gain {
        fn process(&mut self, input: f32) -> f32 {
            input * self.factor
        }

        fn set_sample_rate(&mut self, _sample_rate: f32) {}
        fn reset(&mut self) {}
        fn latency_samples(&self) -> usize {
            0
        }
    }

    #[test]
    fn test_empty_engine() {
        let mut engine = ProcessingEngine::new(48000.0);
        assert!(engine.is_empty());
        assert_eq!(engine.process(1.0), 1.0);

        let input = [1.0, 2.0, 3.0];
        let mut output = [0.0; 3];
        engine.process_block(&input, &mut output);
        assert_eq!(output, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_single_effect() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 2.0 }));

        assert_eq!(engine.process(1.0), 2.0);
        assert_eq!(engine.process(0.5), 1.0);
    }

    #[test]
    fn test_effect_chain() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 2.0 }));
        engine.add_effect(Box::new(Gain { factor: 3.0 }));

        // 1.0 * 2.0 * 3.0 = 6.0
        assert_eq!(engine.process(1.0), 6.0);
    }

    #[test]
    fn test_process_block() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 2.0 }));

        let input = [1.0, 2.0, 3.0, 4.0];
        let mut output = [0.0; 4];
        engine.process_block(&input, &mut output);

        assert_eq!(output, [2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn test_process_file() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 0.5 }));

        let input: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let output = engine.process_file(&input, 64);

        assert_eq!(output.len(), input.len());
        for (i, &sample) in output.iter().enumerate() {
            assert_eq!(sample, i as f32 * 0.5);
        }
    }

    #[test]
    fn test_process_stereo() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 2.0 }));

        let (l, r) = engine.process_stereo(1.0, 0.5);
        assert_eq!(l, 2.0);
        assert_eq!(r, 1.0);
    }

    #[test]
    fn test_process_block_stereo() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 2.0 }));

        let left_in = [1.0, 2.0, 3.0];
        let right_in = [0.5, 1.0, 1.5];
        let mut left_out = [0.0; 3];
        let mut right_out = [0.0; 3];

        engine.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

        assert_eq!(left_out, [2.0, 4.0, 6.0]);
        assert_eq!(right_out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_process_file_stereo() {
        let mut engine = ProcessingEngine::new(48000.0);
        engine.add_effect(Box::new(Gain { factor: 0.5 }));

        let left: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let right: Vec<f32> = (0..1000).map(|i| (i as f32) * 2.0).collect();
        let input = StereoSamples::new(left.clone(), right.clone());

        let output = engine.process_file_stereo(&input, 64);

        assert_eq!(output.len(), input.len());
        for i in 0..output.len() {
            assert_eq!(output.left[i], left[i] * 0.5);
            assert_eq!(output.right[i], right[i] * 0.5);
        }
    }

    #[test]
    fn test_empty_engine_stereo() {
        let mut engine = ProcessingEngine::new(48000.0);

        let (l, r) = engine.process_stereo(1.0, 2.0);
        assert_eq!(l, 1.0);
        assert_eq!(r, 2.0);

        let left_in = [1.0, 2.0];
        let right_in = [3.0, 4.0];
        let mut left_out = [0.0; 2];
        let mut right_out = [0.0; 2];

        engine.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);
        assert_eq!(left_out, [1.0, 2.0]);
        assert_eq!(right_out, [3.0, 4.0]);
    }
}
