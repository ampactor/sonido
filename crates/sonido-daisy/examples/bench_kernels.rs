//! Tier 2: DWT cycle-count benchmarks for all 19 Sonido DSP kernels.
//!
//! Runs each kernel through a 128-sample stereo block and reports cycle counts
//! via defmt/RTT. Compare against the per-block budget of 1,280,000 cycles
//! (480 MHz / 375 blocks-per-second at 48 kHz).
//!
//! # Build & Flash
//!
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example bench_kernels --release -- -O binary bench.bin
//! # Enter bootloader (hold BOOT, tap RESET, release BOOT — LED pulses)
//! dfu-util -a 0 -s 0x90040000:leave -D bench.bin
//! ```
//!
//! # Read output (via defmt RTT + probe-rs or defmt-print)
//!
//! ```text
//! kernel=distortion   cycles=12345  budget=1280000  pct=0.96%
//! kernel=reverb       cycles=98765  budget=1280000  pct=7.72%
//! ...
//! ```

#![no_std]
#![no_main]

extern crate alloc;

use defmt_rtt as _;
use embedded_alloc::LlffHeap as Heap;
use panic_probe as _;

use sonido_core::kernel::DspKernel;

#[global_allocator]
static HEAP: Heap = Heap::empty();

use sonido_daisy::{
    BLOCK_SIZE, CYCLES_PER_BLOCK, SAMPLE_RATE, enable_cycle_counter, measure_cycles,
};

use sonido_effects::kernels::{
    BitcrusherKernel, BitcrusherParams, ChorusKernel, ChorusParams, CompressorKernel,
    CompressorParams, DelayKernel, DelayParams, DistortionKernel, DistortionParams, EqKernel,
    EqParams, FilterKernel, FilterParams, FlangerKernel, FlangerParams, GateKernel, GateParams,
    LimiterKernel, LimiterParams, PhaserKernel, PhaserParams, PreampKernel, PreampParams,
    ReverbKernel, ReverbParams, RingModKernel, RingModParams, StageKernel, StageParams, TapeKernel,
    TapeParams, TremoloKernel, TremoloParams, VibratoKernel, VibratoParams, WahKernel, WahParams,
};

/// Benchmark a single kernel: create, process BLOCK_SIZE stereo samples,
/// report cycle count, then drop (freeing heap allocations for the next kernel).
macro_rules! bench {
    ($name:expr, $kernel:ty, $params:ty) => {{
        let mut k = <$kernel>::new(SAMPLE_RATE);
        let p = <$params>::default();
        let cycles = measure_cycles(|| {
            for _ in 0..BLOCK_SIZE {
                let _ = k.process_stereo(0.5, -0.3, &p);
            }
        });
        report($name, cycles);
        // k and p dropped here — heap memory returned for next kernel
    }};
}

/// Log benchmark result via defmt.
fn report(name: &str, cycles: u32) {
    let pct_x100 = (cycles as u64 * 10000) / CYCLES_PER_BLOCK as u64;
    defmt::info!(
        "kernel={} cycles={} budget={} pct={}.{}%",
        name,
        cycles,
        CYCLES_PER_BLOCK,
        pct_x100 / 100,
        pct_x100 % 100
    );
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    // Initialize heap allocator — kernels use alloc for delay lines, comb buffers.
    // Point directly at D2 SRAM (0x30008000, 256 KB) to avoid .bss in DTCMRAM.
    // Safe during benchmarks: no audio DMA running, this region is unused.
    unsafe {
        HEAP.init(0x3000_8000, 256 * 1024);
    }

    let config = daisy_embassy::default_rcc();
    let _p = embassy_stm32::init(config);

    let mut cp = cortex_m::Peripherals::take().unwrap();
    enable_cycle_counter(&mut cp.DCB, &mut cp.DWT);

    defmt::info!("=== Sonido Kernel Benchmarks ===");
    defmt::info!(
        "sample_rate={} block_size={} budget={} cycles",
        SAMPLE_RATE as u32,
        BLOCK_SIZE,
        CYCLES_PER_BLOCK
    );

    bench!("preamp", PreampKernel, PreampParams);
    bench!("distortion", DistortionKernel, DistortionParams);
    bench!("compressor", CompressorKernel, CompressorParams);
    bench!("gate", GateKernel, GateParams);
    bench!("eq", EqKernel, EqParams);
    bench!("wah", WahKernel, WahParams);
    bench!("chorus", ChorusKernel, ChorusParams);
    bench!("flanger", FlangerKernel, FlangerParams);
    bench!("phaser", PhaserKernel, PhaserParams);
    bench!("tremolo", TremoloKernel, TremoloParams);
    bench!("delay", DelayKernel, DelayParams);
    bench!("filter", FilterKernel, FilterParams);
    bench!("vibrato", VibratoKernel, VibratoParams);
    bench!("tape", TapeKernel, TapeParams);
    bench!("reverb", ReverbKernel, ReverbParams);
    bench!("limiter", LimiterKernel, LimiterParams);
    bench!("bitcrusher", BitcrusherKernel, BitcrusherParams);
    bench!("ringmod", RingModKernel, RingModParams);
    bench!("stage", StageKernel, StageParams);

    defmt::info!("=== Benchmarks complete ===");

    // Halt — benchmarks are one-shot
    loop {
        cortex_m::asm::wfi();
    }
}
