#include "drivers.h"

uint64_t clint_get_time(void) {
  volatile uint64_t *mtime = (volatile uint64_t *)CLINT_MTIME;
  return *mtime;
}

void clint_sleep(uint64_t milliseconds) {
  uint64_t start = clint_get_time();
  // Assuming 1MHz tick rate for the simulator
  uint64_t cycles = milliseconds * 1000;

  while (clint_get_time() < start + cycles) {
    asm volatile("nop");
  }
}
