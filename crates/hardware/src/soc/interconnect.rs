//! System interconnect (bus) — routes physical accesses to devices, ticks
//! devices, aggregates IRQs through PLIC, and exposes the RAM pointer.

use super::devices::Device;
use super::memory::RamRegion;
use crate::common::PhysAddr;

/// Aggregated interrupt signals returned by [`Bus::tick`] each cycle.
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct BusIrqs {
    /// CLINT machine timer interrupt (`mip.MTIP`).
    pub timer: bool,
    /// CLINT machine software interrupt (`mip.MSIP`).
    pub msip: bool,
    /// PLIC machine external interrupt (`mip.MEIP`).
    pub meip: bool,
    /// PLIC supervisor external interrupt (`mip.SEIP`).
    pub seip: bool,
}

/// System bus connecting CPU and devices; routes accesses by physical address.
///
/// Holds a sorted list of devices (RAM, UART, disk, CLINT, PLIC, etc.), bus width and latency
/// for transfer time calculation, and indices for fast RAM/UART/CLINT lookup.
pub struct Bus {
    /// Registered MMIO and memory devices (boxed for dynamic dispatch; `Send + Sync` for thread safety).
    devices: Vec<Box<dyn Device + Send + Sync>>,
    /// Bus width in bytes (e.g., 8 for 64-bit); used to compute transfer cycles.
    pub width_bytes: u64,
    /// Base latency in cycles per transaction.
    pub latency_cycles: u64,
    last_device_idx: usize,
    ram_idx: Option<usize>,
    uart_idx: Option<usize>,
    htif_idx: Option<usize>,
    clint_idx: Option<usize>,
    /// Cached fast-path view of the DRAM device's backing buffer; populated
    /// when a device named `"DRAM"` is registered. The pipeline reads this
    /// on hot loads/stores to skip device-table dispatch.
    ram_region: Option<RamRegion>,
    /// Cached `(start, end_exclusive)` for the HTIF device. The pipeline
    /// checks this before the RAM fast path so HTIF tohost stores are
    /// always routed through the device.
    htif_range: Option<(u64, u64)>,
}

impl std::fmt::Debug for Bus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bus")
            .field("width_bytes", &self.width_bytes)
            .field("latency_cycles", &self.latency_cycles)
            .field("last_device_idx", &self.last_device_idx)
            .field("ram_idx", &self.ram_idx)
            .field("uart_idx", &self.uart_idx)
            .field("htif_idx", &self.htif_idx)
            .field("clint_idx", &self.clint_idx)
            .field("num_devices", &self.devices.len())
            .finish_non_exhaustive()
    }
}

impl Bus {
    /// Creates a new bus with the given width and latency.
    pub fn new(width_bytes: u64, latency_cycles: u64) -> Self {
        Self {
            devices: Vec::new(),
            width_bytes,
            latency_cycles,
            last_device_idx: 0,
            ram_idx: None,
            uart_idx: None,
            htif_idx: None,
            clint_idx: None,
            ram_region: None,
            htif_range: None,
        }
    }

    /// Registers a device on the bus; devices are sorted by base address for lookup.
    pub fn add_device(&mut self, dev: Box<dyn Device + Send + Sync>) {
        self.devices.push(dev);
        self.devices.sort_by_key(|d| d.address_range().0);
        self.ram_idx = self.devices.iter().position(|d| d.name() == "DRAM");
        self.uart_idx = self.devices.iter().position(|d| d.name() == "UART0");
        self.htif_idx = self.devices.iter().position(|d| d.name() == "HTIF");
        self.clint_idx = self.devices.iter().position(|d| d.name() == "CLINT");
        self.last_device_idx = 0;
        self.refresh_fast_paths();
    }

    /// Recomputes the cached `RamRegion` and HTIF range after a device-set change.
    fn refresh_fast_paths(&mut self) {
        self.ram_region = self.ram_idx.and_then(|idx| {
            let (base, size) = self.devices[idx].address_range();
            self.devices[idx].as_memory_mut().map(|mem| RamRegion::new(mem.as_mut_ptr(), base, size))
        });
        self.htif_range = self.htif_idx.map(|idx| {
            let (start, size) = self.devices[idx].address_range();
            (start, start + size)
        });
    }

    /// Returns the cached fast-path view of the DRAM region, or `None` when
    /// no DRAM device is registered.
    #[inline]
    pub const fn ram_region(&self) -> Option<RamRegion> {
        self.ram_region
    }

    /// Returns the cached `(start, end_exclusive)` HTIF range, or `None`
    /// when no HTIF device is registered.
    #[inline]
    pub const fn htif_range(&self) -> Option<(u64, u64)> {
        self.htif_range
    }

    /// Returns cycles = base latency plus ceiling(bytes / `width_bytes`) transfers.
    pub const fn calculate_transit_time(&self, bytes: usize) -> u64 {
        let transfers = (bytes as u64).div_ceil(self.width_bytes);
        self.latency_cycles + transfers
    }

    /// Writes a binary blob into memory at the given physical address.
    pub fn load_binary_at(&mut self, data: &[u8], addr: PhysAddr) {
        if let Some((dev, offset)) = self.find_device(addr) {
            let (_, size) = dev.address_range();
            if offset + (data.len() as u64) <= size {
                dev.write_bytes(offset, data);
                return;
            }
        }
        for (i, byte) in data.iter().enumerate() {
            self.write_u8(PhysAddr::new(addr.val() + i as u64), *byte);
        }
    }

    /// Returns whether the given physical address is backed by any device.
    pub fn is_valid_address(&self, paddr: PhysAddr) -> bool {
        let raw = paddr.val();
        if let Some(idx) = self.ram_idx {
            let (start, size) = self.devices[idx].address_range();
            if raw >= start && raw < start + size {
                return true;
            }
        }
        for dev in &self.devices {
            let (start, size) = dev.address_range();
            if raw >= start && raw < start + size {
                return true;
            }
        }
        false
    }

    /// Advances all devices by one tick and updates PLIC. Returns the
    /// aggregated interrupt vector for this cycle.
    pub fn tick(&mut self) -> BusIrqs {
        let mut timer = false;
        let mut active_irqs = 0u64;

        for i in 0..self.devices.len() {
            let dev = &mut self.devices[i];
            if dev.tick() {
                if let Some(id) = dev.get_irq_id()
                    && id.val() < 64
                {
                    active_irqs |= 1 << id.val();
                }
                if dev.name() == "CLINT" {
                    timer = true;
                }
            }
        }

        // Query MSIP separately — it is independent of the timer comparison.
        let msip = self
            .clint_idx
            .and_then(|idx| self.devices[idx].as_clint_mut())
            .is_some_and(|clint| clint.msip_pending());

        let (meip, seip) = self.find_plic().map_or((false, false), |plic| {
            plic.update_irqs(active_irqs);
            plic.check_interrupts()
        });

        BusIrqs { timer, msip, meip, seip }
    }

    /// Returns whether the UART device has detected a kernel panic pattern.
    pub fn check_kernel_panic(&mut self) -> bool {
        if let Some(idx) = self.uart_idx
            && idx < self.devices.len()
            && let Some(uart) = self.devices[idx].as_uart_mut()
        {
            return uart.check_kernel_panic();
        }
        false
    }

    /// Returns `(ptr, base, end)` for the RAM region, or `None` if not present.
    pub fn get_ram_info(&mut self) -> Option<(*mut u8, u64, u64)> {
        if let Some(idx) = self.ram_idx
            && let Some(mem) = self.devices[idx].as_memory_mut()
        {
            let (base, size) = mem.address_range();
            return Some((mem.as_mut_ptr(), base, base + size));
        }
        None
    }

    fn find_plic(&mut self) -> Option<&mut crate::soc::devices::Plic> {
        for dev in &mut self.devices {
            if let Some(plic) = dev.as_plic_mut() {
                return Some(plic);
            }
        }
        None
    }

    fn find_device(
        &mut self,
        paddr: PhysAddr,
    ) -> Option<(&mut Box<dyn Device + Send + Sync>, u64)> {
        let raw = paddr.val();
        // HTIF sits inside the RAM range so must be checked before any RAM
        // fast-path (last_device_idx cache or ram_idx shortcut).
        if let Some(idx) = self.htif_idx {
            let (start, size) = self.devices[idx].address_range();
            if raw >= start && raw < start + size {
                self.last_device_idx = idx;
                return Some((&mut self.devices[idx], raw - start));
            }
        }

        if self.last_device_idx < self.devices.len() {
            let (start, size) = self.devices[self.last_device_idx].address_range();
            if raw >= start && raw < start + size {
                return Some((&mut self.devices[self.last_device_idx], raw - start));
            }
        }

        if let Some(idx) = self.ram_idx {
            let (start, size) = self.devices[idx].address_range();
            if raw >= start && raw < start + size {
                self.last_device_idx = idx;
                return Some((&mut self.devices[idx], raw - start));
            }
        }

        for (i, dev) in self.devices.iter_mut().enumerate() {
            let (start, size) = dev.address_range();
            if raw >= start && raw < start + size {
                self.last_device_idx = i;
                return Some((dev, raw - start));
            }
        }
        None
    }

    /// Reads one byte at the given physical address; returns 0 if no device claims the address.
    pub fn read_u8(&mut self, paddr: PhysAddr) -> u8 {
        if let Some((dev, offset)) = self.find_device(paddr) { dev.read_u8(offset) } else { 0 }
    }
    /// Reads two bytes (little-endian) at the given physical address; returns 0 if unclaimed.
    pub fn read_u16(&mut self, paddr: PhysAddr) -> u16 {
        if let Some((dev, offset)) = self.find_device(paddr) { dev.read_u16(offset) } else { 0 }
    }
    /// Reads four bytes (little-endian) at the given physical address; returns 0 if unclaimed.
    pub fn read_u32(&mut self, paddr: PhysAddr) -> u32 {
        if let Some((dev, offset)) = self.find_device(paddr) { dev.read_u32(offset) } else { 0 }
    }
    /// Reads eight bytes (little-endian) at the given physical address; returns 0 if unclaimed.
    pub fn read_u64(&mut self, paddr: PhysAddr) -> u64 {
        if let Some((dev, offset)) = self.find_device(paddr) { dev.read_u64(offset) } else { 0 }
    }
    /// Writes one byte at the given physical address; no-op if no device claims it.
    pub fn write_u8(&mut self, paddr: PhysAddr, val: u8) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u8(offset, val);
        }
    }
    /// Writes two bytes (little-endian) at the given physical address; no-op if unclaimed.
    pub fn write_u16(&mut self, paddr: PhysAddr, val: u16) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u16(offset, val);
        }
    }
    /// Writes four bytes (little-endian) at the given physical address; no-op if unclaimed.
    pub fn write_u32(&mut self, paddr: PhysAddr, val: u32) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u32(offset, val);
        }
    }
    /// Writes eight bytes (little-endian) at the given physical address; no-op if unclaimed.
    pub fn write_u64(&mut self, paddr: PhysAddr, val: u64) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u64(offset, val);
        }
    }
}
