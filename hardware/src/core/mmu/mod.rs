use crate::core::cpu::Csrs;
use crate::core::types::{AccessType, PhysAddr, TranslationResult, Trap, VirtAddr};
use crate::isa::csr;
use crate::system::Bus;

use self::tlb::Tlb;

pub mod tlb;

pub struct Mmu {
    pub dtlb: Tlb,
    pub itlb: Tlb,
}

impl Mmu {
    pub fn new(tlb_size: usize) -> Self {
        Self {
            dtlb: Tlb::new(tlb_size),
            itlb: Tlb::new(tlb_size),
        }
    }

    pub fn translate(
        &mut self,
        vaddr: VirtAddr,
        access: AccessType,
        privilege: u8,
        csrs: &Csrs,
        bus: &mut Bus,
    ) -> TranslationResult {
        let satp = csrs.satp;
        let mode = (satp >> csr::SATP_MODE_SHIFT) & 0xF;

        if privilege == 3 || mode == 0 {
            return TranslationResult::success(PhysAddr::new(vaddr.val()), 0);
        }

        if mode != csr::SATP_MODE_SV39 {
            return TranslationResult::fault(Trap::InstructionAccessFault(vaddr.val()), 0);
        }

        let vpn = vaddr.vpn2() << 18 | vaddr.vpn1() << 9 | vaddr.vpn0();

        let tlb_entry = if access == AccessType::Fetch {
            self.itlb.lookup(vpn)
        } else {
            self.dtlb.lookup(vpn)
        };

        if let Some((ppn, r, w, x, u)) = tlb_entry {
            // Check Permissions
            if access == AccessType::Fetch && !x {
                return TranslationResult::fault(Trap::InstructionPageFault(vaddr.val()), 0);
            }
            if access == AccessType::Write && !w {
                return TranslationResult::fault(Trap::StorePageFault(vaddr.val()), 0);
            }
            if access == AccessType::Read && !r {
                let mxr = (csrs.sstatus & csr::MSTATUS_MXR) != 0;
                if !mxr || !x {
                    return TranslationResult::fault(Trap::LoadPageFault(vaddr.val()), 0);
                }
            }

            if privilege == 0 && !u {
                let fault = match access {
                    AccessType::Fetch => Trap::InstructionPageFault(vaddr.val()),
                    AccessType::Write => Trap::StorePageFault(vaddr.val()),
                    AccessType::Read => Trap::LoadPageFault(vaddr.val()),
                };
                return TranslationResult::fault(fault, 0);
            }

            if privilege == 1 && u {
                let sum = (csrs.sstatus & csr::MSTATUS_SUM) != 0;
                if !sum {
                    let fault = match access {
                        AccessType::Fetch => Trap::InstructionPageFault(vaddr.val()),
                        AccessType::Write => Trap::StorePageFault(vaddr.val()),
                        AccessType::Read => Trap::LoadPageFault(vaddr.val()),
                    };
                    return TranslationResult::fault(fault, 0);
                }
            }

            let paddr = (ppn << 12) | vaddr.page_offset();
            return TranslationResult::success(PhysAddr::new(paddr), 0);
        }

        // Page Table Walk
        let root_ppn = satp & 0xFFF_FFFF_FFFF;
        let mut pt_addr = PhysAddr::new(root_ppn << 12);
        let mut cycles = 0;

        for level in (0..3).rev() {
            let vpn_i = match level {
                2 => vaddr.vpn2(),
                1 => vaddr.vpn1(),
                _ => vaddr.vpn0(),
            };

            let pte_addr = pt_addr.val() + (vpn_i * 8);

            // We bypass CPU cache simulation here for simplicity in this step
            cycles += bus.calculate_transit_time(8);

            let pte = bus.read_u64(pte_addr);

            if (pte & 1) == 0 {
                let fault = match access {
                    AccessType::Fetch => Trap::InstructionPageFault(vaddr.val()),
                    AccessType::Write => Trap::StorePageFault(vaddr.val()),
                    AccessType::Read => Trap::LoadPageFault(vaddr.val()),
                };
                return TranslationResult::fault(fault, cycles);
            }

            let r = (pte >> 1) & 1 != 0;
            let w = (pte >> 2) & 1 != 0;
            let x = (pte >> 3) & 1 != 0;

            if !r && !w && !x {
                // Pointer to next level
                let next_ppn = (pte >> 10) & 0xFFF_FFFF_FFFF;
                pt_addr = PhysAddr::new(next_ppn << 12);
                continue;
            }

            // Leaf found
            if w && !r {
                return TranslationResult::fault(Trap::StorePageFault(vaddr.val()), cycles);
            }

            // A/D Bit Updates
            let a = (pte >> 6) & 1 != 0;
            let d = (pte >> 7) & 1 != 0;
            let mut new_pte = pte;
            let mut update = false;

            if !a {
                new_pte |= 1 << 6;
                update = true;
            }
            if access == AccessType::Write && !d {
                new_pte |= 1 << 7;
                update = true;
            }

            if update {
                bus.write_u64(pte_addr, new_pte);
                cycles += 10;
            }

            let pte_ppn = (pte >> 10) & 0xFFF_FFFF_FFFF;
            let offset_mask = (1 << (12 + 9 * level)) - 1;
            let final_paddr = (pte_ppn << 12) | (vaddr.val() & offset_mask);

            // Refill TLB
            if access == AccessType::Fetch {
                self.itlb.insert(vpn, pte_ppn, new_pte);
            } else {
                self.dtlb.insert(vpn, pte_ppn, new_pte);
            }

            return TranslationResult::success(PhysAddr::new(final_paddr), cycles);
        }

        let fault = match access {
            AccessType::Fetch => Trap::InstructionPageFault(vaddr.val()),
            AccessType::Write => Trap::StorePageFault(vaddr.val()),
            AccessType::Read => Trap::LoadPageFault(vaddr.val()),
        };
        TranslationResult::fault(fault, cycles)
    }
}
