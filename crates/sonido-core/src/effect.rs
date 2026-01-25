//! Core Effect trait and related types.
//!
//! The [`Effect`] trait is the foundation of the DSP framework. All audio
//! effects implement this trait, providing a consistent interface for
//! single-sample and block-based processing.
//!
//! ## Design Decisions
//!
//! - **Mono processing**: Single `f32` input/output for simplicity and
//!   compatibility with guitar pedal-style effects. Stereo effects can
//!   be built by combining two mono instances or via future extensions.
//!
//! - **Object-safe**: The trait is designed to be object-safe, allowing
//!   `dyn Effect` for runtime effect chaining. However, generic/static
//!   dispatch is preferred for maximum performance.
//!
//! - **No allocations**: All methods are designed to be called in real-time
//!   audio contexts with zero heap allocations.

/// Core trait for all audio effects.
///
/// Effects process audio samples, either one at a time or in blocks.
/// The trait is designed to be object-safe while supporting efficient
/// static dispatch when used with generics.
///
/// # Example
///
/// ```rust
/// use sonido_core::Effect;
///
/// struct Gain {
///     gain: f32,
/// }
///
/// impl Effect for Gain {
///     fn process(&mut self, input: f32) -> f32 {
///         input * self.gain
///     }
///
///     fn set_sample_rate(&mut self, _sample_rate: f32) {
///         // Gain doesn't depend on sample rate
///     }
///
///     fn reset(&mut self) {
///         // Gain has no internal state to reset
///     }
/// }
/// ```
pub trait Effect {
    /// Process a single sample.
    ///
    /// This is the core processing function. For effects with internal state
    /// (filters, delays, etc.), this advances the state by one sample.
    ///
    /// # Arguments
    /// * `input` - Input sample, typically in range [-1.0, 1.0]
    ///
    /// # Returns
    /// Processed output sample
    fn process(&mut self, input: f32) -> f32;

    /// Process a block of samples.
    ///
    /// Default implementation calls `process()` for each sample. Effects
    /// may override this for SIMD optimization or more efficient block
    /// processing.
    ///
    /// # Arguments
    /// * `input` - Input sample buffer
    /// * `output` - Output sample buffer (must be same length as input)
    ///
    /// # Panics
    /// Default implementation panics if `input.len() != output.len()`
    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(
            input.len(),
            output.len(),
            "Input and output buffers must have same length"
        );
        for (inp, out) in input.iter().zip(output.iter_mut()) {
            *out = self.process(*inp);
        }
    }

    /// Process a block of samples in-place.
    ///
    /// Convenience method for when input and output are the same buffer.
    /// Default implementation processes each sample in place.
    ///
    /// # Arguments
    /// * `buffer` - Buffer to process in-place
    fn process_block_inplace(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Update the sample rate.
    ///
    /// Called when the sample rate changes. Effects should recalculate
    /// any sample-rate-dependent coefficients (filter coefficients,
    /// delay times in samples, LFO increments, etc.).
    ///
    /// # Arguments
    /// * `sample_rate` - New sample rate in Hz (e.g., 44100.0, 48000.0)
    fn set_sample_rate(&mut self, sample_rate: f32);

    /// Reset internal state.
    ///
    /// Clears all internal state (delay lines, filter history, etc.)
    /// without changing parameters. Called when playback stops/starts
    /// or when the effect is bypassed to prevent artifacts.
    fn reset(&mut self);

    /// Report processing latency in samples.
    ///
    /// Returns the number of samples of latency introduced by this effect.
    /// Used for latency compensation in DAWs. Most effects have zero latency;
    /// lookahead limiters and linear-phase filters are exceptions.
    ///
    /// Default returns 0 (no latency).
    fn latency_samples(&self) -> usize {
        0
    }
}

/// Extension trait for chaining effects.
///
/// Provides a fluent interface for building effect chains with static dispatch.
/// For dynamic chains, use `Vec<Box<dyn Effect>>` instead.
pub trait EffectExt: Effect + Sized {
    /// Chain this effect with another, creating a composite effect.
    ///
    /// The output of `self` feeds into the input of `next`.
    ///
    /// # Example
    /// ```rust,ignore
    /// let chain = distortion.chain(chorus).chain(delay);
    /// ```
    fn chain<E: Effect>(self, next: E) -> Chain<Self, E> {
        Chain {
            first: self,
            second: next,
        }
    }
}

// Blanket implementation for all Effects
impl<T: Effect> EffectExt for T {}

/// Two effects chained in series.
///
/// Created by [`EffectExt::chain`]. The first effect's output feeds
/// into the second effect's input.
pub struct Chain<A, B> {
    first: A,
    second: B,
}

impl<A: Effect, B: Effect> Effect for Chain<A, B> {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let mid = self.first.process(input);
        self.second.process(mid)
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Process first effect into output buffer
        self.first.process_block(input, output);
        // Process second effect in-place
        self.second.process_block_inplace(output);
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.first.set_sample_rate(sample_rate);
        self.second.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.first.reset();
        self.second.reset();
    }

    fn latency_samples(&self) -> usize {
        self.first.latency_samples() + self.second.latency_samples()
    }
}

impl<A, B> Chain<A, B> {
    /// Get a reference to the first effect in the chain.
    pub fn first(&self) -> &A {
        &self.first
    }

    /// Get a mutable reference to the first effect in the chain.
    pub fn first_mut(&mut self) -> &mut A {
        &mut self.first
    }

    /// Get a reference to the second effect in the chain.
    pub fn second(&self) -> &B {
        &self.second
    }

    /// Get a mutable reference to the second effect in the chain.
    pub fn second_mut(&mut self) -> &mut B {
        &mut self.second
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Gain(f32);

    impl Effect for Gain {
        fn process(&mut self, input: f32) -> f32 {
            input * self.0
        }
        fn set_sample_rate(&mut self, _: f32) {}
        fn reset(&mut self) {}
    }

    #[test]
    fn test_chain() {
        let mut chain = Gain(2.0).chain(Gain(3.0));
        assert_eq!(chain.process(1.0), 6.0);
    }

    #[test]
    fn test_chain_block() {
        let mut chain = Gain(2.0).chain(Gain(0.5));
        let input = [1.0, 2.0, 3.0];
        let mut output = [0.0; 3];
        chain.process_block(&input, &mut output);
        assert_eq!(output, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_chain_latency() {
        struct LatentEffect(usize);
        impl Effect for LatentEffect {
            fn process(&mut self, input: f32) -> f32 { input }
            fn set_sample_rate(&mut self, _: f32) {}
            fn reset(&mut self) {}
            fn latency_samples(&self) -> usize { self.0 }
        }

        let chain = LatentEffect(10).chain(LatentEffect(5));
        assert_eq!(chain.latency_samples(), 15);
    }
}
