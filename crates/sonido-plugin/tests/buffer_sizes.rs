//! Buffer size robustness tests for all sonido effects.
//!
//! Verifies that every effect processes audio without panics, NaN,
//! or infinity at buffer sizes from 1 to 4096 samples.

use sonido_registry::EffectRegistry;

const BUFFER_SIZES: &[usize] = &[1, 2, 7, 32, 64, 128, 256, 512, 1024, 2048, 4096];
const SAMPLE_RATE: f32 = 48000.0;

fn is_finite_buffer(buf: &[f32]) -> bool {
    buf.iter().all(|s| s.is_finite())
}

#[test]
fn all_effects_buffer_sizes_stereo() {
    let registry = EffectRegistry::new();

    for desc in registry.all_effects() {
        for &size in BUFFER_SIZES {
            let mut effect = registry
                .create(desc.id, SAMPLE_RATE)
                .unwrap_or_else(|| panic!("failed to create {}", desc.id));

            // Generate a simple test signal (sine-ish impulse train).
            let left_in: Vec<f32> = (0..size).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
            let right_in = left_in.clone();
            let mut left_out = vec![0.0f32; size];
            let mut right_out = vec![0.0f32; size];

            effect.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

            assert!(
                is_finite_buffer(&left_out),
                "effect {} produced non-finite left output at buffer size {size}",
                desc.id
            );
            assert!(
                is_finite_buffer(&right_out),
                "effect {} produced non-finite right output at buffer size {size}",
                desc.id
            );
        }
    }
}

#[test]
fn all_effects_buffer_sizes_inplace() {
    let registry = EffectRegistry::new();

    for desc in registry.all_effects() {
        for &size in BUFFER_SIZES {
            let mut effect = registry
                .create(desc.id, SAMPLE_RATE)
                .unwrap_or_else(|| panic!("failed to create {}", desc.id));

            let mut left: Vec<f32> = (0..size).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
            let mut right = left.clone();

            effect.process_block_stereo_inplace(&mut left, &mut right);

            assert!(
                is_finite_buffer(&left),
                "effect {} produced non-finite left output (inplace) at buffer size {size}",
                desc.id
            );
            assert!(
                is_finite_buffer(&right),
                "effect {} produced non-finite right output (inplace) at buffer size {size}",
                desc.id
            );
        }
    }
}

#[test]
fn all_effects_buffer_size_1_repeated() {
    let registry = EffectRegistry::new();

    // Process many single-sample buffers to test state accumulation.
    for desc in registry.all_effects() {
        let mut effect = registry
            .create(desc.id, SAMPLE_RATE)
            .unwrap_or_else(|| panic!("failed to create {}", desc.id));

        for i in 0..1000 {
            let input = [(i as f32 * 0.05).sin() * 0.5];
            let mut left_out = [0.0f32];
            let mut right_out = [0.0f32];

            effect.process_block_stereo(&input, &input, &mut left_out, &mut right_out);

            assert!(
                left_out[0].is_finite() && right_out[0].is_finite(),
                "effect {} produced non-finite output at single-sample iteration {i}",
                desc.id
            );
        }
    }
}
