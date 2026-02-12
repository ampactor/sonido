# Daisy Seed Integration Reference

Hardware integration guide for running sonido on the Electrosmith Daisy Seed
(STM32H750 Cortex-M7). Covers the MCU, Rust ecosystem, memory budget, audio
I/O, and the path from sonido's existing no_std crates to a running pedal.

## Hardware Overview

### Daisy Seed Board

| Spec | Value |
|------|-------|
| MCU | STM32H750IBK6 (ARM Cortex-M7, single core) |
| Clock | 480 MHz |
| FPU | Single-precision hardware FPU (no double, no SIMD) |
| On-board SDRAM | 64 MB (IS42S16400J) |
| On-board Flash | 8 MB QSPI (IS25LP064A) |
| Audio Codec | AK4556 (rev 1.0), WM8731 (rev 1.1), PCM3060 (rev 1.2) |
| Audio | 24-bit stereo, up to 96 kHz |
| GPIO | 31 configurable pins (12x 16-bit ADC, 2x 12-bit DAC) |
| USB | Micro-USB (power, flashing, debug, serial) |
| Price | ~$30 |

### STM32H750 Memory Map

| Region | Address | Size | Access | Use for |
|--------|---------|------|--------|---------|
| ITCM | 0x0000_0000 | 64 KB | 0-wait, instruction only | Code hot paths |
| DTCM | 0x2000_0000 | 128 KB | 0-wait, data only | Audio buffers, filter state, hot DSP data |
| AXI SRAM | 0x2400_0000 | 512 KB | 0-1 wait | Delay lines, reverb buffers, heap |
| D2 SRAM1 | 0x3000_0000 | 128 KB | 1-2 wait | DMA buffers (SAI audio) |
| D2 SRAM2 | 0x3002_0000 | 128 KB | 1-2 wait | DMA buffers |
| D2 SRAM3 | 0x3004_0000 | 32 KB | 1-2 wait | Small peripheral buffers |
| D3 SRAM4 | 0x3800_0000 | 64 KB | 1-2 wait | Low-power domain |
| External SDRAM | 0xC000_0000 | 64 MB | 4-8 wait | Long delay lines (>500ms) |

**Total internal SRAM: 1 MB.** Key constraint: DTCM is fastest but only 128 KB.
AXI SRAM (512 KB) is the primary working memory for DSP allocations. External
SDRAM is available for large buffers but access latency is 4-8x worse.

### Audio Path

The Daisy Seed uses the STM32H750's SAI (Serial Audio Interface) peripheral
connected to the on-board codec. Audio flows through DMA double-buffering:

```
Codec ADC → SAI RX → DMA → SRAM buffer (ping)
                              ↓
                     Process buffer (pong) ← CPU
                              ↓
           SAI TX ← DMA ← SRAM buffer (ping)  → Codec DAC
```

- **DMA double-buffer**: While CPU processes one half-buffer, DMA fills/drains
  the other. Callback fires at each half-transfer.
- **Default block size**: 32 samples (zlosynth `daisy` crate), configurable to 64.
  At 48 kHz, 32 samples = 0.67 ms latency.
- **Sample rate**: 48 kHz default, 96 kHz optional (feature flag in `daisy` crate).
- **Format**: 24-bit I2S, processed as f32 internally.

## Rust Ecosystem for Daisy Seed

### Option A: `daisy-embassy` (recommended for new projects)

| | |
|---|---|
| Crate | [`daisy-embassy`](https://crates.io/crates/daisy-embassy) v0.2.3 |
| Repo | https://github.com/daisy-embassy/daisy-embassy |
| Framework | [Embassy](https://embassy.dev/) (async/await) |
| License | MIT |
| Status | Active development (last update: Feb 2026) |

Embassy provides async tasks, timers, and peripheral drivers. `daisy-embassy`
handles SAI + DMA + codec initialization automatically via builder macros.
Audio callback is an async task.

```rust
// Simplified daisy-embassy audio passthrough pattern
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let config = daisy_embassy::default_rcc();
    let p = embassy_stm32::init(config);
    let board = daisy_embassy::DaisySeed::new(/* peripherals */);
    let mut audio = board.audio_interface();

    audio.start(|input, output| {
        // Process audio here — this is where sonido effects run
        for (i, o) in input.iter().zip(output.iter_mut()) {
            *o = effect.process(*i);
        }
    }).await;
}
```

### Option B: `daisy` by Zlosynth (production-proven)

| | |
|---|---|
| Crate | [`daisy`](https://crates.io/crates/daisy) v0.11.0 |
| Repo | https://github.com/zlosynth/daisy |
| Framework | Bare-metal with interrupt handlers (compatible with RTIC) |
| License | MIT |
| Status | Production — ships in commercial Eurorack modules |

Zlosynth's [`Kaseta`](https://github.com/zlosynth/kaseta) is a saturating tape
delay Eurorack module built entirely in Rust on Daisy Patch SM. This proves
the pattern works at commercial quality. Their architecture:

```
kaseta/            (workspace root)
├── dsp/           (no_std DSP crate — pure algorithms, no hardware)
├── firmware/      (Daisy-specific binary — thin wrapper)
└── control/       (parameter mapping, no_std)
```

This maps directly to sonido's structure:
```
sonido-core      → dsp/       (no_std primitives)
sonido-effects   → dsp/       (no_std effects)
sonido-platform  → control/   (hardware abstraction)
new: sonido-daisy → firmware/  (Daisy-specific binary)
```

Supported codec variants: AK4556 (Seed 1.0), WM8731 (Seed 1.1), PCM3060
(Seed 1.2, Patch SM). Default: 48 kHz, 32-sample blocks.

### Foundation Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| `stm32h7xx-hal` | 0.16.0 | Hardware abstraction layer for STM32H7 |
| `cortex-m` | — | Low-level Cortex-M access (NVIC, SCB, MPU) |
| `cortex-m-rt` | — | Runtime (vector table, entry point, linker script) |
| `embedded-hal` | — | Hardware abstraction traits |
| `defmt` | — | Efficient embedded logging (zero-cost when disabled) |
| `probe-rs` | — | Flashing and debugging via SWD/JTAG |

### Build Toolchain

```bash
# Install target
rustup target add thumbv7em-none-eabihf

# Install probe-rs for flashing/debugging
cargo install probe-rs-tools

# Build for Daisy Seed
cargo build --target thumbv7em-none-eabihf --release

# Flash
probe-rs run --chip STM32H750IBKx target/thumbv7em-none-eabihf/release/sonido-daisy
```

## Memory Budget

### Per-Effect Memory Usage

Estimated from sonido source code. Each `InterpolatedDelay` buffer is
`max_delay_samples × 4 bytes` (f32). Struct overhead and SmoothedParams add
~100-200 bytes per effect.

| Effect | Buffer Formula | Memory @ 48 kHz | Notes |
|--------|---------------|-----------------|-------|
| Reverb (stereo) | 8+8 combs + 4+4 allpasses | ~110 KB | Freeverb tunings scaled from 44.1k |
| Reverb (mono) | 8 combs + 4 allpasses | ~55 KB | Half the buffers |
| Delay (2s, stereo) | 96k × 2 × 4B | ~750 KB | **Exceeds AXI SRAM** — needs SDRAM |
| Delay (500ms, stereo) | 24k × 2 × 4B | ~188 KB | Fits in AXI SRAM |
| Delay (300ms, mono) | 14.4k × 4B | ~56 KB | Default delay time |
| Chorus | 960 × 2 × 4B | ~8 KB | 20ms max delay |
| Flanger | ~480 × 2 × 4B | ~4 KB | ~10ms max delay |
| Phaser | 6 allpasses (tiny) | ~1 KB | No delay buffers |
| Distortion | No buffers | <1 KB | Pure waveshaping |
| Compressor | Envelope state | <1 KB | |
| All other effects | Minimal | <1 KB each | Gate, Tremolo, Wah, Preamp, Filter, TapeSat |

### Memory Placement Strategy

```
DTCM (128 KB, 0-wait):
  ├── Audio DMA buffers (256 samples × 2 ch × 4B = 2 KB)
  ├── Stack (8-16 KB)
  ├── SmoothedParam arrays, filter coefficients
  ├── Hot per-sample state (biquad, SVF, envelope follower)
  └── ~100 KB available for small effect state

AXI SRAM (512 KB, 0-1 wait):
  ├── Reverb buffers (~110 KB stereo)
  ├── Chorus delay lines (~8 KB)
  ├── Flanger delay lines (~4 KB)
  ├── Short delay lines (<500ms = <188 KB)
  ├── Heap allocator for dynamic chains
  └── ~200 KB headroom for effect chains

External SDRAM (64 MB, 4-8 wait):
  ├── Long delay lines (>500ms)
  ├── Sampler/looper buffers
  └── Large lookup tables
```

### Chain Configurations That Fit

**Comfortable (total memory < 300 KB, fits in AXI SRAM with headroom):**

| Chain | Memory Est. | CPU Est. (CM7) |
|-------|------------|----------------|
| Preamp → Distortion → Chorus → Delay(300ms) | ~65 KB | ~30% |
| Gate → TapeSat → Flanger → Delay(300ms) | ~62 KB | ~22% |
| Preamp → Wah → Distortion → Chorus | ~10 KB | ~32% |
| Preamp → TapeSat → Tremolo → Delay(300ms) | ~58 KB | ~19% |

**Tight (memory OK, CPU 50-80%):**

| Chain | Memory Est. | CPU Est. (CM7) |
|-------|------------|----------------|
| Preamp → Distortion → Chorus → Delay → Reverb(mono) | ~120 KB | ~78% |
| Compressor → Distortion → Reverb(mono) | ~57 KB | ~77% |

**Needs SDRAM:**

| Chain | Memory Est. | Notes |
|-------|------------|-------|
| Any chain with Delay > 500ms stereo | > 188 KB delay alone | Move delay buffer to SDRAM |
| Reverb(stereo) + Delay(2s) | ~860 KB | SDRAM required |

**Does not fit (CPU budget exceeded):**
- ParametricEq or MultiVibrato (>100% CPU alone)
- Phaser + Reverb (>124% CPU)
- See `docs/BENCHMARKS.md` for full Cortex-M7 cycle estimates

## Hothouse Platform Mapping

The Cleveland Music Co. Hothouse is a DIY pedal enclosure for Daisy Seed.
See `docs/HARDWARE.md` for full pin mapping and control details.

| Hothouse Control | Daisy Pin | sonido Mapping |
|-----------------|-----------|----------------|
| KNOB_1–6 | PIN_21–25, 28 (ADC) | `ControlId::hardware(0x00..0x05)` |
| TOGGLE_1–3 | PIN_5–10 (GPIO) | `ControlId::hardware(0x10..0x12)` |
| FOOTSWITCH_1–2 | PIN_27, 14 (GPIO) | `ControlId::hardware(0x20..0x21)` |
| LED_1–2 | PIN_4, 3 (GPIO) | `ControlId::hardware(0x30..0x31)` |

The `PlatformController` trait (`crates/sonido-platform/src/lib.rs`) maps
directly to Hothouse's physical controls. A Daisy firmware implementation would:

1. Implement `PlatformController` reading ADC/GPIO pins
2. Use `ControlMapper` to route controls to effect parameters
3. Process audio in the DMA callback using sonido's `Effect` trait

### Preset System

27 preset slots via 3× three-way toggles (3^3 = 27 combinations). Each preset
maps 6 knobs to effect parameters. No display needed — LED blink patterns
indicate state.

## Implementation Path

### Phase 1: Audio Passthrough

1. Create `crates/sonido-daisy/` binary crate
2. Add `daisy-embassy` (or zlosynth `daisy`) dependency
3. Initialize board, codec, SAI + DMA
4. Audio callback: copy input → output (verify hardware works)
5. Flash and test with headphones

### Phase 2: Single Effect

1. Import `sonido-effects` (no_std)
2. Instantiate one effect (e.g., `Distortion`) in static memory
3. Process audio through effect in callback
4. Wire one knob to drive parameter via ADC
5. Verify: real-time audio, no glitches, knob responds

### Phase 3: Effect Chain + Controls

1. Implement `PlatformController` for Hothouse pin layout
2. Wire `ControlMapper` to route knobs → effect parameters
3. Build a fixed effect chain (e.g., Preamp → Distortion → Chorus → Delay)
4. Toggle-based preset selection (27 configurations)
5. Footswitch bypass

### Phase 4: Dynamic Chains

1. Allocate effect chain from heap (AXI SRAM)
2. Toggle combinations select different chains
3. Memory-aware chain builder (refuse chains that exceed budget)
4. CPU usage monitoring (DWT cycle counter)

## Critical Constraints

### What sonido already handles

- All core crates compile no_std (`sonido-core`, `sonido-effects`, `sonido-synth`,
  `sonido-registry`, `sonido-platform`)
- `Effect` trait is hardware-agnostic
- `process_block()` and `process_block_stereo()` match DMA callback pattern
- `ParameterInfo` enables runtime parameter discovery for control mapping
- `libm` used everywhere (no `f32::sin()` in no_std crates)

### What needs work

- **no_std Vec allocation**: Effects use `alloc::vec::Vec` for delay buffers.
  On Cortex-M7, need a global allocator pointing at AXI SRAM. The
  [`embedded-alloc`](https://crates.io/crates/embedded-alloc) crate provides this.
- **Memory placement**: Large buffers (reverb, delay) should live in AXI SRAM
  or SDRAM, not DTCM. May need custom allocator or static buffers.
- **Pre-existing no_std test failure**: `cargo test --no-default-features -p sonido-core`
  currently fails with `cannot find type Vec` — must be fixed before firmware.
- **No `process_block()` in core primitives**: Some primitives (Biquad, SVF) only
  have per-sample `process()`. Block processing would improve cache behavior on CM7.
- **SmoothedParam no short-circuit**: When settled, still computes exponential
  smoothing per sample. Optimization target for tight CPU budgets.

## References

- [Electrosmith Daisy Seed](https://electro-smith.com/products/daisy-seed)
- [Daisy Seed Datasheet](https://daisy.nyc3.cdn.digitaloceanspaces.com/products/seed/Daisy_Seed_datasheet_v1-1-5.pdf)
- [STM32H750 Product Page](https://www.st.com/en/microcontrollers-microprocessors/stm32h750-value-line.html)
- [daisy-embassy crate](https://crates.io/crates/daisy-embassy)
- [zlosynth/daisy crate](https://github.com/zlosynth/daisy)
- [zlosynth/kaseta](https://github.com/zlosynth/kaseta) — Production Rust DSP on Daisy
- [libDaisy (C++)](https://github.com/electro-smith/libDaisy)
- [Hothouse Hardware Reference](HARDWARE.md)
- [Sonido Benchmarks & Cortex-M7 Estimates](BENCHMARKS.md)
