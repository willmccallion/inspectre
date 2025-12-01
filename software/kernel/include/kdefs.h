#ifndef KDEFS_H
#define KDEFS_H

#include <stdint.h>

// Memory Layout
#define KERNEL_SIZE 65536
#define RAM_USER_BASE 0x80200000

// ANSI Colors
#define ANSI_GREEN "\x1b[32m"
#define ANSI_RED "\x1b[31m"
#define ANSI_CYAN "\x1b[36m"
#define ANSI_RESET "\x1b[0m"

// Assembly External
extern long switch_to_user(uint64_t entry_point);

#endif
