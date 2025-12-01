#include "klib.h"

int kstrcmp(const char *s1, const char *s2) {
  while (*s1 && (*s1 == *s2)) {
    s1++;
    s2++;
  }
  return *(const unsigned char *)s1 - *(const unsigned char *)s2;
}

void kmemcpy(void *dest, const void *src, uint32_t n) {
  uint8_t *d = (uint8_t *)dest;
  const uint8_t *s = (const uint8_t *)src;
  while (n--)
    *d++ = *s++;
}

void kmemset(void *dest, uint8_t val, uint32_t n) {
  uint8_t *d = (uint8_t *)dest;
  while (n--)
    *d++ = val;
}
