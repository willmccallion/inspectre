.PHONY: all clean run build-software build-hardware

all: build-software build-hardware

build-software:
	@echo "[Software] Building OS & Userland..."
	$(MAKE) -C software

build-hardware:
	@echo "[Hardware] Building CPU Simulator..."
	cd hardware && cargo build --release

# Passes the correct relative paths to the binary
run: all
	@echo "[Running] Booting RISC-V System..."
	./hardware/target/release/riscv-emulator \
		--config hardware/configs/default.toml \
		--disk software/disk.img

clean:
	$(MAKE) -C software clean
	rm -rf target
	cd hardware && cargo clean
