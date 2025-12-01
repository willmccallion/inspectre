pub mod clint;
pub mod uart;
pub mod virtio_disk;

pub use clint::Clint;
pub use uart::Uart;
pub use virtio_disk::VirtualDisk;

/// A trait representing a memory-mapped hardware device.
pub trait Device {
    /// Returns the display name of the device for debugging.
    fn name(&self) -> &str;

    /// Returns the (Base Address, Size) of the device.
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
}
