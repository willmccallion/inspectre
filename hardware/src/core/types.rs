#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);

impl VirtAddr {
    #[inline(always)]
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub fn val(&self) -> u64 {
        self.0
    }

    pub fn vpn0(&self) -> u64 {
        (self.0 >> 12) & 0x1FF
    }

    pub fn vpn1(&self) -> u64 {
        (self.0 >> 21) & 0x1FF
    }

    pub fn vpn2(&self) -> u64 {
        (self.0 >> 30) & 0x1FF
    }

    pub fn page_offset(&self) -> u64 {
        self.0 & 0xFFF
    }
}

impl PhysAddr {
    #[inline(always)]
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    #[inline(always)]
    pub fn val(&self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessType {
    Fetch,
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trap {
    InstructionAddressMisaligned(u64),
    InstructionAccessFault(u64),
    IllegalInstruction(u32),
    Breakpoint(u64),
    LoadAddressMisaligned(u64),
    LoadAccessFault(u64),
    StoreAddressMisaligned(u64),
    StoreAccessFault(u64),
    EnvironmentCallFromUMode,
    EnvironmentCallFromSMode,
    EnvironmentCallFromMMode,
    InstructionPageFault(u64),
    LoadPageFault(u64),
    StorePageFault(u64),
    MachineTimerInterrupt,
    UserSoftwareInterrupt,
    SupervisorSoftwareInterrupt,
    MachineSoftwareInterrupt,
    SupervisorTimerInterrupt,
    ExternalInterrupt,
    RequestedTrap(u64),
}

pub struct TranslationResult {
    pub paddr: PhysAddr,
    pub cycles: u64,
    pub trap: Option<Trap>,
}

impl TranslationResult {
    pub fn success(paddr: PhysAddr, cycles: u64) -> Self {
        Self {
            paddr,
            cycles,
            trap: None,
        }
    }

    pub fn fault(trap: Trap, cycles: u64) -> Self {
        Self {
            paddr: PhysAddr(0),
            cycles,
            trap: Some(trap),
        }
    }
}
