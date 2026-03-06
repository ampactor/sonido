//! Build script for sonido-daisy.
//!
//! Copies the memory.x linker script to the output directory so that
//! the linker can find it.

fn main() {
    println!("cargo:rerun-if-changed=memory.x");
}
