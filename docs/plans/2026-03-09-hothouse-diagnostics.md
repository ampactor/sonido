# Hothouse Hardware Diagnostics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring the Hothouse DIY pedal platform from "assembled, blinky validated" to fully tested with exhaustive diagnostics covering every control, audio path, and DSP kernel on real hardware.

**Architecture:** Tiered firmware examples in `crates/sonido-daisy/examples/`. Each tier validates one hardware subsystem before building on it. daisy-embassy 0.2 provides the audio interface (SAI + DMA), ADC, and GPIO. All examples use BOOT_SRAM mode — the Electrosmith bootloader copies firmware from QSPI flash (`0x90040000`) to AXI SRAM (`0x24000000`) on each boot.

**Tech Stack:** Rust (no_std, thumbv7em-none-eabihf), Embassy async runtime, daisy-embassy 0.2, embassy-stm32 0.5, sonido-core/sonido-effects kernels, defmt logging, USB CDC ACM serial output.

---

### Task 1: Reconcile block size constant

**Context:** daisy-embassy hardcodes `BLOCK_LENGTH = 32` (32 stereo sample pairs). The callback receives `&[u32]` of length 64 (32 pairs × 2 channels interleaved). Our `sonido-daisy/src/lib.rs` declares `BLOCK_SIZE = 128`. This mismatch must be resolved before any audio example works.

**Files:**
- Modify: `crates/sonido-daisy/src/lib.rs:36-43`

**Step 1: Update BLOCK_SIZE and derived constants**

Change `BLOCK_SIZE` from 128 to 32 to match daisy-embassy's hardcoded `BLOCK_LENGTH`. Update `DMA_BUFFER_SIZE` accordingly. Add a comment explaining the constraint.

```rust
/// Default block size in samples per channel.
///
/// daisy-embassy hardcodes `BLOCK_LENGTH = 32` (32 stereo pairs per callback).
/// Each callback receives 64 interleaved u32 values: `[L0, R0, L1, R1, ...]`.
/// This constant matches that — do NOT change without updating daisy-embassy.
pub const BLOCK_SIZE: usize = 32;

/// Number of audio channels (stereo).
pub const CHANNELS: usize = 2;

/// DMA buffer size: block_size * channels * 2 (double-buffer).
pub const DMA_BUFFER_SIZE: usize = BLOCK_SIZE * CHANNELS * 2;
```

**Step 2: Update CYCLES_PER_BLOCK calculation**

With 32-sample blocks at 48 kHz, callback rate = 1500 Hz. At 480 MHz: 480_000_000 / 1500 = 320,000 cycles per block.

```rust
/// Available CPU cycles per audio block at 48 kHz.
///
/// At 480 MHz and 48 kHz with 32-sample blocks:
/// 480_000_000 / (48_000 / 32) = 320_000 cycles per block.
pub const CYCLES_PER_BLOCK: u32 = CPU_CLOCK_HZ / (SAMPLE_RATE as u32 / BLOCK_SIZE as u32);
```

**Step 3: Update AudioCallback type**

The current `AudioCallback` type describes f32 buffers, but daisy-embassy uses interleaved `u32` (24-bit packed). Update the type to reflect reality and add conversion helpers.

```rust
/// Converts a u32 sample (24-bit signed, left-justified in 32 bits) to f32 [-1.0, 1.0].
///
/// The PCM3060 codec outputs 24-bit signed samples packed into 32-bit words.
/// The value is treated as a signed 32-bit integer and divided by 2^31.
#[inline]
pub fn u24_to_f32(sample: u32) -> f32 {
    (sample as i32) as f32 / 2_147_483_648.0
}

/// Converts an f32 sample [-1.0, 1.0] to u32 (24-bit signed, left-justified in 32 bits).
///
/// Inverse of [`u24_to_f32`]. Output is suitable for the PCM3060 codec DAC input.
#[inline]
pub fn f32_to_u24(sample: f32) -> u32 {
    (sample * 2_147_483_648.0) as i32 as u32
}
```

**Step 4: Update bench_kernels.rs to use new BLOCK_SIZE**

The bench macro iterates `BLOCK_SIZE` times (now 32 instead of 128). The benchmark results will reflect per-callback cost. Update the doc comment at the top of the file to explain this.

No code change needed — it already uses `BLOCK_SIZE` constant. Just verify it still compiles:

Run: `cd crates/sonido-daisy && cargo check --example bench_kernels --release`
Expected: compiles successfully

**Step 5: Verify all examples compile**

Run: `cd crates/sonido-daisy && cargo check --examples --release`
Expected: all 4 examples compile

**Step 6: Commit**

```bash
git add crates/sonido-daisy/src/lib.rs
git commit -m "fix(daisy): reconcile BLOCK_SIZE with daisy-embassy (128 → 32)"
```

---

### Task 2: Flash and validate kernel benchmarks

**Context:** `bench_kernels.rs` is fully implemented but has never been flashed. This is Tier 2 validation — proves all 19 kernels run on Cortex-M7 hardware and provides real cycle-count data.

**Files:**
- No code changes needed — just build, flash, and capture output

**Step 1: Build the benchmark binary**

```bash
cd crates/sonido-daisy
cargo objcopy --example bench_kernels --release -- -O binary bench.bin
```

Expected: produces `bench.bin` (should be under 480 KB to fit AXI SRAM)

**Step 2: Check binary size**

```bash
ls -la bench.bin
```

Expected: well under 480 KB (480,000 bytes). If it exceeds this, the bootloader will fail to copy it.

**Step 3: Flash via web flasher**

1. Power cycle Hothouse (unplug, replug power supply)
2. Enter DFU mode: press/release RESET, then hold BOOT until LED pulses
3. Open [flash.daisy.audio](https://flash.daisy.audio/) in Chrome
4. Connect → select DFU device → upload `bench.bin` → Flash

Expected: "Flash successful" message in web flasher

**Step 4: Read benchmark results via USB serial**

After flashing, the Daisy resets, runs benchmarks (~1 second), then enumerates as USB serial:

```bash
# Wait 3 seconds for USB enumeration
cat /dev/ttyACM0
```

Expected output (format — actual numbers will differ):
```
=== Sonido Kernel Benchmarks ===
sample_rate=48000 block_size=32 budget=320000 cycles
       preamp     XXXXX cycles  X.XX%
   distortion     XXXXX cycles  X.XX%
   ...
=== End ===
```

**Step 5: Save benchmark results**

Copy the output to `docs/BENCHMARKS_HARDWARE.md` as the first real hardware measurement baseline.

**Step 6: Validate critical budgets**

Check that no single kernel exceeds 100% of the cycle budget (320,000 cycles for 32-sample blocks). For chains, sum the kernels and verify they fit.

**Step 7: Commit benchmark results**

```bash
git add docs/BENCHMARKS_HARDWARE.md
git commit -m "docs: add first real Cortex-M7 kernel benchmark results"
```

---

### Task 3: Implement audio passthrough

**Context:** `passthrough.rs` is a stub with `todo!()`. This is Tier 3 — validates the codec, SAI, DMA, and audio path. Uses daisy-embassy's audio interface API. The callback receives interleaved `u32` samples (24-bit signed packed into 32 bits).

**Files:**
- Modify: `crates/sonido-daisy/examples/passthrough.rs`

**Step 1: Implement audio passthrough**

Replace the stub with a working passthrough using daisy-embassy's audio interface:

```rust
//! Tier 3: Audio passthrough — validate codec and DMA.
//!
//! Copies audio input directly to output with no processing.
//! Validates the full audio path: codec ADC → SAI RX → DMA → CPU → DMA → SAI TX → codec DAC.
//!
//! # Audio Format
//!
//! daisy-embassy delivers 32 stereo pairs per callback as interleaved `u32`:
//! `[L0, R0, L1, R1, ..., L31, R31]` — 64 elements total.
//! Each `u32` is a 24-bit signed sample left-justified in 32 bits.
//!
//! Passthrough = `output.copy_from_slice(input)` — no format conversion needed.
//!
//! # Build & Flash
//!
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example passthrough --release -- -O binary passthrough.bin
//! # Enter bootloader (hold BOOT, tap RESET, release BOOT — LED pulses)
//! # Flash via web flasher (flash.daisy.audio) or:
//! dfu-util -a 0 -s 0x90040000:leave -D passthrough.bin
//! ```
//!
//! # Testing
//!
//! 1. Connect guitar/synth to Hothouse input
//! 2. Connect Hothouse output to amp/interface
//! 3. Play — audio should pass through unmodified
//! 4. Verify: no clicks, pops, level changes, or added noise

#![no_std]
#![no_main]

use daisy_embassy::new_daisy_board;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32 as hal;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = daisy_embassy::default_rcc();
    let p = hal::init(config);
    let board = new_daisy_board!(p);

    defmt::info!("sonido-daisy passthrough starting");

    // Initialize audio interface with default settings (48 kHz, 32-sample blocks)
    let interface = board
        .audio_peripherals
        .prepare_interface(Default::default())
        .await;
    let mut interface = defmt::unwrap!(interface.start_interface().await);

    defmt::info!("audio interface started — passthrough active");

    // Passthrough: copy input samples directly to output.
    // Both slices are 64 interleaved u32 values: [L0, R0, L1, R1, ..., L31, R31]
    defmt::unwrap!(
        interface
            .start_callback(|input, output| {
                output.copy_from_slice(input);
            })
            .await
    );
}
```

**Step 2: Verify it compiles**

Run: `cd crates/sonido-daisy && cargo check --example passthrough --release`
Expected: compiles successfully

**Step 3: Build the binary**

```bash
cd crates/sonido-daisy
cargo objcopy --example passthrough --release -- -O binary passthrough.bin
```

**Step 4: Flash and test**

1. Enter DFU mode on Hothouse
2. Flash `passthrough.bin` via web flasher or dfu-util
3. Connect audio: guitar/synth → Hothouse input, Hothouse output → amp/interface
4. Play and listen

Expected: clean passthrough, no clicks/pops/noise/level changes

**Step 5: Commit**

```bash
git add crates/sonido-daisy/examples/passthrough.rs
git commit -m "feat(daisy): implement audio passthrough (Tier 3)"
```

---

### Task 4: Implement GPIO diagnostics example

**Context:** Before implementing the full single-effect example, we need to validate all Hothouse GPIO: 2 LEDs, 2 footswitches, 3 toggle switches. This standalone diagnostic example cycles through each control and reports state via defmt + LED feedback.

**Files:**
- Create: `crates/sonido-daisy/examples/gpio_diag.rs`

**Step 1: Write GPIO diagnostics firmware**

This example reads all Hothouse GPIO pins and provides visual feedback:
- LEDs alternate on startup (proves both work)
- Footswitch press → corresponding LED toggles
- Toggle switch changes → defmt log output + LED blink pattern

```rust
//! GPIO diagnostics — validate all Hothouse controls.
//!
//! Tests all digital I/O on the Hothouse:
//! - 2 LEDs (D22/PA5, D23/PA4): alternate blink on startup
//! - 2 footswitches (D25/PA0, D26/PD11): press → toggle corresponding LED
//! - 3 toggle switches (3-way, 2 GPIO each): position logged via defmt
//!
//! Results reported via defmt RTT (probe) or visually via LEDs.
//! No audio — GPIO only.
//!
//! # Build & Flash
//!
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example gpio_diag --release -- -O binary gpio_diag.bin
//! dfu-util -a 0 -s 0x90040000:leave -D gpio_diag.bin
//! ```
//!
//! # Test Procedure
//!
//! 1. Flash and observe: LEDs alternate 5x (proves both LEDs work)
//! 2. Press left footswitch → LED 1 toggles
//! 3. Press right footswitch → LED 2 toggles
//! 4. Move each toggle switch through all 3 positions:
//!    - Both LEDs flash = Up position
//!    - LED 1 only = Middle position
//!    - LED 2 only = Down position

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32 as hal;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::Timer;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let config = daisy_embassy::default_rcc();
    let p = hal::init(config);

    defmt::info!("=== Hothouse GPIO Diagnostics ===");

    // --- LEDs ---
    let mut led1 = Output::new(p.PA5, Level::Low, Speed::Low); // D22 / LED_1
    let mut led2 = Output::new(p.PA4, Level::Low, Speed::Low); // D23 / LED_2

    // --- Footswitches (active-low, need pull-up) ---
    let fsw1 = Input::new(p.PA0, Pull::Up);  // D25 / FOOTSWITCH_1
    let fsw2 = Input::new(p.PD11, Pull::Up); // D26 / FOOTSWITCH_2

    // --- Toggle Switches (2 GPIO each, pull-up) ---
    let sw1_up   = Input::new(p.PB4, Pull::Up);  // D9  / SWITCH_1 up
    let sw1_down = Input::new(p.PB5, Pull::Up);  // D10 / SWITCH_1 down
    let sw2_up   = Input::new(p.PG10, Pull::Up); // D7  / SWITCH_2 up
    let sw2_down = Input::new(p.PG11, Pull::Up); // D8  / SWITCH_2 down
    let sw3_up   = Input::new(p.PD2, Pull::Up);  // D5  / SWITCH_3 up
    let sw3_down = Input::new(p.PC12, Pull::Up); // D6  / SWITCH_3 down

    // Startup: alternate LEDs 5 times to prove both work
    defmt::info!("LED test: alternating 5x");
    for _ in 0..5 {
        led1.set_high();
        led2.set_low();
        Timer::after_millis(200).await;
        led1.set_low();
        led2.set_high();
        Timer::after_millis(200).await;
    }
    led1.set_low();
    led2.set_low();
    defmt::info!("LED test complete");

    // Main loop: poll controls, report changes
    let mut prev_fsw1 = true; // pull-up → high when not pressed
    let mut prev_fsw2 = true;
    let mut led1_state = false;
    let mut led2_state = false;
    let mut prev_sw1: u8 = 255; // sentinel
    let mut prev_sw2: u8 = 255;
    let mut prev_sw3: u8 = 255;

    defmt::info!("Entering control polling loop (20ms period)");
    defmt::info!("Press footswitches to toggle LEDs");
    defmt::info!("Move toggles — LED pattern shows position");

    loop {
        // --- Footswitch 1 (active-low) ---
        let fsw1_pressed = fsw1.is_low();
        if fsw1_pressed && prev_fsw1 {
            // Rising edge (was high, now low — button just pressed)
        }
        if !fsw1_pressed && !prev_fsw1 {
            // Falling edge — released. Toggle LED on release for clean behavior.
            led1_state = !led1_state;
            if led1_state { led1.set_high(); } else { led1.set_low(); }
            defmt::info!("FOOTSWITCH_1 released → LED_1 = {}", led1_state);
        }
        prev_fsw1 = !fsw1_pressed; // invert: true = not pressed

        // --- Footswitch 2 (active-low) ---
        let fsw2_pressed = fsw2.is_low();
        if !fsw2_pressed && !prev_fsw2 {
            led2_state = !led2_state;
            if led2_state { led2.set_high(); } else { led2.set_low(); }
            defmt::info!("FOOTSWITCH_2 released → LED_2 = {}", led2_state);
        }
        prev_fsw2 = !fsw2_pressed;

        // --- Toggle switches ---
        let sw1_pos = read_toggle(sw1_up.is_low(), sw1_down.is_low());
        if sw1_pos != prev_sw1 {
            defmt::info!("SWITCH_1 = {}", toggle_name(sw1_pos));
            prev_sw1 = sw1_pos;
            show_toggle_leds(&mut led1, &mut led2, sw1_pos).await;
        }

        let sw2_pos = read_toggle(sw2_up.is_low(), sw2_down.is_low());
        if sw2_pos != prev_sw2 {
            defmt::info!("SWITCH_2 = {}", toggle_name(sw2_pos));
            prev_sw2 = sw2_pos;
            show_toggle_leds(&mut led1, &mut led2, sw2_pos).await;
        }

        let sw3_pos = read_toggle(sw3_up.is_low(), sw3_down.is_low());
        if sw3_pos != prev_sw3 {
            defmt::info!("SWITCH_3 = {}", toggle_name(sw3_pos));
            prev_sw3 = sw3_pos;
            show_toggle_leds(&mut led1, &mut led2, sw3_pos).await;
        }

        Timer::after_millis(20).await; // 50 Hz poll rate — sufficient for debounce
    }
}

/// Read a 3-way toggle from its two GPIO pins.
/// Returns: 0 = Up, 1 = Middle, 2 = Down.
fn read_toggle(up_active: bool, down_active: bool) -> u8 {
    match (up_active, down_active) {
        (true, false)  => 0, // Up
        (false, false) => 1, // Middle
        (false, true)  => 2, // Down
        (true, true)   => 3, // Both — hardware fault
    }
}

fn toggle_name(pos: u8) -> &'static str {
    match pos {
        0 => "UP",
        1 => "MIDDLE",
        2 => "DOWN",
        3 => "FAULT(both)",
        _ => "UNKNOWN",
    }
}

/// Flash LED pattern to indicate toggle position (no probe needed).
async fn show_toggle_leds(led1: &mut Output<'_>, led2: &mut Output<'_>, pos: u8) {
    // Brief flash to indicate position change
    match pos {
        0 => { // Up — both LEDs
            led1.set_high(); led2.set_high();
            Timer::after_millis(150).await;
            led1.set_low(); led2.set_low();
        }
        1 => { // Middle — LED 1 only
            led1.set_high(); led2.set_low();
            Timer::after_millis(150).await;
            led1.set_low();
        }
        2 => { // Down — LED 2 only
            led1.set_low(); led2.set_high();
            Timer::after_millis(150).await;
            led2.set_low();
        }
        _ => { // Fault — rapid alternation
            for _ in 0..5 {
                led1.set_high(); led2.set_low();
                Timer::after_millis(50).await;
                led1.set_low(); led2.set_high();
                Timer::after_millis(50).await;
            }
            led1.set_low(); led2.set_low();
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cd crates/sonido-daisy && cargo check --example gpio_diag --release`
Expected: compiles successfully

**Step 3: Build, flash, and test**

```bash
cd crates/sonido-daisy
cargo objcopy --example gpio_diag --release -- -O binary gpio_diag.bin
```

Flash via web flasher, then follow the test procedure in the doc comment:
1. Watch LEDs alternate 5x on startup
2. Press each footswitch → corresponding LED toggles
3. Move each toggle → LED pattern shows position

**Step 4: Commit**

```bash
git add crates/sonido-daisy/examples/gpio_diag.rs
git commit -m "feat(daisy): GPIO diagnostics for all Hothouse controls (LEDs, footswitches, toggles)"
```

---

### Task 5: Implement ADC diagnostics example

**Context:** Validate all 6 Hothouse knob potentiometers via ADC. Reads all 6 channels, formats readings as 0.0–1.0 values, and outputs via USB serial (CDC ACM) — same pattern as bench_kernels. Also provides LED feedback: LED brightness (via PWM-like toggle) indicates knob 1 position.

**Files:**
- Create: `crates/sonido-daisy/examples/adc_diag.rs`

**Step 1: Write ADC diagnostics firmware**

```rust
//! ADC diagnostics — validate all 6 Hothouse knob potentiometers.
//!
//! Reads ADC channels 0-5, normalizes to 0.0–1.0, outputs readings via
//! USB serial (CDC ACM) every 500ms. LED 1 brightness tracks KNOB_1.
//!
//! # Pin Mapping
//!
//! | Knob | Daisy Pin | STM32 GPIO | ADC Channel |
//! |------|-----------|------------|:-----------:|
//! | KNOB_1 | D16 | PA3  | ADC1_IN3  |
//! | KNOB_2 | D17 | PB1  | ADC1_IN5  |
//! | KNOB_3 | D18 | PA7  | ADC1_IN7  |
//! | KNOB_4 | D19 | PA6  | ADC1_IN3  |
//! | KNOB_5 | D20 | PC1  | ADC1_IN11 |
//! | KNOB_6 | D21 | PC4  | ADC1_IN4  |
//!
//! # Build & Flash
//!
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example adc_diag --release -- -O binary adc_diag.bin
//! dfu-util -a 0 -s 0x90040000:leave -D adc_diag.bin
//! ```
//!
//! # Read Output
//!
//! ```bash
//! cat /dev/ttyACM0
//! ```
//!
//! # Test Procedure
//!
//! 1. Flash and connect USB serial
//! 2. Turn each knob fully CCW → reading should be ~0.00
//! 3. Turn each knob fully CW → reading should be ~1.00
//! 4. Turn each knob to 12 o'clock → reading should be ~0.50
//! 5. Verify all 6 knobs produce independent, smooth readings
//! 6. Watch LED 1 — should brighten/dim as KNOB_1 turns

#![no_std]
#![no_main]

use core::fmt::Write as FmtWrite;

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32 as hal;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usb::Driver;
use embassy_stm32::{bind_interrupts, peripherals, usb};
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use panic_probe as _;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, Driver<'static, peripherals::USB_OTG_FS>>) -> ! {
    device.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let config = daisy_embassy::default_rcc();
    let p = hal::init(config);

    defmt::info!("=== Hothouse ADC Diagnostics ===");

    // LED for visual feedback
    let mut led1 = Output::new(p.PA5, Level::Low, Speed::Low);

    // ADC setup
    let mut adc = Adc::new(p.ADC1);
    adc.set_sample_time(SampleTime::CYCLES32_5);

    // ADC input pins — must be configured as analog
    let mut knob1 = p.PA3;  // D16 / KNOB_1
    let mut knob2 = p.PB1;  // D17 / KNOB_2
    let mut knob3 = p.PA7;  // D18 / KNOB_3
    let mut knob4 = p.PA6;  // D19 / KNOB_4
    let mut knob5 = p.PC1;  // D20 / KNOB_5
    let mut knob6 = p.PC4;  // D21 / KNOB_6

    // --- USB CDC ACM setup (same pattern as bench_kernels) ---
    static mut EP_OUT_BUF: [u8; 256] = [0u8; 256];

    #[allow(static_mut_refs)]
    let driver = Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        unsafe { &mut EP_OUT_BUF },
        hal::usb::Config::default(),
    );

    let mut usb_config = embassy_usb::Config::new(0x1209, 0x0001);
    usb_config.manufacturer = Some("Sonido");
    usb_config.product = Some("ADC Diagnostics");
    usb_config.serial_number = Some("002");

    static mut CONFIG_DESC: [u8; 256] = [0; 256];
    static mut BOS_DESC: [u8; 256] = [0; 256];
    static mut MSOS_DESC: [u8; 256] = [0; 256];
    static mut CONTROL_BUF: [u8; 64] = [0; 64];
    static mut CDC_STATE: Option<State<'static>> = None;

    #[allow(static_mut_refs)]
    unsafe {
        CDC_STATE = Some(State::new());
    }

    #[allow(static_mut_refs)]
    let mut builder = unsafe {
        embassy_usb::Builder::new(
            driver,
            usb_config,
            &mut CONFIG_DESC,
            &mut BOS_DESC,
            &mut MSOS_DESC,
            &mut CONTROL_BUF,
        )
    };

    #[allow(static_mut_refs)]
    let mut class = unsafe { CdcAcmClass::new(&mut builder, CDC_STATE.as_mut().unwrap(), 64) };

    let usb = builder.build();
    spawner.spawn(usb_task(usb)).unwrap();

    defmt::info!("USB serial ready — waiting for connection");

    // --- Main loop: read ADCs, send via USB serial ---
    let adc_max = 65535.0_f32; // 16-bit ADC

    loop {
        class.wait_connection().await;
        defmt::info!("USB serial connected");

        loop {
            // Read all 6 knobs
            let raw = [
                adc.blocking_read(&mut knob1),
                adc.blocking_read(&mut knob2),
                adc.blocking_read(&mut knob3),
                adc.blocking_read(&mut knob4),
                adc.blocking_read(&mut knob5),
                adc.blocking_read(&mut knob6),
            ];

            // Normalize to 0.0–1.0
            let norm: [f32; 6] = core::array::from_fn(|i| raw[i] as f32 / adc_max);

            // LED 1 brightness tracks KNOB_1 (simple on/off threshold)
            if norm[0] > 0.5 { led1.set_high(); } else { led1.set_low(); }

            // Format output
            let mut buf = [0u8; 256];
            let len = {
                struct BufWriter<'a> { buf: &'a mut [u8], pos: usize }
                impl<'a> core::fmt::Write for BufWriter<'a> {
                    fn write_str(&mut self, s: &str) -> core::fmt::Result {
                        let bytes = s.as_bytes();
                        let remaining = self.buf.len() - self.pos;
                        let len = bytes.len().min(remaining);
                        self.buf[self.pos..self.pos + len].copy_from_slice(&bytes[..len]);
                        self.pos += len;
                        Ok(())
                    }
                }
                let mut w = BufWriter { buf: &mut buf, pos: 0 };
                // Use integer math to avoid f32 formatting (no_std)
                for (i, &v) in norm.iter().enumerate() {
                    let pct = (v * 100.0) as u32;
                    let _ = write!(w, "K{}=0.{:02} ", i + 1, pct);
                }
                let _ = write!(w, "\r\n");
                w.pos
            };

            // Send via USB
            for chunk in buf[..len].chunks(64) {
                if class.write_packet(chunk).await.is_err() {
                    break;
                }
            }

            defmt::info!(
                "ADC: K1={} K2={} K3={} K4={} K5={} K6={}",
                raw[0], raw[1], raw[2], raw[3], raw[4], raw[5]
            );

            embassy_time::Timer::after_millis(500).await;
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cd crates/sonido-daisy && cargo check --example adc_diag --release`
Expected: compiles successfully

**Step 3: Build, flash, and test**

```bash
cd crates/sonido-daisy
cargo objcopy --example adc_diag --release -- -O binary adc_diag.bin
```

Flash and follow the test procedure. Key validations:
- Each knob independently produces a full 0.00–1.00 range
- No crosstalk between adjacent channels
- Readings are stable (no jitter beyond ±0.02)
- LED 1 responds to KNOB_1

**Step 4: Commit**

```bash
git add crates/sonido-daisy/examples/adc_diag.rs
git commit -m "feat(daisy): ADC diagnostics for all 6 Hothouse knobs"
```

---

### Task 6: Implement single-effect processing

**Context:** Tier 4 — the first real DSP on hardware. Processes audio through a distortion kernel with KNOB_1 controlling drive via `from_knobs()`. This validates the entire pipeline: ADC → params → kernel → audio callback.

**Files:**
- Modify: `crates/sonido-daisy/examples/single_effect.rs`

**Step 1: Implement single-effect processing**

```rust
//! Tier 4: Single effect processing — first real DSP on hardware.
//!
//! Processes audio through a Distortion kernel with live knob control:
//! - KNOB_1 → Drive (0.0–1.0 mapped to 0–40 dB)
//! - KNOB_2 → Tone
//! - KNOB_3 → Output level
//! - KNOB_4 → Mix (dry/wet)
//!
//! Uses `DistortionParams::from_knobs()` to map 0.0–1.0 ADC readings
//! to parameter ranges — the standard embedded deployment pattern.
//!
//! Toggle switch 1 selects distortion mode:
//! - Up: Overdrive
//! - Middle: Distortion
//! - Down: Fuzz
//!
//! Footswitch 1 = bypass toggle. LED 1 = active (on) / bypassed (off).
//!
//! # Build & Flash
//!
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example single_effect --release -- -O binary single_effect.bin
//! dfu-util -a 0 -s 0x90040000:leave -D single_effect.bin
//! ```
//!
//! # Test Procedure
//!
//! 1. Connect guitar → Hothouse input, Hothouse output → amp
//! 2. Flash and play — should hear distortion
//! 3. Turn KNOB_1: drive should increase/decrease smoothly
//! 4. Turn KNOB_4: mix should blend dry/wet
//! 5. Press FOOTSWITCH_1: bypass toggle (LED 1 shows state)
//! 6. Move TOGGLE_1: distortion character changes

#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use daisy_embassy::new_daisy_board;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32 as hal;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::Timer;
use embedded_alloc::LlffHeap as Heap;
use panic_probe as _;

use sonido_core::kernel::DspKernel;
use sonido_daisy::{SAMPLE_RATE, f32_to_u24, u24_to_f32};
use sonido_effects::kernels::{DistortionKernel, DistortionParams};

#[global_allocator]
static HEAP: Heap = Heap::empty();

// Shared state between control-polling task and audio callback.
// Audio callback reads these atomically; control task writes them.
static KNOB_1: AtomicU16 = AtomicU16::new(0);
static KNOB_2: AtomicU16 = AtomicU16::new(0);
static KNOB_3: AtomicU16 = AtomicU16::new(32768); // mid = reasonable output
static KNOB_4: AtomicU16 = AtomicU16::new(65535); // full wet
static BYPASSED: AtomicBool = AtomicBool::new(false);
static DIST_MODE: AtomicU16 = AtomicU16::new(1); // 0=overdrive, 1=distortion, 2=fuzz

#[embassy_executor::task]
async fn control_task(
    mut adc: Adc<'static, hal::peripherals::ADC1>,
    mut knob1_pin: hal::peripherals::PA3,
    mut knob2_pin: hal::peripherals::PB1,
    mut knob3_pin: hal::peripherals::PA6,
    mut knob4_pin: hal::peripherals::PC1,
    fsw1: Input<'static>,
    sw1_up: Input<'static>,
    sw1_down: Input<'static>,
    mut led1: Output<'static>,
) {
    let mut prev_fsw1 = true;
    let mut bypassed = false;
    led1.set_high(); // start active (not bypassed)

    loop {
        // Read knobs
        KNOB_1.store(adc.blocking_read(&mut knob1_pin), Ordering::Relaxed);
        KNOB_2.store(adc.blocking_read(&mut knob2_pin), Ordering::Relaxed);
        KNOB_3.store(adc.blocking_read(&mut knob3_pin), Ordering::Relaxed);
        KNOB_4.store(adc.blocking_read(&mut knob4_pin), Ordering::Relaxed);

        // Footswitch 1 — bypass toggle on release
        let fsw1_not_pressed = fsw1.is_high();
        if fsw1_not_pressed && !prev_fsw1 {
            bypassed = !bypassed;
            BYPASSED.store(bypassed, Ordering::Relaxed);
            if bypassed { led1.set_low(); } else { led1.set_high(); }
            defmt::info!("bypass = {}", bypassed);
        }
        prev_fsw1 = fsw1_not_pressed;

        // Toggle switch 1 — distortion mode
        let mode = match (sw1_up.is_low(), sw1_down.is_low()) {
            (true, false) => 0,  // Up = Overdrive
            (false, false) => 1, // Middle = Distortion
            (false, true) => 2,  // Down = Fuzz
            _ => 1,              // Fault → default
        };
        DIST_MODE.store(mode, Ordering::Relaxed);

        Timer::after_millis(10).await; // 100 Hz control rate
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize heap in D2 SRAM
    unsafe { HEAP.init(0x3000_8000, 256 * 1024); }

    let config = daisy_embassy::default_rcc();
    let p = hal::init(config);
    let board = new_daisy_board!(p);

    defmt::info!("=== Sonido Single Effect (Distortion) ===");

    // GPIO setup
    let led1 = Output::new(p.PA5, Level::High, Speed::Low);
    let fsw1 = Input::new(p.PA0, Pull::Up);
    let sw1_up = Input::new(p.PB4, Pull::Up);
    let sw1_down = Input::new(p.PB5, Pull::Up);

    // ADC setup
    let mut adc = Adc::new(p.ADC1);
    adc.set_sample_time(SampleTime::CYCLES32_5);

    // Spawn control polling task
    spawner.spawn(control_task(
        adc,
        p.PA3, p.PB1, p.PA6, p.PC1,
        fsw1, sw1_up, sw1_down, led1,
    )).unwrap();

    // Create distortion kernel
    let mut kernel = DistortionKernel::new(SAMPLE_RATE);
    let adc_max = 65535.0_f32;

    // Start audio interface
    let interface = board
        .audio_peripherals
        .prepare_interface(Default::default())
        .await;
    let mut interface = defmt::unwrap!(interface.start_interface().await);

    defmt::info!("audio started — play your guitar!");

    defmt::unwrap!(
        interface
            .start_callback(move |input, output| {
                let bypassed = BYPASSED.load(Ordering::Relaxed);

                if bypassed {
                    output.copy_from_slice(input);
                    return;
                }

                // Read knob values and build params
                let k1 = KNOB_1.load(Ordering::Relaxed) as f32 / adc_max;
                let k2 = KNOB_2.load(Ordering::Relaxed) as f32 / adc_max;
                let k3 = KNOB_3.load(Ordering::Relaxed) as f32 / adc_max;
                let k4 = KNOB_4.load(Ordering::Relaxed) as f32 / adc_max;

                let mut params = DistortionParams::from_knobs(k1, k2, k3, k4);

                // Set distortion mode from toggle
                let mode = DIST_MODE.load(Ordering::Relaxed);
                params.set(0, mode as f32); // param 0 = mode

                // Process each stereo pair
                // Input format: interleaved u32 [L0, R0, L1, R1, ...]
                for i in 0..(input.len() / 2) {
                    let left_in = u24_to_f32(input[i * 2]);
                    let right_in = u24_to_f32(input[i * 2 + 1]);

                    let (left_out, right_out) = kernel.process_stereo(left_in, right_in, &params);

                    output[i * 2] = f32_to_u24(left_out);
                    output[i * 2 + 1] = f32_to_u24(right_out);
                }
            })
            .await
    );
}
```

**Step 2: Verify it compiles**

Run: `cd crates/sonido-daisy && cargo check --example single_effect --release`
Expected: compiles successfully

**Step 3: Build and flash**

```bash
cd crates/sonido-daisy
cargo objcopy --example single_effect --release -- -O binary single_effect.bin
```

Flash via web flasher.

**Step 4: Test with guitar**

1. Connect guitar → Hothouse input, Hothouse output → amp
2. Play — should hear distortion
3. Turn KNOB_1 fully CCW → clean(ish), fully CW → heavy distortion
4. Turn KNOB_4 → dry/wet blend
5. Press FOOTSWITCH_1 → bypass (LED off), press again → active (LED on)
6. Move TOGGLE_1 → distortion character changes (overdrive/distortion/fuzz)

**Step 5: Commit**

```bash
git add crates/sonido-daisy/examples/single_effect.rs
git commit -m "feat(daisy): single-effect processing with live knob control (Tier 4)"
```

---

### Task 7: Implement full Hothouse diagnostics (combined)

**Context:** A comprehensive diagnostics firmware that validates everything simultaneously: audio passthrough with optional effect, all 6 ADCs, all GPIO, USB serial output. This is the "master test" — if this works, the hardware is fully validated.

**Files:**
- Create: `crates/sonido-daisy/examples/hothouse_diag.rs`

**Step 1: Write comprehensive diagnostics firmware**

This combines audio, ADC, GPIO, and USB serial into one firmware:

- **Audio**: passthrough by default, distortion when FOOTSWITCH_1 active
- **ADC**: all 6 knobs read continuously, reported via USB serial
- **GPIO**: both LEDs, both footswitches, all 3 toggles
- **USB serial**: formatted report every 500ms with all readings
- **defmt**: full logging for probe users

The USB serial output should look like:

```
=== Hothouse Diagnostics ===
K1=0.50 K2=0.73 K3=0.00 K4=1.00 K5=0.50 K6=0.25
SW1=MID SW2=UP SW3=DOWN  FSW1=off FSW2=off
MODE=passthrough  AUDIO=ok
```

This is a larger example that reuses patterns from Tasks 3-5. Full code implementation should combine:
- Audio callback from Task 3 (passthrough) / Task 6 (effect processing)
- ADC reading from Task 5
- GPIO reading from Task 4
- USB serial output from Task 2/5
- Control task pattern from Task 6

**Step 2: Build and flash**

```bash
cd crates/sonido-daisy
cargo objcopy --example hothouse_diag --release -- -O binary hothouse_diag.bin
```

**Step 3: Full test procedure**

Run every test from Tasks 2-6:
1. LEDs alternate on startup (both work)
2. USB serial shows all 6 knob readings
3. Turn each knob through full range — verify 0.00–1.00
4. Move each toggle — USB serial updates position
5. Press each footswitch — USB serial updates state
6. Connect audio: guitar → Hothouse → amp
7. Audio passthrough working (no clicks/pops)
8. FOOTSWITCH_1 toggles distortion on/off
9. Knobs control distortion parameters when active
10. TOGGLE_1 switches distortion mode

**Step 4: Commit**

```bash
git add crates/sonido-daisy/examples/hothouse_diag.rs
git commit -m "feat(daisy): comprehensive Hothouse diagnostics (audio + ADC + GPIO + USB serial)"
```

---

### Task 8: Update documentation

**Context:** All diagnostics examples are implemented and tested. Update EMBEDDED.md, the tier table, and CHANGELOG.md.

**Files:**
- Modify: `docs/EMBEDDED.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `CLAUDE.md` (Key Files table)

**Step 1: Update EMBEDDED.md tier table**

Add the new examples and remove "(stub)" markers:

```markdown
| Tier | Example | What It Validates | Hardware Needed |
|:----:|---------|-------------------|-----------------|
| 1 | `blinky_bare.rs` | Toolchain, flash, BOOT_SRAM path | Seed + USB |
| 1 | `blinky.rs` | Embassy runtime + clock init | Seed + USB |
| 2 | `bench_kernels.rs` | DWT cycle counts for all 19 kernels | Seed + USB |
| 3 | `passthrough.rs` | Codec, DMA, audio path | Hothouse |
| 3 | `gpio_diag.rs` | LEDs, footswitches, toggle switches | Hothouse |
| 3 | `adc_diag.rs` | All 6 ADC knob channels | Hothouse |
| 4 | `single_effect.rs` | Real-time DSP with live knob control | Hothouse + guitar |
| 4 | `hothouse_diag.rs` | Full hardware validation (audio + controls + USB) | Hothouse + guitar |
```

**Step 2: Update Phase 3 and Phase 4 sections**

Remove "stub" language. Add build/flash/test instructions matching the doc comments in each example.

**Step 3: Add block size note**

Document that `BLOCK_SIZE = 32` matches daisy-embassy's hardcoded `BLOCK_LENGTH`, and why.

**Step 4: Update CHANGELOG.md**

Add entry for the Hothouse diagnostics implementation.

**Step 5: Update CLAUDE.md Key Files table**

Add rows for the new example files:

```markdown
| Daisy GPIO diagnostics | crates/sonido-daisy/examples/gpio_diag.rs |
| Daisy ADC diagnostics | crates/sonido-daisy/examples/adc_diag.rs |
| Daisy Hothouse diagnostics | crates/sonido-daisy/examples/hothouse_diag.rs |
```

**Step 6: Commit**

```bash
git add docs/EMBEDDED.md docs/CHANGELOG.md CLAUDE.md
git commit -m "docs: update embedded guide for Hothouse diagnostics (Tiers 2-4)"
```

---

### Task 9: Cross-compile verification

**Context:** Final verification — ensure all examples compile cleanly for the ARM target, clippy is clean, and doc tests pass.

**Step 1: Check all examples compile for ARM**

```bash
cd crates/sonido-daisy
cargo check --examples --release
```

Expected: all 7 examples compile (blinky_bare, blinky, bench_kernels, passthrough, gpio_diag, adc_diag, single_effect, hothouse_diag)

**Step 2: Run clippy**

```bash
cd crates/sonido-daisy
cargo clippy --examples --release -- -W clippy::all
```

Expected: no warnings (or only known allowances)

**Step 3: Verify binary sizes fit BOOT_SRAM (480 KB)**

```bash
cd crates/sonido-daisy
for ex in blinky_bare blinky bench_kernels passthrough gpio_diag adc_diag single_effect hothouse_diag; do
    cargo objcopy --example $ex --release -- -O binary /tmp/$ex.bin 2>/dev/null
    size=$(stat -c%s /tmp/$ex.bin 2>/dev/null || echo "FAILED")
    echo "$ex: $size bytes ($(( size / 1024 )) KB / 480 KB)"
done
```

Expected: all binaries under 491,520 bytes (480 KB)

**Step 4: Verify workspace tests still pass**

```bash
cargo test -p sonido-core -p sonido-effects --quiet
```

Expected: all tests pass (block size change in lib.rs doesn't affect workspace crates)

---

## Diagnostics Protocol Reference

After all tasks are complete, this is the standard procedure for validating a new Hothouse build:

### Quick Validation (~5 min)

1. **Power on** — Daisy LED should pulse (bootloader grace period)
2. **Flash `blinky.bin`** — LED blinks 1 Hz = hardware alive
3. **Flash `gpio_diag.bin`** — LEDs alternate, footswitches toggle, toggles report
4. **Flash `passthrough.bin`** — audio passes through clean

### Full Validation (~20 min)

1. Run Quick Validation above
2. **Flash `bench_kernels.bin`** — `cat /dev/ttyACM0` shows all 19 kernels with cycle counts
3. **Flash `adc_diag.bin`** — `cat /dev/ttyACM0` shows 6 knobs, turn each through full range
4. **Flash `single_effect.bin`** — guitar through distortion, knobs responsive, bypass works
5. **Flash `hothouse_diag.bin`** — all controls, audio, and USB serial working simultaneously

### Troubleshooting

| Symptom | Check |
|---------|-------|
| No LED on power-on | Header seating (press firmly), cable (data vs charge-only) |
| Bootloader pulse but no flash | DFU mode not entered, hold BOOT during flash |
| Binary too large | Use `--release`, check binary size < 480 KB |
| No USB serial output | Wait 3s for enumeration, check `dmesg \| tail`, replug USB |
| ADC stuck at 0 or max | Check solder joints on knob pins, verify correct ADC channel |
| Audio clicks/pops | Check block size (must be 32), verify DMA buffer alignment |
| Toggle reads FAULT | Solder bridge between up/down pins, or missing pull-up |
| MCU very hot | Normal at 480 MHz, check for dual power sources (USB + DC) |
