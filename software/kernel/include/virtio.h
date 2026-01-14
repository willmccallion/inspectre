#ifndef VIRTIO_H
#define VIRTIO_H

#include <stdint.h>

// VirtIO MMIO Register Offsets
#define VIRTIO_MMIO_MAGIC_VALUE 0x000
#define VIRTIO_MMIO_VERSION 0x004
#define VIRTIO_MMIO_DEVICE_ID 0x008
#define VIRTIO_MMIO_VENDOR_ID 0x00c
#define VIRTIO_MMIO_DEVICE_FEATURES 0x010
#define VIRTIO_MMIO_DRIVER_FEATURES 0x020
#define VIRTIO_MMIO_GUEST_PAGE_SIZE 0x028
#define VIRTIO_MMIO_QUEUE_SEL 0x030
#define VIRTIO_MMIO_QUEUE_NUM_MAX 0x034
#define VIRTIO_MMIO_QUEUE_NUM 0x038
#define VIRTIO_MMIO_QUEUE_ALIGN 0x03c
#define VIRTIO_MMIO_QUEUE_PFN 0x040
#define VIRTIO_MMIO_QUEUE_READY 0x044
#define VIRTIO_MMIO_QUEUE_NOTIFY 0x050
#define VIRTIO_MMIO_INTERRUPT_STATUS 0x060
#define VIRTIO_MMIO_INTERRUPT_ACK 0x064
#define VIRTIO_MMIO_STATUS 0x070
#define VIRTIO_MMIO_QUEUE_DESC_LOW 0x080
#define VIRTIO_MMIO_QUEUE_DESC_HIGH 0x084
#define VIRTIO_MMIO_QUEUE_AVAIL_LOW 0x090
#define VIRTIO_MMIO_QUEUE_AVAIL_HIGH 0x094
#define VIRTIO_MMIO_QUEUE_USED_LOW 0x0a0
#define VIRTIO_MMIO_QUEUE_USED_HIGH 0x0a4

// Status Bits
#define VIRTIO_STATUS_ACKNOWLEDGE 1
#define VIRTIO_STATUS_DRIVER 2
#define VIRTIO_STATUS_DRIVER_OK 4
#define VIRTIO_STATUS_FEATURES_OK 8

// Descriptor Flags
#define VIRTQ_DESC_F_NEXT 1
#define VIRTQ_DESC_F_WRITE 2

// Block Request Types
#define VIRTIO_BLK_T_IN 0
#define VIRTIO_BLK_T_OUT 1

struct virtq_desc {
  uint64_t addr;
  uint32_t len;
  uint16_t flags;
  uint16_t next;
};

struct virtq_avail {
  uint16_t flags;
  uint16_t idx;
  uint16_t ring[16]; // Fixed size for simplicity
};

struct virtq_used_elem {
  uint32_t id;
  uint32_t len;
};

struct virtq_used {
  uint16_t flags;
  uint16_t idx;
  struct virtq_used_elem ring[16];
};

struct virtio_blk_req {
  uint32_t type;
  uint32_t reserved;
  uint64_t sector;
};

void virtio_init(void);
void virtio_disk_read(uint64_t sector, void *dst, uint32_t count);

#endif
