//! # PLIC Device Tests
//!
//! Tests for the Platform-Level Interrupt Controller device.

use rvsim_core::config::Config;
use rvsim_core::soc::Soc;

#[test]
fn test_plic_name() {
    let config = Config::default();
    let _soc = Soc::new(&config, "");
}

#[test]
fn test_plic_device_integration() {
    let config = Config::default();
    let _soc = Soc::new(&config, "");

    // System should initialize without panicking
}
