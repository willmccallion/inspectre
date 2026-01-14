#include "drivers.h"
#include "fs.h"
#include "kdefs.h"
#include "klib.h"
#include "mm.h"

static void disk_read_bytes(uint32_t offset, void *dst, uint32_t len) {
  uint64_t start_sector = offset / 512;
  uint64_t end_sector = (offset + len - 1) / 512;
  uint32_t sector_count = end_sector - start_sector + 1;

  uint8_t *buf = (uint8_t *)kalloc();

  for (uint64_t s = 0; s < sector_count; s++) {
    virtio_disk_read(start_sector + s, buf, 512);

    uint32_t buf_offset = 0;
    uint32_t copy_len = 512;

    if (s == 0) {
      buf_offset = offset % 512;
      copy_len = 512 - buf_offset;
    }

    if (s == sector_count - 1) {
      uint32_t remaining = len - ((s * 512) - (offset % 512));
      if (remaining < copy_len)
        copy_len = remaining;
    }

    kmemcpy((uint8_t *)dst + ((s * 512) - (offset % 512)), buf + buf_offset,
            copy_len);
  }
  kfree(buf);
}

void fs_ls(void) {
  uint32_t count;
  disk_read_bytes(KERNEL_SIZE, &count, 4);

  kprint("PERM   SIZE    NAME\n");
  kprint("----   ----    ----\n");

  struct FileHeader fh;
  uint32_t header_offset = KERNEL_SIZE + 4;

  for (uint32_t i = 0; i < count; i++) {
    disk_read_bytes(header_offset + (i * sizeof(struct FileHeader)), &fh,
                    sizeof(struct FileHeader));
    kprint("-r-x   ");
    kprint_long(fh.size);
    kprint("    ");
    kprint(fh.name);
    kprint("\n");
  }
}

int fs_find(const char *name, struct FileHeader *out_header) {
  uint32_t count;
  disk_read_bytes(KERNEL_SIZE, &count, 4);

  struct FileHeader fh;
  uint32_t header_offset = KERNEL_SIZE + 4;

  for (uint32_t i = 0; i < count; i++) {
    disk_read_bytes(header_offset + (i * sizeof(struct FileHeader)), &fh,
                    sizeof(struct FileHeader));
    if (kstrcmp(name, fh.name) == 0) {
      *out_header = fh;
      return 1;
    }
  }
  return 0;
}

void fs_load(const struct FileHeader *header, void *dst) {
  disk_read_bytes(header->offset, dst, header->size);
}
