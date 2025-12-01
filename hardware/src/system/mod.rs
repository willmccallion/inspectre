pub mod bus;
pub mod devices;
pub mod memory;

pub use self::bus::Bus;

use self::devices::{Clint, Uart, VirtualDisk};
use self::memory::Memory;
use self::memory::controller::{DramController, MemoryController, SimpleController};
use crate::config::Config;
use crate::sim::loader::load_binary;

pub struct System {
    pub bus: Bus,
    pub mem_controller: Box<dyn MemoryController>,
}

impl System {
    pub fn new(config: &Config, disk_path: &str) -> Self {
        let mut bus = Bus::new(config.system.bus_width, config.system.bus_latency);

        let ram_base = config.system.ram_base_val();
        let ram_size = config.memory.ram_size_val();
        let mem = Memory::new(ram_size, ram_base);

        let uart_base = config.system.uart_base_val();
        let uart = Uart::new(uart_base);

        let clint_addr = config.system.clint_base_val();
        let clint = Clint::new(clint_addr, config.system.clint_divider);

        let disk_base = config.system.disk_base_val();
        let mut disk = VirtualDisk::new(disk_base);
        if !disk_path.is_empty() {
            let disk_data = load_binary(disk_path);
            if !disk_data.is_empty() {
                disk.load(disk_data);
            }
        }

        bus.add_device(Box::new(mem));
        bus.add_device(Box::new(uart));
        bus.add_device(Box::new(disk));
        bus.add_device(Box::new(clint));

        let mem_controller: Box<dyn MemoryController> = match config.memory.controller.as_str() {
            "DRAM" => Box::new(DramController::new(
                config.memory.t_cas,
                config.memory.t_ras,
                config.memory.t_pre,
            )),
            _ => Box::new(SimpleController::new(config.memory.row_miss_latency)),
        };

        Self {
            bus,
            mem_controller,
        }
    }

    pub fn load_binary_at(&mut self, data: &[u8], addr: u64) {
        self.bus.load_binary_at(data, addr);
    }

    pub fn tick(&mut self) -> bool {
        self.bus.tick()
    }
}
