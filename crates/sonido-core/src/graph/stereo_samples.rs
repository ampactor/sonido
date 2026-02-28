//! Stereo audio buffer for file-level processing.
//!
//! [`StereoSamples`] holds a pair of `Vec<f32>` buffers (left/right channels)
//! and provides conversion utilities (mono, interleaved). Used by
//! [`GraphEngine::process_file_stereo`](super::GraphEngine::process_file_stereo)
//! and WAV I/O routines.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A pair of stereo audio buffers (left and right channels).
///
/// This is the standard interchange type for stereo audio data in Sonido.
/// Each channel is a `Vec<f32>` of equal length.
///
/// # Example
///
/// ```rust,ignore
/// use sonido_core::graph::StereoSamples;
///
/// let samples = StereoSamples::new(vec![1.0; 1024], vec![0.5; 1024]);
/// assert_eq!(samples.len(), 1024);
///
/// let mono = samples.to_mono();
/// assert_eq!(mono[0], 0.75); // (1.0 + 0.5) / 2
/// ```
#[derive(Debug, Clone)]
pub struct StereoSamples {
    /// Left channel samples.
    pub left: Vec<f32>,
    /// Right channel samples.
    pub right: Vec<f32>,
}

impl StereoSamples {
    /// Create new stereo samples from left and right channels.
    pub fn new(left: Vec<f32>, right: Vec<f32>) -> Self {
        debug_assert_eq!(left.len(), right.len(), "Channels must have same length");
        Self { left, right }
    }

    /// Create stereo samples from mono by duplicating to both channels.
    pub fn from_mono(mono: Vec<f32>) -> Self {
        Self {
            left: mono.clone(),
            right: mono,
        }
    }

    /// Get the number of samples per channel.
    pub fn len(&self) -> usize {
        self.left.len()
    }

    /// Check if the buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    /// Mix down to mono by averaging channels.
    pub fn to_mono(&self) -> Vec<f32> {
        self.left
            .iter()
            .zip(self.right.iter())
            .map(|(l, r)| (l + r) * 0.5)
            .collect()
    }

    /// Convert to interleaved format (L, R, L, R, ...).
    pub fn to_interleaved(&self) -> Vec<f32> {
        let mut interleaved = Vec::with_capacity(self.left.len() * 2);
        for (l, r) in self.left.iter().zip(self.right.iter()) {
            interleaved.push(*l);
            interleaved.push(*r);
        }
        interleaved
    }

    /// Create from interleaved format (L, R, L, R, ...).
    pub fn from_interleaved(interleaved: &[f32]) -> Self {
        let len = interleaved.len() / 2;
        let mut left = Vec::with_capacity(len);
        let mut right = Vec::with_capacity(len);

        for chunk in interleaved.chunks(2) {
            if chunk.len() == 2 {
                left.push(chunk[0]);
                right.push(chunk[1]);
            }
        }

        Self { left, right }
    }
}
