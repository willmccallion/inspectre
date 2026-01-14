use crate::system::devices::Device;
use std::convert::TryInto;

const VIRTIO_MMIO_MAGIC: u32 = 0x74726976;
const VIRTIO_MMIO_VENDOR_ID: u32 = 0x554d4551;
const VIRTIO_MMIO_DEVICE_ID: u32 = 2;

const VIRTQ_DESC_F_NEXT: u16 = 1;

pub struct VirtioBlock {
    base_addr: u64,
    disk_image: Vec<u8>,
    ram_ptr: *mut u8, // Pointer to main RAM
    ram_size: usize,

    status: u32,
    queue_num: u32,
    queue_ready: u32,
    queue_notify: u32,

    queue_desc_low: u32,
    queue_desc_high: u32,
    queue_avail_low: u32,
    queue_avail_high: u32,
    queue_used_low: u32,
    queue_used_high: u32,

    interrupt_status: u32,
    last_avail_idx: u16,
}

unsafe impl Send for VirtioBlock {}
unsafe impl Sync for VirtioBlock {}

impl VirtioBlock {
    pub fn new(base_addr: u64, ram_ptr: *mut u8, ram_size: usize) -> Self {
        Self {
            base_addr,
            disk_image: Vec::new(),
            ram_ptr,
            ram_size,
            status: 0,
            queue_num: 0,
            queue_ready: 0,
            queue_notify: 0,
            queue_desc_low: 0,
            queue_desc_high: 0,
            queue_avail_low: 0,
            queue_avail_high: 0,
            queue_used_low: 0,
            queue_used_high: 0,
            interrupt_status: 0,
            last_avail_idx: 0,
        }
    }

    pub fn load(&mut self, data: Vec<u8>) {
        self.disk_image = data;
    }

    fn dma_read(&self, addr: u64, len: usize) -> Vec<u8> {
        if addr + (len as u64) > self.ram_size as u64 {
            return vec![0; len];
        }
        unsafe {
            let ptr = self.ram_ptr.add(addr as usize);
            std::slice::from_raw_parts(ptr, len).to_vec()
        }
    }

    fn dma_write(&self, addr: u64, data: &[u8]) {
        if addr + (data.len() as u64) > self.ram_size as u64 {
            return;
        }
        unsafe {
            let ptr = self.ram_ptr.add(addr as usize);
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        }
    }

    fn process_queue(&mut self) {
        let desc_addr = ((self.queue_desc_high as u64) << 32) | (self.queue_desc_low as u64);
        let avail_addr = ((self.queue_avail_high as u64) << 32) | (self.queue_avail_low as u64);
        let used_addr = ((self.queue_used_high as u64) << 32) | (self.queue_used_low as u64);

        let avail_idx = u16::from_le_bytes(self.dma_read(avail_addr + 2, 2).try_into().unwrap());

        while self.last_avail_idx != avail_idx {
            let ring_offset = 4 + (self.last_avail_idx as u64 % self.queue_num as u64) * 2;
            let head_idx = u16::from_le_bytes(
                self.dma_read(avail_addr + ring_offset, 2)
                    .try_into()
                    .unwrap(),
            );

            // 1. Descriptor 0: Header
            let d0_addr = u64::from_le_bytes(
                self.dma_read(desc_addr + (head_idx as u64 * 16), 8)
                    .try_into()
                    .unwrap(),
            );
            let d0_next = u16::from_le_bytes(
                self.dma_read(desc_addr + (head_idx as u64 * 16) + 12, 2)
                    .try_into()
                    .unwrap(),
            );

            let header = self.dma_read(d0_addr, 16);
            let sector = u64::from_le_bytes(header[8..16].try_into().unwrap());

            // 2. Descriptor 1: Data
            let d1_addr = u64::from_le_bytes(
                self.dma_read(desc_addr + (d0_next as u64 * 16), 8)
                    .try_into()
                    .unwrap(),
            );
            let d1_len = u32::from_le_bytes(
                self.dma_read(desc_addr + (d0_next as u64 * 16) + 8, 4)
                    .try_into()
                    .unwrap(),
            );
            let d1_next = u16::from_le_bytes(
                self.dma_read(desc_addr + (d0_next as u64 * 16) + 12, 2)
                    .try_into()
                    .unwrap(),
            );

            // Read from Disk Image
            let offset = (sector * 512) as usize;
            if offset < self.disk_image.len() {
                let len = std::cmp::min(d1_len as usize, self.disk_image.len() - offset);
                self.dma_write(d1_addr, &self.disk_image[offset..offset + len]);
            }

            // 3. Descriptor 2: Status
            let d2_addr = u64::from_le_bytes(
                self.dma_read(desc_addr + (d1_next as u64 * 16), 8)
                    .try_into()
                    .unwrap(),
            );
            self.dma_write(d2_addr, &[0]); // Success

            // Update Used Ring
            let used_idx_addr = used_addr + 2;
            let current_used =
                u16::from_le_bytes(self.dma_read(used_idx_addr, 2).try_into().unwrap());
            let used_elem = used_addr + 4 + (current_used as u64 % self.queue_num as u64) * 8;

            self.dma_write(used_elem, &u32::from(head_idx).to_le_bytes());
            self.dma_write(used_elem + 4, &0u32.to_le_bytes());
            self.dma_write(used_idx_addr, &current_used.wrapping_add(1).to_le_bytes());

            self.last_avail_idx = self.last_avail_idx.wrapping_add(1);
        }
        self.interrupt_status |= 1;
    }
}

impl Device for VirtioBlock {
    fn name(&self) -> &str {
        "VirtIO-Blk"
    }
    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x1000)
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        match offset {
            0x00 => VIRTIO_MMIO_MAGIC,
            0x04 => 2,
            0x08 => VIRTIO_MMIO_DEVICE_ID,
            0x0c => VIRTIO_MMIO_VENDOR_ID,
            0x10 => 0,
            0x34 => 16,
            0x44 => self.queue_ready,
            0x60 => self.interrupt_status,
            0x70 => self.status,
            _ => 0,
        }
    }

    fn write_u32(&mut self, offset: u64, val: u32) {
        match offset {
            0x30 => {} // Queue Sel
            0x38 => self.queue_num = val,
            0x44 => self.queue_ready = val,
            0x50 => {
                self.queue_notify = val;
                self.process_queue();
            }
            0x64 => self.interrupt_status &= !val,
            0x70 => self.status = val,
            0x80 => self.queue_desc_low = val,
            0x84 => self.queue_desc_high = val,
            0x90 => self.queue_avail_low = val,
            0x94 => self.queue_avail_high = val,
            0xa0 => self.queue_used_low = val,
            0xa4 => self.queue_used_high = val,
            _ => {}
        }
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        (self.read_u32(offset & !3) >> ((offset & 3) * 8)) as u8
    }
    fn read_u16(&mut self, offset: u64) -> u16 {
        (self.read_u32(offset & !3) >> ((offset & 3) * 8)) as u16
    }
    fn read_u64(&mut self, offset: u64) -> u64 {
        self.read_u32(offset) as u64
    }
    fn write_u8(&mut self, offset: u64, val: u8) {
        self.write_u32(offset & !3, val as u32);
    }
    fn write_u16(&mut self, offset: u64, val: u16) {
        self.write_u32(offset & !3, val as u32);
    }
    fn write_u64(&mut self, offset: u64, val: u64) {
        self.write_u32(offset, val as u32);
    }

    fn tick(&mut self) -> bool {
        (self.interrupt_status & 1) != 0
    }
    fn get_irq_id(&self) -> Option<u32> {
        Some(1)
    }
}
