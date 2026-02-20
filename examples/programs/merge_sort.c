#include "stdio.h"

#define SIZE 1024

long arr[SIZE];
long tmp[SIZE];

static unsigned long long seed = 999;
long rand_next(void) {
  seed = seed * 6364136223846793005ULL + 1;
  return (long)(seed >> 33);
}

void merge(long arr[], long tmp[], int l, int m, int r) {
  int i, j, k;
  int n1 = m - l + 1;
  int n2 = r - m;

  // Copy both halves into tmp
  for (i = 0; i < n1; i++)
    tmp[i] = arr[l + i];
  for (j = 0; j < n2; j++)
    tmp[n1 + j] = arr[m + 1 + j];

  // Merge back
  i = 0;
  j = 0;
  k = l;
  while (i < n1 && j < n2) {
    if (tmp[i] <= tmp[n1 + j]) {
      arr[k++] = tmp[i++];
    } else {
      arr[k++] = tmp[n1 + j];
      j++;
    }
  }

  while (i < n1)
    arr[k++] = tmp[i++];
  while (j < n2) {
    arr[k++] = tmp[n1 + j];
    j++;
  }
}

void merge_sort(long arr[], long tmp[], int l, int r) {
  if (l < r) {
    int m = l + (r - l) / 2;
    merge_sort(arr, tmp, l, m);
    merge_sort(arr, tmp, m + 1, r);
    merge(arr, tmp, l, m, r);
  }
}

int main(void) {
  printf("Initializing array with %d elements...\n", SIZE);
  for (int i = 0; i < SIZE; i++)
    arr[i] = rand_next() % 1000;

  printf("Starting Merge Sort...\n");
  merge_sort(arr, tmp, 0, SIZE - 1);

  printf("Verifying...\n");
  int sorted = 1;
  for (int i = 0; i < SIZE - 1; i++) {
    if (arr[i] > arr[i + 1]) {
      printf("Error at index %d: %ld > %ld\n", i, arr[i], arr[i + 1]);
      sorted = 0;
      break;
    }
  }

  if (sorted)
    printf("SUCCESS: Array is sorted.\n");
  else
    printf("FAILURE: Array is NOT sorted.\n");

  return 0;
}
