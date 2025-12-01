#include "kdefs.h"
#include "klib.h"

extern char _kernel_end[];

#define PAGE_SIZE 4096
#define RAM_START 0x80000000
#define RAM_SIZE (128 * 1024 * 1024) // 128 MB
#define RAM_END (RAM_START + RAM_SIZE)

struct Run {
  struct Run *next;
};

struct Run *freelist;

void kinit() {
  freelist = 0;

  // Start allocating AFTER the kernel
  uint64_t start = (uint64_t)_kernel_end;
  // Align to next page boundary
  start = (start + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);

  // Chop memory into pages and free them
  for (; start + PAGE_SIZE <= RAM_END; start += PAGE_SIZE) {
    // We can't use kfree yet because we are manually building the list
    struct Run *r = (struct Run *)start;
    r->next = freelist;
    freelist = r;
  }
}

// Allocate one 4096-byte physical page
void *kalloc() {
  struct Run *r = freelist;
  if (r) {
    freelist = r->next;
    // Zero out the page for security/safety
    kmemset(r, 0, PAGE_SIZE);
  }
  return (void *)r;
}

// Free a physical page
void kfree(void *pa) {
  struct Run *r = (struct Run *)pa;

  // Sanity check: Ensure address is aligned and within RAM
  if ((uint64_t)pa % PAGE_SIZE != 0 || (uint64_t)pa < RAM_START ||
      (uint64_t)pa >= RAM_END) {
    kprint("PMM: Panic! Invalid kfree ");
    kprint_hex((uint64_t)pa);
    kprint("\n");
    while (1)
      ;
  }

  // Fill with junk to catch dangling pointer bugs
  kmemset(pa, 1, PAGE_SIZE);

  r->next = freelist;
  freelist = r;
}
