TARGET = riscv64-elf
CC = $(TARGET)-gcc
LD = $(TARGET)-ld
OBJCOPY = $(TARGET)-objcopy

# Directories (Relative to software/)
BUILD_DIR = build
LIB_DIR   = libc
USER_DIR  = user
KERN_DIR  = kernel
BENCH_DIR = benchmarks

# Output Directories
BIN_DIR   = bin
BIN_KERN  = $(BIN_DIR)/kernel
BIN_USER  = $(BIN_DIR)/user
BIN_BENCH = $(BIN_DIR)/benchmarks

# Base Flags (Architecture + Freestanding + Includes)
CFLAGS = -march=rv64g -mabi=lp64 -mcmodel=medany -ffreestanding -nostdlib -g
INCLUDES = -I$(LIB_DIR) -I$(KERN_DIR)/include

# Specific Optimization Levels
KERN_CFLAGS  = $(CFLAGS) $(INCLUDES) -O3
USER_CFLAGS  = $(CFLAGS) $(INCLUDES) -O3
# Microbenchmarks need -O0 to preserve volatile loops/timing loops
MICRO_CFLAGS = $(CFLAGS) $(INCLUDES) -O0
# Real apps need -O3 for realistic register pressure and instruction mix
APP_CFLAGS   = $(CFLAGS) $(INCLUDES) -O3

# Objects
CRT0_OBJ   = $(BUILD_DIR)/libc/crt0.o
STDIO_OBJ  = $(BUILD_DIR)/libc/stdio.o
STDLIB_OBJ = $(BUILD_DIR)/libc/stdlib.o
LINKER_SCR = $(LIB_DIR)/user.ld

# --- Sources: Kernel ---
KERN_SRC_DIRS = $(KERN_DIR) $(KERN_DIR)/drivers $(KERN_DIR)/lib $(KERN_DIR)/fs $(KERN_DIR)/mm
KERN_C_SRCS = $(foreach dir, $(KERN_SRC_DIRS), $(wildcard $(dir)/*.c))
KERN_ENTRY_OBJ = $(BUILD_DIR)/kernel/entry.o
KERN_C_OBJS    = $(patsubst $(KERN_DIR)/%.c, $(BUILD_DIR)/kernel/%.o, $(KERN_C_SRCS))
KERN_BIN  = $(BIN_KERN)/kernel.bin

# --- Sources: User ---
USER_C_SRCS = $(wildcard $(USER_DIR)/*.c)
USER_C_BINS = $(patsubst $(USER_DIR)/%.c, $(BIN_USER)/%.bin, $(USER_C_SRCS))
USER_S_SRCS = $(wildcard $(USER_DIR)/*.s)
USER_S_BINS = $(patsubst $(USER_DIR)/%.s, $(BIN_USER)/%.bin, $(USER_S_SRCS))

# --- Sources: Benchmarks ---
BENCH_MICRO_SRCS = $(wildcard $(BENCH_DIR)/microbenchmarks/*.c)
BENCH_SYNTH_SRCS = $(wildcard $(BENCH_DIR)/synthetic/*.c)
BENCH_KERN_SRCS  = $(wildcard $(BENCH_DIR)/kernels/*.c)
BENCH_PROG_SRCS  = $(wildcard $(BENCH_DIR)/complete_prog/*.c)

BENCH_MICRO_BINS = $(patsubst $(BENCH_DIR)/microbenchmarks/%.c, $(BIN_BENCH)/%.bin, $(BENCH_MICRO_SRCS))
BENCH_SYNTH_BINS = $(patsubst $(BENCH_DIR)/synthetic/%.c, $(BIN_BENCH)/%.bin, $(BENCH_SYNTH_SRCS))
BENCH_KERN_BINS  = $(patsubst $(BENCH_DIR)/kernels/%.c, $(BIN_BENCH)/%.bin, $(BENCH_KERN_SRCS))
BENCH_PROG_BINS  = $(patsubst $(BENCH_DIR)/complete_prog/%.c, $(BIN_BENCH)/%.bin, $(BENCH_PROG_SRCS))

BENCH_BINS = $(BENCH_MICRO_BINS) $(BENCH_SYNTH_BINS) $(BENCH_KERN_BINS) $(BENCH_PROG_BINS)

.PHONY: all clean dirs disk

all: dirs disk

dirs:
	@mkdir -p $(BUILD_DIR)/kernel/drivers $(BUILD_DIR)/kernel/lib \
	          $(BUILD_DIR)/kernel/fs $(BUILD_DIR)/kernel/mm \
	          $(BUILD_DIR)/libc $(BUILD_DIR)/user \
	          $(BUILD_DIR)/benchmarks \
	          $(BIN_KERN) $(BIN_USER) $(BIN_BENCH)

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
$(BIN_USER)/%.bin: $(USER_DIR)/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (User) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/user/$*.o -o $(BUILD_DIR)/user/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/user/$*.elf $@

$(BIN_USER)/%.bin: $(USER_DIR)/%.s
	@echo "  AS (User) $<"
	@$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/$*.o
	@$(LD) -T $(LINKER_SCR) $(BUILD_DIR)/user/$*.o -o $(BUILD_DIR)/user/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/user/$*.elf $@

# --- Benchmarks Rules ---

# 1. Microbenchmarks (O0)
$(BENCH_MICRO_BINS): $(BIN_BENCH)/%.bin: $(BENCH_DIR)/microbenchmarks/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Micro -O0) $<"
	@$(CC) $(MICRO_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

# 2. Synthetic (O3)
$(BENCH_SYNTH_BINS): $(BIN_BENCH)/%.bin: $(BENCH_DIR)/synthetic/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Synth -O3) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

# 3. Kernels (O3)
$(BENCH_KERN_BINS): $(BIN_BENCH)/%.bin: $(BENCH_DIR)/kernels/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Kern -O3) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

# 4. Complete Programs (O3)
$(BENCH_PROG_BINS): $(BIN_BENCH)/%.bin: $(BENCH_DIR)/complete_prog/%.c $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ)
	@echo "  CC (Bench/Prog -O3) $<"
	@$(CC) $(APP_CFLAGS) -c $< -o $(BUILD_DIR)/benchmarks/$*.o
	@$(LD) -T $(LINKER_SCR) $(CRT0_OBJ) $(STDIO_OBJ) $(STDLIB_OBJ) $(BUILD_DIR)/benchmarks/$*.o -o $(BUILD_DIR)/benchmarks/$*.elf
	@$(OBJCOPY) -O binary $(BUILD_DIR)/benchmarks/$*.elf $@

# --- Disk ---
disk: $(KERN_BIN) $(USER_C_BINS) $(USER_S_BINS) $(BENCH_BINS)
	@echo "  PACKING disk.img"
	@python3 tools/mkfs.py

clean:
	rm -rf build bin *.img
