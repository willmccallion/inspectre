#include "drivers.h"

// Read the 64-bit cycle counter from the CLINT hardware
uint64_t clint_get_time(void) {
  volatile uint64_t *mtime = (volatile uint64_t *)CLINT_MTIME;
  return *mtime;
}

// Busy-wait sleep using the hardware timer
void clint_sleep(uint64_t milliseconds) {
  uint64_t start = clint_get_time();
  uint64_t cycles = milliseconds * 1000;

  while (clint_get_time() < start + cycles) {
    // Busy wait
    asm volatile("nop");
  }
}
