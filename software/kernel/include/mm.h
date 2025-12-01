#ifndef MM_H
#define MM_H

#include <stdint.h>

void kinit(void);
void *kalloc(void);
void kfree(void *pa);

#endif
