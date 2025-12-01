#include "drivers.h"
#include "klib.h"

void kprint(const char *s) {
  while (*s)
    uart_putc(*s++);
}

void kprint_long(long n) {
  if (n == 0) {
    uart_putc('0');
    return;
  }
  if (n < 0) {
    uart_putc('-');
    n = -n;
  }
  char buf[20];
  int i = 0;
  while (n > 0) {
    buf[i++] = (n % 10) + '0';
    n /= 10;
  }
  while (i > 0)
    uart_putc(buf[--i]);
}

void kprint_hex(uint64_t n) {
  kprint("0x");
  char hex[] = "0123456789abcdef";
  for (int i = 60; i >= 0; i -= 4) {
    int nibble = (n >> i) & 0xF;
    uart_putc(hex[nibble]);
  }
}

void kgets(char *buf, int max) {
  int i = 0;
  while (i < max - 1) {
    char c = uart_getc();
    if (c == 0)
      continue;

    if (c == 127 || c == '\b') { // Backspace
      if (i > 0)
        i--;
      continue;
    }

    if (c == '\n' || c == '\r')
      break;

    buf[i++] = c;
  }
  buf[i] = 0;
  uart_putc('\n');
}
