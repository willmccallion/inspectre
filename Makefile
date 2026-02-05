# Top-level Makefile: build all software and the simulator.
# Use this to build everything; use software/Makefile for software-only.

.PHONY: all software hardware linux clean run-bare run-linux

# Default: build simulator and all bare-metal software (kernel, user, benchmarks, disk).
# Linux (Image + rootfs) is separate: make linux
all: hardware software

# RISC-V simulator (release binary at target/release/sim)
hardware:
	cargo build --release

# Bare-metal: kernel, user programs, benchmarks, disk image.
# Does not build Linux; use 'make linux' for that.
software:
	$(MAKE) -C software

# Linux: Buildroot Image + rootfs (downloads Buildroot, builds kernel + rootfs).
# Output: software/linux/output/Image, disk.img, fw_jump.bin
linux:
	$(MAKE) -C software linux

# Convenience: run a bare-metal binary (builds if needed)
run-bare: hardware software
	./target/release/sim run -f software/bin/benchmarks/qsort.bin

# Convenience: build Linux then boot (builds if needed)
run-linux: hardware
	./target/release/sim script scripts/setup/boot_linux.py

clean:
	$(MAKE) -C software clean
	cargo clean
