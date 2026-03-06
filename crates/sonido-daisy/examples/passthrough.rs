//! Tier 3: Audio passthrough — validate codec and DMA.
//!
//! Copies audio input directly to output with no processing.
//! Requires Daisy Seed with audio codec connected.
//!
//! # Implementation needed
//!
//! - Initialize SAI audio interface via `daisy-embassy`
//! - Set up DMA double-buffered audio callback
//! - Copy input samples to output in the callback

#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let config = daisy_embassy::default_rcc();
    let _p = embassy_stm32::init(config);

    defmt::info!("passthrough: not yet implemented");
    todo!("Implement audio passthrough using daisy-embassy audio interface")
}
