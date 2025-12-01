#ifndef KLIB_H
#define KLIB_H

#include <stdint.h>

// IO
void kprint(const char *s);
void kprint_long(long n);
void kprint_hex(uint64_t n);
void kgets(char *buf, int max);

// String / Memory
int kstrcmp(const char *s1, const char *s2);
void kmemcpy(void *dest, const void *src, uint32_t n);
void kmemset(void *dest, uint8_t val, uint32_t n);

#endif
