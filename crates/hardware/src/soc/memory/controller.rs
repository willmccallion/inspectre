//! Memory controllers: `SimpleController` (fixed latency) and `DramController`
//! (multi-bank, row-buffer-aware with refresh).

/// Trait for memory controller implementations that report access latency in cycles.
///
/// Implementors must be `Send + Sync` for thread-safe use with the bus and Python bindings.
pub trait MemoryController: Send + Sync {
    /// Returns the latency in cycles for an access to `addr` at `current_cycle`.
    fn access_latency(&mut self, addr: u64, current_cycle: u64) -> u64;
}

/// Fixed-latency memory controller; every access takes the same number of cycles.
#[derive(Debug)]
pub struct SimpleController {
    latency: u64,
}

impl SimpleController {
    /// Creates a simple controller with the given fixed latency in cycles.
    pub const fn new(latency: u64) -> Self {
        Self { latency }
    }
}

impl MemoryController for SimpleController {
    fn access_latency(&mut self, _addr: u64, _current_cycle: u64) -> u64 {
        self.latency
    }
}

/// Per-bank state for DRAM row-buffer tracking.
#[derive(Debug)]
struct BankState {
    /// Currently open row in this bank, or `None` if no row is active.
    open_row: Option<u64>,
    /// Cycle at which this bank becomes available (after activation or refresh).
    busy_until: u64,
}

/// Configuration parameters for constructing a [`DramController`].
#[derive(Clone, Copy, Debug)]
pub struct DramConfig {
    /// Column access strobe latency (cycles).
    pub t_cas: u64,
    /// Row access strobe latency (cycles).
    pub t_ras: u64,
    /// Precharge latency (cycles).
    pub t_pre: u64,
    /// Row-to-row delay for different-bank activations (cycles).
    pub t_rrd: u64,
    /// Number of independent DRAM banks.
    pub num_banks: usize,
    /// Size of a DRAM row (page) in bytes. Must be a power of two.
    pub row_size_bytes: usize,
    /// Refresh interval in cycles (0 disables refresh).
    pub t_refi: u64,
    /// Refresh cycle time in cycles.
    pub t_rfc: u64,
}

/// DRAM controller with multi-bank row buffers, tRRD, and refresh modeling.
///
/// Each bank independently tracks its open row and busy state. Refresh
/// periodically marks all banks as unavailable for `t_rfc` cycles.
#[derive(Debug)]
pub struct DramController {
    banks: Vec<BankState>,
    num_banks: usize,
    t_cas: u64,
    t_ras: u64,
    t_pre: u64,
    t_rrd: u64,
    t_refi: u64,
    t_rfc: u64,
    row_mask: u64,
    row_shift: u32,
    /// Cycle of the last bank activation (for tRRD enforcement).
    /// `None` means no activation has occurred yet.
    last_activate_cycle: Option<u64>,
    /// Next cycle at which an auto-refresh fires.
    next_refresh_cycle: u64,
}

impl DramController {
    /// Creates a DRAM controller from a [`DramConfig`].
    pub fn new(cfg: DramConfig) -> Self {
        debug_assert!(
            cfg.row_size_bytes.is_power_of_two(),
            "row_size_bytes must be a power of two"
        );
        debug_assert!(cfg.num_banks > 0, "num_banks must be > 0");

        let row_shift = cfg.row_size_bytes.trailing_zeros();
        let row_mask = !(cfg.row_size_bytes as u64 - 1);

        let mut banks = Vec::with_capacity(cfg.num_banks);
        for _ in 0..cfg.num_banks {
            banks.push(BankState { open_row: None, busy_until: 0 });
        }

        Self {
            banks,
            num_banks: cfg.num_banks,
            t_cas: cfg.t_cas,
            t_ras: cfg.t_ras,
            t_pre: cfg.t_pre,
            t_rrd: cfg.t_rrd,
            t_refi: cfg.t_refi,
            t_rfc: cfg.t_rfc,
            row_mask,
            row_shift,
            last_activate_cycle: None,
            next_refresh_cycle: if cfg.t_refi > 0 { cfg.t_refi } else { u64::MAX },
        }
    }

    /// Determines the bank index for a given address.
    /// `bank = (addr >> row_shift) % num_banks`
    #[inline]
    const fn bank_index(&self, addr: u64) -> usize {
        ((addr >> self.row_shift) as usize) % self.num_banks
    }

    /// Returns the row address for a given physical address.
    #[inline]
    const fn row_addr(&self, addr: u64) -> u64 {
        addr & self.row_mask
    }

    /// Marks all banks busy for `t_rfc` cycles if a refresh is due. Returns
    /// the earliest cycle the caller can proceed.
    fn handle_refresh(&mut self, current_cycle: u64) -> u64 {
        if self.t_refi == 0 {
            return current_cycle;
        }

        let mut effective_cycle = current_cycle;

        while effective_cycle >= self.next_refresh_cycle {
            let refresh_end = self.next_refresh_cycle + self.t_rfc;
            for bank in &mut self.banks {
                if bank.busy_until < refresh_end {
                    bank.busy_until = refresh_end;
                }
                bank.open_row = None;
            }
            self.next_refresh_cycle += self.t_refi;
            if effective_cycle < refresh_end {
                effective_cycle = refresh_end;
            }
        }

        effective_cycle
    }

    /// Enforces tRRD spacing and performs a row activation.
    const fn activate(&mut self, mut ready_cycle: u64) -> u64 {
        if let Some(last_act) = self.last_activate_cycle {
            let earliest_activate = last_act + self.t_rrd;
            if ready_cycle < earliest_activate {
                ready_cycle = earliest_activate;
            }
        }
        self.last_activate_cycle = Some(ready_cycle);
        ready_cycle
    }
}

impl MemoryController for DramController {
    fn access_latency(&mut self, addr: u64, current_cycle: u64) -> u64 {
        let mut ready_cycle = self.handle_refresh(current_cycle);

        let bank_idx = self.bank_index(addr);
        let row = self.row_addr(addr);

        if ready_cycle < self.banks[bank_idx].busy_until {
            ready_cycle = self.banks[bank_idx].busy_until;
        }

        match self.banks[bank_idx].open_row {
            Some(open_row) if open_row == row => {
                self.banks[bank_idx].busy_until = ready_cycle + self.t_cas;
                (ready_cycle - current_cycle) + self.t_cas
            }
            Some(_) => {
                ready_cycle += self.t_pre;
                ready_cycle = self.activate(ready_cycle);
                self.banks[bank_idx].open_row = Some(row);
                self.banks[bank_idx].busy_until = ready_cycle + self.t_ras;
                (ready_cycle - current_cycle) + self.t_ras + self.t_cas
            }
            None => {
                ready_cycle = self.activate(ready_cycle);
                self.banks[bank_idx].open_row = Some(row);
                self.banks[bank_idx].busy_until = ready_cycle + self.t_ras;
                (ready_cycle - current_cycle) + self.t_ras + self.t_cas
            }
        }
    }
}
