//! # PLIC Device Tests
//!
//! Tests for the Platform-Level Interrupt Controller device.

use rvsim_core::config::Config;
use rvsim_core::soc::Soc;

#[test]
fn test_plic_name() {
    let config = Config::default();
    let exit_signal = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
    let _soc = Soc::new(&config, "", &exit_signal);
}

#[test]
fn test_plic_device_integration() {
    let config = Config::default();
    let exit_signal = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX));
    let _soc = Soc::new(&config, "", &exit_signal);

    // System should initialize without panicking
}
