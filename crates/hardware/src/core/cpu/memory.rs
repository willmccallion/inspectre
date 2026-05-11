//! Memory Access Helpers — virtual-to-physical address translation.

use super::Cpu;
use crate::common::{AccessType, PhysAddr, TranslationResult, Trap, VirtAddr};
use crate::core::units::mmu::pmp::PmpResult;

impl Cpu {
    /// Translates a virtual address to a physical address using the MMU.
    pub fn translate(
        &mut self,
        vaddr: VirtAddr,
        access: AccessType,
        size: u64,
    ) -> TranslationResult {
        if self.direct_mode {
            let paddr = PhysAddr::new(vaddr.val());

            // PMP operates on physical addresses independent of virtual memory
            // translation (RISC-V Privileged Spec §3.7).  M-mode with no
            // matching entries gets full access, so this is transparent to
            // programs that do not configure PMP.
            let is_machine = self.hart.privilege == crate::core::arch::mode::PrivilegeMode::Machine;
            let pmp_result = self.hart.pmp.check(
                paddr.val(),
                size,
                matches!(access, AccessType::Read),
                matches!(access, AccessType::Write),
                matches!(access, AccessType::Fetch),
                is_machine,
            );
            if pmp_result != PmpResult::Allow {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(vaddr.val()),
                    AccessType::Read => Trap::LoadAccessFault(vaddr.val()),
                    AccessType::Write => Trap::StoreAccessFault(vaddr.val()),
                };
                return TranslationResult::fault(trap, 0);
            }

            if !self.soc.bus.is_valid_address(paddr) {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(vaddr.val()),
                    AccessType::Read => Trap::LoadAccessFault(vaddr.val()),
                    AccessType::Write => Trap::StoreAccessFault(vaddr.val()),
                };
                return TranslationResult::fault(trap, 0);
            }
            return TranslationResult::success(paddr, 0);
        }

        // MPRV: when set and access is not Fetch, use MPP as effective privilege.
        let effective_priv = if access != AccessType::Fetch
            && (self.hart.csrs.mstatus & crate::core::arch::csr::MSTATUS_MPRV) != 0
        {
            use crate::core::arch::csr::{MSTATUS_MPP_MASK, MSTATUS_MPP_SHIFT};
            use crate::core::arch::mode::PrivilegeMode;
            let mpp = ((self.hart.csrs.mstatus >> MSTATUS_MPP_SHIFT) & MSTATUS_MPP_MASK) as u8;
            PrivilegeMode::from_u8(mpp)
        } else {
            self.hart.privilege
        };

        let result = self.hart.mmu.translate_with_pmp(
            vaddr,
            access,
            effective_priv,
            &self.hart.csrs,
            &mut self.soc.bus,
            Some(&self.hart.pmp),
        );

        // PMP check on the translated physical address.
        // PMP applies to all privilege modes: M-mode with no matching entry gets Allow,
        // S/U-mode with no matching entry gets NoMatch (denied).
        if result.trap.is_none() {
            let paddr = result.paddr.val();
            let is_machine = effective_priv == crate::core::arch::mode::PrivilegeMode::Machine;
            let pmp_result = self.hart.pmp.check(
                paddr,
                size,
                matches!(access, AccessType::Read),
                matches!(access, AccessType::Write),
                matches!(access, AccessType::Fetch),
                is_machine,
            );
            if pmp_result != PmpResult::Allow {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(vaddr.val()),
                    AccessType::Read => Trap::LoadAccessFault(vaddr.val()),
                    AccessType::Write => Trap::StoreAccessFault(vaddr.val()),
                };
                return TranslationResult::fault(trap, result.cycles);
            }

            // Accesses to unmapped physical memory raise an access fault —
            // real hardware reports a bus error that the CPU turns into a
            // load/store/inst access fault. Firmware that probes memory
            // must install a trap handler (same as real hardware).
            if !self.soc.bus.is_valid_address(result.paddr) {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(vaddr.val()),
                    AccessType::Read => Trap::LoadAccessFault(vaddr.val()),
                    AccessType::Write => Trap::StoreAccessFault(vaddr.val()),
                };
                return TranslationResult::fault(trap, result.cycles);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_translate_direct_mode() {
        let mut config = Config::default();
        config.general.direct_mode = true;
        let mut cpu = Cpu::build(&config, "");

        let result = cpu.translate(VirtAddr::new(0x8000_0000), AccessType::Read, 4);
        assert_eq!(result.paddr.val(), 0x8000_0000);
        assert!(result.trap.is_none());

        let result = cpu.translate(VirtAddr::new(0xFFFF_FFFF_FFFF_FFFF), AccessType::Fetch, 4);
        assert!(result.trap.is_some());
    }

    #[test]
    fn test_translate_direct_mode_pmp_deny() {
        use crate::core::arch::mode::PrivilegeMode;

        let mut config = Config::default();
        config.general.direct_mode = true;
        let mut cpu = Cpu::build(&config, "");

        cpu.hart.pmp.set_addr(0, 0x9000_0000u64 >> 2);
        cpu.hart.pmp.set_cfg(0, 0x88);

        cpu.hart.privilege = PrivilegeMode::Supervisor;

        let result = cpu.translate(VirtAddr::new(0x8000_0000), AccessType::Read, 4);
        assert!(result.trap.is_some(), "PMP should deny the access");
    }

    #[test]
    fn test_translate_direct_mode_pmp_allow_mmode() {
        let mut config = Config::default();
        config.general.direct_mode = true;
        let mut cpu = Cpu::build(&config, "");

        let result = cpu.translate(VirtAddr::new(0x8000_0000), AccessType::Read, 4);
        assert!(result.trap.is_none(), "M-mode should have full access with no PMP entries");
        assert_eq!(result.paddr.val(), 0x8000_0000);
    }
}
