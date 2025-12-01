#include "drivers.h"

#define UART_PTR ((volatile char *)UART_BASE)

void uart_putc(char c) { *UART_PTR = c; }

char uart_getc(void) { return *UART_PTR; }

uint8_t *disk_get_base(void) { return (uint8_t *)DISK_BASE; }
