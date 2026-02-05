# RISC-V Software Build System
# Supports RV64GC (IMAFDС) with double-precision hard-float ABI

TARGET = riscv64-elf
CC = $(TARGET)-gcc
LD = $(TARGET)-ld
OBJCOPY = $(TARGET)-objcopy

# Directories
BUILD_DIR = build
LIB_DIR   = libc
USER_DIR  = user
KERN_DIR  = kernel
BENCH_DIR = benchmarks
LINUX_DIR = linux

# Base Flags
# -march=rv64gc: RV64I + M (multiply) + A (atomics) + F (single-float) + D (double-float) + C (compressed)
# -mabi=lp64d: LP64 with double-precision hard-float calling convention
CFLAGS = -march=rv64gc -mabi=lp64d -mcmodel=medany -ffreestanding -nostdlib -g
INCLUDES = -I$(LIB_DIR) -I$(KERN_DIR)/include

# Specific Optimization Levels
KERN_CFLAGS  = $(CFLAGS) $(INCLUDES) -O3
USER_CFLAGS  = $(CFLAGS) $(INCLUDES) -O3
MICRO_CFLAGS = $(CFLAGS) $(INCLUDES) -O0
APP_CFLAGS   = $(CFLAGS) $(INCLUDES) -O3

# Objects
CRT0_OBJ   = $(BUILD_DIR)/libc/crt0.o
STDIO_OBJ  = $(BUILD_DIR)/libc/stdio.o
STDLIB_OBJ = $(BUILD_DIR)/libc/stdlib.o
LINKER_SCR = $(LIB_DIR)/user.ld

# --- Sources ---
KERN_SRC_DIRS = $(KERN_DIR) $(KERN_DIR)/drivers $(KERN_DIR)/lib $(KERN_DIR)/fs $(KERN_DIR)/mm
KERN_C_SRCS = $(foreach dir, $(KERN_SRC_DIRS), $(wildcard $(dir)/*.c))
KERN_ENTRY_OBJ = $(BUILD_DIR)/kernel/entry.o
KERN_C_OBJS    = $(patsubst $(KERN_DIR)/%.c, $(BUILD_DIR)/kernel/%.o, $(KERN_C_SRCS))
KERN_BIN  = bin/kernel/kernel.bin

USER_C_SRCS = $(wildcard $(USER_DIR)/*.c)
USER_C_BINS = $(patsubst $(USER_DIR)/%.c, bin/user/%.bin, $(USER_C_SRCS))
USER_S_SRCS = $(wildcard $(USER_DIR)/*.s)
USER_S_BINS = $(patsubst $(USER_DIR)/%.s, bin/user/%.bin, $(USER_S_SRCS))

BENCH_MICRO_SRCS = $(wildcard $(BENCH_DIR)/microbenchmarks/*.c)
BENCH_SYNTH_SRCS = $(wildcard $(BENCH_DIR)/synthetic/*.c)
BENCH_KERN_SRCS  = $(wildcard $(BENCH_DIR)/kernels/*.c)
BENCH_PROG_SRCS  = $(wildcard $(BENCH_DIR)/complete_prog/*.c)

BENCH_BINS = \
	$(patsubst $(BENCH_DIR)/microbenchmarks/%.c, bin/benchmarks/%.bin, $(BENCH_MICRO_SRCS)) \
	$(patsubst $(BENCH_DIR)/synthetic/%.c, bin/benchmarks/%.bin, $(BENCH_SYNTH_SRCS)) \
	$(patsubst $(BENCH_DIR)/kernels/%.c, bin/benchmarks/%.bin, $(BENCH_KERN_SRCS)) \
	$(patsubst $(BENCH_DIR)/complete_prog/%.c, bin/benchmarks/%.bin, $(BENCH_PROG_SRCS))

.PHONY: all clean dirs disk linux

all: dirs disk

dirs:
	@mkdir -p $(BUILD_DIR)/kernel/drivers $(BUILD_DIR)/kernel/lib \
	          $(BUILD_DIR)/kernel/fs $(BUILD_DIR)/kernel/mm \
	          $(BUILD_DIR)/libc $(BUILD_DIR)/user \
	          $(BUILD_DIR)/benchmarks \
	          bin/kernel bin/user bin/benchmarks

# --- Libc ---
$(CRT0_OBJ): $(LIB_DIR)/crt0.s
	@echo "  AS (Lib) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $@

$(STDIO_OBJ): $(LIB_DIR)/stdio.c
	@echo "  CC (Lib) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $@

$(STDLIB_OBJ): $(LIB_DIR)/stdlib.c
	@echo "  CC (Lib) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $@

# --- Kernel ---
$(BUILD_DIR)/kernel/%.o: $(KERN_DIR)/%.c
	@echo "  CC (Kern) $<"
	@$(CC) $(KERN_CFLAGS) -c $< -o $@

$(BUILD_DIR)/kernel/entry.o: $(KERN_DIR)/entry.s
	@echo "  AS (Kern) $<"
	@$(CC) $(KERN_CFLAGS) -c $< -o $@

$(KERN_BIN): $(KERN_ENTRY_OBJ) $(KERN_C_OBJS)
	@echo "  LD (Kern) $@"
	@$(LD) -T $(KERN_DIR)/kernel.ld $(KERN_ENTRY_OBJ) $(KERN_C_OBJS) -o $(BUILD_DIR)/kernel.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/kernel.elf $@

# --- User Apps ---
bin/user/%.bin: $(USER_DIR)/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (User) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/user/$*.o -o $(BUILD_DIR)/user/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/user/$*.elf $@

bin/user/%.bin: $(USER_DIR)/%.s
	@echo "  AS (User) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/$*.o
	@$(LD) -T $(LINKER_SCR) $(BUILD_DIR)/user/$*.o -o $(BUILD_DIR)/user/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/user/$*.elf $@

# --- Benchmarks ---
bin/benchmarks/%.bin: $(BENCH_DIR)/microbenchmarks/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Micro) $<"
	@$(CC) $(MICRO_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

bin/benchmarks/%.bin: $(BENCH_DIR)/synthetic/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Synth) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

bin/benchmarks/%.bin: $(BENCH_DIR)/kernels/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Kern) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

bin/benchmarks/%.bin: $(BENCH_DIR)/complete_prog/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Prog) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

# --- Linux (download Buildroot + build via scripts/setup/boot_linux.py) ---
linux:
	@echo "[Software] Building Linux (download + build in $(LINUX_DIR))..."
	@cd .. && python3 scripts/setup/boot_linux.py --no-boot

# --- Disk Image ---
disk: $(KERN_BIN) $(USER_C_BINS) $(USER_S_BINS) $(BENCH_BINS)
	@echo "  PACKING disk.img (Bare Metal)"
	@python3 tools/mkfs.py

clean:
	rm -rf build bin *.img
	rm -rf $(LINUX_DIR)/output $(LINUX_DIR)/buildroot-*
