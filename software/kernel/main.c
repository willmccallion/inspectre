#include "drivers.h"
#include "fs.h"
#include "kdefs.h"
#include "klib.h"
#include "mm.h"

void kmain() {
  kprint("\n" ANSI_CYAN "RISC-V OS (VirtIO Enabled)" ANSI_RESET "\n");

  kinit();       // Init Memory
  virtio_init(); // Init Disk

  kprint("[ " ANSI_GREEN "OK" ANSI_RESET " ] System Ready.\n\n");

  while (1) {
    kprint("# ");
    char cmd[32];
    kgets(cmd, 32);

    if (cmd[0] == 0)
      continue;
    if (kstrcmp(cmd, "ls") == 0) {
      fs_ls();
      continue;
    }
    if (kstrcmp(cmd, "exit") == 0) {
      *(volatile uint32_t *)SYSCON_BASE = 0x5555;
      while (1)
        ;
    }

    struct FileHeader fh;
    if (fs_find(cmd, &fh)) {
      kmemset((void *)RAM_USER_BASE, 0, 0x100000);
      fs_load(&fh, (void *)RAM_USER_BASE);
      switch_to_user(RAM_USER_BASE);
    } else {
      kprint("Unknown command.\n");
    }
  }
}
