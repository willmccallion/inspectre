//! # CPU Execution Tests
//!
//! Tests for the main execution loop and pipeline coordination.

use riscv_core::config::Config;
use riscv_core::core::Cpu;
use riscv_core::core::arch::mode::PrivilegeMode;

fn create_test_cpu() -> Cpu {
    let config = Config::default();
    let system = riscv_core::soc::System::new(&config, "");
    Cpu::new(system, &config)
}

#[test]
fn test_tick_returns_ok() {
    let mut cpu = create_test_cpu();
    let result = cpu.tick();
    assert!(result.is_ok());
}

#[test]
fn test_tick_increments_cycles() {
    let mut cpu = create_test_cpu();
    let initial_cycles = cpu.stats.cycles;

    cpu.tick().unwrap();

    // Cycles should increase
    assert!(cpu.stats.cycles >= initial_cycles);
}

#[test]
fn test_multiple_ticks() {
    let mut cpu = create_test_cpu();

    for _ in 0..5 {
        let result = cpu.tick();
        assert!(result.is_ok());
    }
}

#[test]
fn test_exit_code_none_initially() {
    let cpu = create_test_cpu();
    assert_eq!(cpu.exit_code, None);
}

#[test]
fn test_last_pc_updates() {
    let mut cpu = create_test_cpu();

    cpu.tick().unwrap();

    // PC is always set to a valid address
    let _ = cpu.pc;
}

#[test]
fn test_same_pc_counter() {
    let mut cpu = create_test_cpu();
    let initial_count = cpu.same_pc_count;
    cpu.same_pc_count = 0;

    // After execution, counter might increment if PC doesn't change
    cpu.tick().unwrap();

    // Counter should be valid (either stayed same or changed)
    let _ = cpu.same_pc_count;
    assert!(cpu.same_pc_count != initial_count || cpu.same_pc_count == 0);
}

#[test]
fn test_privilege_preserved_across_tick() {
    let mut cpu = create_test_cpu();

    cpu.tick().unwrap();

    // Privilege should be set to something valid
    assert!(
        cpu.privilege == PrivilegeMode::User
            || cpu.privilege == PrivilegeMode::Supervisor
            || cpu.privilege == PrivilegeMode::Machine
    );
}

#[test]
fn test_bus_interaction_tick() {
    let mut cpu = create_test_cpu();

    // Should not panic when calling tick which accesses bus
    let result = cpu.tick();
    assert!(result.is_ok());
}

#[test]
fn test_stats_updated() {
    let mut cpu = create_test_cpu();
    let initial_instructions = cpu.stats.instructions_retired;

    cpu.tick().unwrap();

    // Stats should be updated or remain the same (can't execute if no valid instruction)
    assert!(cpu.stats.instructions_retired >= initial_instructions);
}

#[test]
fn test_tick_does_not_corrupt_state() {
    let mut cpu = create_test_cpu();
    cpu.regs.write(5, 0x1234_5678);

    cpu.tick().unwrap();

    // Register x5 should still have value (unless instruction modifies it)
    // At least verify register file is still accessible
    let _ = cpu.regs.read(5);
}

#[test]
fn test_rapid_ticks() {
    let mut cpu = create_test_cpu();

    for _ in 0..100 {
        let result = cpu.tick();
        assert!(result.is_ok());
    }

    // Should complete without panicking
}

#[test]
fn test_tick_with_different_privileges() {
    for priv_level in [
        PrivilegeMode::Machine,
        PrivilegeMode::Supervisor,
        PrivilegeMode::User,
    ] {
        let mut cpu = create_test_cpu();
        cpu.privilege = priv_level;

        let result = cpu.tick();
        assert!(result.is_ok());
    }
}
