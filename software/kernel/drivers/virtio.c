#include "virtio.h"
#include "drivers.h"
#include "klib.h"
#include "mm.h"

#define VIRTIO0 VIRTIO_BASE
#define QUEUE_SIZE 16

// Data structures must be aligned to 4096 for VirtIO
__attribute__((aligned(4096))) struct virtq_desc desc[QUEUE_SIZE];
__attribute__((aligned(4096))) struct virtq_avail avail;
__attribute__((aligned(4096))) struct virtq_used used;

static int free_head = 0;
static int used_idx = 0;

static volatile uint32_t *reg(uint32_t offset) {
  return (volatile uint32_t *)(VIRTIO0 + offset);
}

void virtio_init(void) {
  if (*reg(VIRTIO_MMIO_MAGIC_VALUE) != 0x74726976) {
    kprint("VirtIO: Device not found!\n");
    return;
  }

  *reg(VIRTIO_MMIO_STATUS) = 0; // Reset
  *reg(VIRTIO_MMIO_STATUS) = VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER;
  *reg(VIRTIO_MMIO_DRIVER_FEATURES) = *reg(VIRTIO_MMIO_DEVICE_FEATURES);
  *reg(VIRTIO_MMIO_STATUS) |= VIRTIO_STATUS_FEATURES_OK;

  *reg(VIRTIO_MMIO_QUEUE_SEL) = 0;
  *reg(VIRTIO_MMIO_QUEUE_NUM) = QUEUE_SIZE;

  uint64_t desc_addr = (uint64_t)desc;
  uint64_t avail_addr = (uint64_t)&avail;
  uint64_t used_addr = (uint64_t)&used;

  *reg(VIRTIO_MMIO_QUEUE_DESC_LOW) = (uint32_t)desc_addr;
  *reg(VIRTIO_MMIO_QUEUE_DESC_HIGH) = (uint32_t)(desc_addr >> 32);
  *reg(VIRTIO_MMIO_QUEUE_AVAIL_LOW) = (uint32_t)avail_addr;
  *reg(VIRTIO_MMIO_QUEUE_AVAIL_HIGH) = (uint32_t)(avail_addr >> 32);
  *reg(VIRTIO_MMIO_QUEUE_USED_LOW) = (uint32_t)used_addr;
  *reg(VIRTIO_MMIO_QUEUE_USED_HIGH) = (uint32_t)(used_addr >> 32);

  *reg(VIRTIO_MMIO_QUEUE_READY) = 1;
  *reg(VIRTIO_MMIO_STATUS) |= VIRTIO_STATUS_DRIVER_OK;

  for (int i = 0; i < QUEUE_SIZE - 1; i++)
    desc[i].next = i + 1;
  desc[QUEUE_SIZE - 1].next = 0;

  kprint("VirtIO: Initialized.\n");
}

static int alloc_desc(void) {
  int d = free_head;
  free_head = desc[d].next;
  return d;
}

static void free_desc(int d) {
  desc[d].next = free_head;
  free_head = d;
}

void virtio_disk_read(uint64_t sector, void *dst, uint32_t count) {
  int idx0 = alloc_desc();
  int idx1 = alloc_desc();
  int idx2 = alloc_desc();

  struct virtio_blk_req *req = (struct virtio_blk_req *)kalloc();
  req->type = VIRTIO_BLK_T_IN;
  req->sector = sector;

  uint8_t *status = (uint8_t *)kalloc();
  *status = 111;

  desc[idx0].addr = (uint64_t)req;
  desc[idx0].len = sizeof(struct virtio_blk_req);
  desc[idx0].flags = VIRTQ_DESC_F_NEXT;
  desc[idx0].next = idx1;

  desc[idx1].addr = (uint64_t)dst;
  desc[idx1].len = count;
  desc[idx1].flags = VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE;
  desc[idx1].next = idx2;

  desc[idx2].addr = (uint64_t)status;
  desc[idx2].len = 1;
  desc[idx2].flags = VIRTQ_DESC_F_WRITE;
  desc[idx2].next = 0;

  avail.ring[avail.idx % QUEUE_SIZE] = idx0;
  asm volatile("fence" ::: "memory");
  avail.idx++;
  asm volatile("fence" ::: "memory");

  *reg(VIRTIO_MMIO_QUEUE_NOTIFY) = 0;

  while (used.idx == used_idx)
    asm volatile("nop");
  used_idx++;

  kfree(req);
  kfree(status);
  free_desc(idx0);
  free_desc(idx1);
  free_desc(idx2);
}
