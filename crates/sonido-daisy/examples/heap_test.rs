//! Minimal heap test — blinky + D2 SRAM heap allocation.
//!
//! If LED blinks: heap works. If no blink: D2 SRAM access is crashing.
//!
//! # Build & Flash
//! ```bash
//! cd crates/sonido-daisy
//! cargo objcopy --example heap_test --release -- -O binary heap_test.bin
//! dfu-util -a 0 -s 0x90040000:leave -D heap_test.bin
//! ```

#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_alloc::LlffHeap as Heap;
use panic_probe as _;

use sonido_daisy::ClockProfile;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Step 1: Enable D2 SRAM clocks
    sonido_daisy::enable_d2_sram();

    // Step 2: Init heap at D2 SRAM
    unsafe {
        HEAP.init(0x3000_8000, 256 * 1024);
    }

    // Step 3: RCC + embassy init
    let config = sonido_daisy::rcc_config(ClockProfile::Performance);
    let p = embassy_stm32::init(config);

    // Step 4: LED
    use embassy_stm32::gpio::{Level, Output, Speed};
    let mut led = Output::new(p.PC7, Level::Low, Speed::Low);

    // Step 5: Test heap allocation
    let v = vec![42u32; 1024]; // 4KB on heap
    defmt::info!("Heap alloc OK, v[0]={}", v[0]);

    // Step 6: Blink to confirm success
    loop {
        led.set_high();
        Timer::after_millis(200).await;
        led.set_low();
        Timer::after_millis(200).await;
    }
}
