pub mod clint;
pub mod plic;
pub mod syscon;
pub mod uart;
pub mod virtio_disk;

pub use clint::Clint;
pub use plic::Plic;
pub use syscon::SysCon;
pub use uart::Uart;
pub use virtio_disk::VirtioBlock;

pub trait Device {
    fn name(&self) -> &str;
    fn address_range(&self) -> (u64, u64);
    fn read_u8(&mut self, offset: u64) -> u8;
    fn read_u16(&mut self, offset: u64) -> u16;
    fn read_u32(&mut self, offset: u64) -> u32;
    fn read_u64(&mut self, offset: u64) -> u64;
    fn write_u8(&mut self, offset: u64, val: u8);
    fn write_u16(&mut self, offset: u64, val: u16);
    fn write_u32(&mut self, offset: u64, val: u32);
    fn write_u64(&mut self, offset: u64, val: u64);
    fn write_bytes(&mut self, offset: u64, data: &[u8]) {
        for (i, byte) in data.iter().enumerate() {
            self.write_u8(offset + i as u64, *byte);
        }
    }
    fn tick(&mut self) -> bool {
        false
    }

    fn get_irq_id(&self) -> Option<u32> {
        None
    }
    fn as_plic_mut(&mut self) -> Option<&mut Plic> {
        None
    }
}
