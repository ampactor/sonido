//! Sonido DSP firmware for Electrosmith Daisy Seed.
//!
//! This crate provides the hardware integration layer between Sonido's `no_std`
//! DSP kernels and the Daisy Seed's STM32H750 Cortex-M7 processor. It includes
//! DWT cycle-counting utilities for kernel benchmarking and audio callback types
//! for DMA double-buffered processing.
//!
//! # Target Hardware
//!
//! - **MCU**: STM32H750IBK6 (ARM Cortex-M7, 480 MHz, single-precision FPU)
//! - **Audio**: 24-bit stereo codec via SAI + DMA, 48 kHz default
//! - **Memory**: 128 KB DTCM (0-wait), 512 KB AXI SRAM, 64 MB SDRAM
//!
//! # Usage Tiers
//!
//! - **Tier 1**: Blinky (validate toolchain + flash)
//! - **Tier 2**: Kernel benchmarks (DWT cycle counts for all 19 effects)
//! - **Tier 3**: Audio passthrough (validate codec + DMA)
//! - **Tier 4**: Single effect processing (first real DSP on hardware)

#![no_std]

use cortex_m::peripheral::DWT;

/// Default audio sample rate in Hz.
pub const SAMPLE_RATE: f32 = 48_000.0;

/// Default block size in samples (matches Daisy Seed DMA half-transfer).
pub const BLOCK_SIZE: usize = 128;

/// Number of audio channels (stereo).
pub const CHANNELS: usize = 2;

/// DMA buffer size: block_size * channels * 2 (double-buffer).
pub const DMA_BUFFER_SIZE: usize = BLOCK_SIZE * CHANNELS * 2;

/// CPU clock frequency in Hz (STM32H750 max).
pub const CPU_CLOCK_HZ: u32 = 480_000_000;

/// Available CPU cycles per audio block at 48 kHz.
///
/// At 480 MHz and 48 kHz with 128-sample blocks:
/// 480_000_000 / (48_000 / 128) = 1_280_000 cycles per block.
pub const CYCLES_PER_BLOCK: u32 = CPU_CLOCK_HZ / (SAMPLE_RATE as u32 / BLOCK_SIZE as u32);

/// Measures the number of CPU cycles consumed by a closure using the DWT cycle counter.
///
/// # Prerequisites
///
/// The DWT cycle counter must be enabled before calling this function.
/// Use [`enable_cycle_counter`] at startup.
///
/// # Example
///
/// ```ignore
/// enable_cycle_counter(&mut cp.DCB, &mut cp.DWT);
/// let cycles = measure_cycles(|| {
///     kernel.process_stereo(0.5, 0.5, &params);
/// });
/// defmt::info!("Kernel used {} cycles", cycles);
/// ```
#[inline]
pub fn measure_cycles<F: FnOnce()>(f: F) -> u32 {
    let start = DWT::cycle_count();
    f();
    let end = DWT::cycle_count();
    end.wrapping_sub(start)
}

/// Enables the DWT cycle counter.
///
/// Must be called once at startup before using [`measure_cycles`].
/// Takes mutable references to the DCB and DWT peripherals from `cortex_m::Peripherals`.
///
/// # Example
///
/// ```ignore
/// let mut cp = cortex_m::Peripherals::take().unwrap();
/// enable_cycle_counter(&mut cp.DCB, &mut cp.DWT);
/// ```
pub fn enable_cycle_counter(dcb: &mut cortex_m::peripheral::DCB, dwt: &mut DWT) {
    dcb.enable_trace();
    dwt.enable_cycle_counter();
}

/// Audio callback signature for DMA double-buffered processing.
///
/// Called once per block (128 samples). Input and output are interleaved stereo:
/// `[L0, R0, L1, R1, ...]`.
pub type AudioCallback = fn(input: &[f32], output: &mut [f32]);
