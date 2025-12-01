#ifndef DRIVERS_H
#define DRIVERS_H

#include <stdint.h>

#define UART_BASE 0x10000000
#define DISK_BASE 0x90000000
#define CLINT_BASE 0x02000000

// Register Offsets
#define CLINT_MSIP (CLINT_BASE + 0x0000)
#define CLINT_MTIMECMP (CLINT_BASE + 0x4000)
#define CLINT_MTIME (CLINT_BASE + 0xBFF8)

void uart_putc(char c);
char uart_getc(void);
uint8_t *disk_get_base(void);

uint64_t clint_get_time(void);
void clint_sleep(uint64_t milliseconds);

#endif
