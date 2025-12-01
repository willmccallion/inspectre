#ifndef FS_H
#define FS_H

#include <stdint.h>

struct FileHeader {
  char name[32];
  uint32_t offset;
  uint32_t size;
};

void fs_ls(void);
int fs_find(const char *name, struct FileHeader *out_header);
void fs_load(const struct FileHeader *header, void *dst);

#endif
