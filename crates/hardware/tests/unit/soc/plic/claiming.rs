//! PLIC claim/complete flow tests.

use rvsim_core::soc::devices::Device;
use rvsim_core::soc::devices::plic::Plic;

#[test]
fn plic_claim_with_no_pending_returns_zero() {
    let mut plic = Plic::new(0);
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(0x200000), (0) as u64, 4); // threshold 0
    plic.update_irqs(0);
    plic.check_interrupts();

    let claim = (crate::common::probe::read(&mut plic, rvsim_core::common::PhysAddr::new(0x200004), 4) as u32);
    assert_eq!(claim, 0);
}

#[test]
fn plic_supervisor_context() {
    let mut plic = Plic::new(0);
    // Source 3, priority 4
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(12), (4) as u64, 4);
    // Enable source 3 for context 1 (supervisor)
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(0x2000 + 0x80), (1 << 3) as u64, 4);
    // Threshold for ctx 1 = 0
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(0x200000 + 0x1000), (0) as u64, 4);

    plic.update_irqs(1 << 3);
    let (meip, seip) = plic.check_interrupts();
    assert!(!meip, "Not enabled for machine context");
    assert!(seip, "Should trigger supervisor external interrupt");
}

#[test]
fn plic_tick_reports_interrupt() {
    let mut plic = Plic::new(0);
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(4), (3) as u64, 4);
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(0x2000), (1 << 1) as u64, 4);
    crate::common::probe::write(&mut plic, rvsim_core::common::PhysAddr::new(0x200000), (0) as u64, 4);
    plic.update_irqs(1 << 1);

    assert!(plic.tick(), "tick should return true when interrupt pending");
}

#[test]
fn plic_tick_no_interrupt() {
    let mut plic = Plic::new(0);
    plic.update_irqs(0);
    assert!(!plic.tick());
}
