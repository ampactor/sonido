//! Tier 4: Single effect processing on hardware.
//!
//! Processes audio through a single Sonido kernel (e.g., Distortion)
//! with parameters mapped from ADC knob readings via `from_knobs()`.
//!
//! # Implementation needed
//!
//! - Audio passthrough (Tier 3) working first
//! - Read ADC values for knob positions
//! - Construct kernel params via `from_knobs()`
//! - Process audio through kernel in DMA callback

#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let config = daisy_embassy::default_rcc();
    let _p = embassy_stm32::init(config);

    defmt::info!("single_effect: not yet implemented");
    todo!("Implement single effect processing with ADC-mapped parameters")
}
