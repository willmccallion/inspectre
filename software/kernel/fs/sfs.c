#include "drivers.h"
#include "fs.h"
#include "kdefs.h"
#include "klib.h"

static struct FileHeader *get_headers(uint32_t *count_out) {
  uint8_t *disk = disk_get_base();
  uint32_t *count_ptr = (uint32_t *)(disk + KERNEL_SIZE);
  *count_out = *count_ptr;
  // Headers start immediately after the count
  return (struct FileHeader *)(disk + KERNEL_SIZE + 4);
}

void fs_ls(void) {
  uint32_t count;
  struct FileHeader *headers = get_headers(&count);

  kprint("PERM   SIZE    NAME\n");
  kprint("----   ----    ----\n");
  for (uint32_t i = 0; i < count; i++) {
    kprint("-r-x   ");
    kprint_long(headers[i].size);
    kprint("    ");
    kprint(headers[i].name);
    kprint("\n");
  }
}

int fs_find(const char *name, struct FileHeader *out_header) {
  uint32_t count;
  struct FileHeader *headers = get_headers(&count);

  for (uint32_t i = 0; i < count; i++) {
    if (kstrcmp(name, headers[i].name) == 0) {
      *out_header = headers[i];
      return 1; // Found
    }
  }
  return 0; // Not found
}

void fs_load(const struct FileHeader *header, void *dst) {
  uint8_t *disk = disk_get_base();
  uint8_t *src = (uint8_t *)(disk + header->offset);
  kmemcpy(dst, src, header->size);
}
