{
  description = "rvsim — RISC-V cycle-accurate simulator dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        # Cross toolchain is referenced via store path, not put in PATH —
        # otherwise spike's autoconf picks `riscv64-none-elf-gcc` for the
        # native build and fails with "cannot run C compiled programs".
        riscvGcc = pkgs.pkgsCross.riscv64-embedded.buildPackages.gcc;
        riscvBinutils = pkgs.pkgsCross.riscv64-embedded.buildPackages.binutils;
      in {
        devShells.default = pkgs.mkShell {
          name = "rvsim";

          packages = with pkgs; [
            rustc cargo rustfmt clippy

            python3
            python3Packages.pip
            python3Packages.virtualenv
            python3Packages.ruff

            gcc gnumake autoconf automake pkg-config dtc cmake

            go

            git curl
          ];

          shellHook = ''
            # Defensive: nix stdenv may set CC to a wrapper that resolves to
            # the wrong toolchain. Force native for spike's configure.
            unset CC CXX AR RANLIB

            export TOOLCHAIN_BIN="$PWD/.nix-toolchain-bin"
            mkdir -p "$TOOLCHAIN_BIN"
            for tool in gcc g++ as ld objdump objcopy strip ar nm ranlib readelf; do
              if [ -e ${riscvGcc}/bin/riscv64-none-elf-$tool ]; then
                ln -sf ${riscvGcc}/bin/riscv64-none-elf-$tool "$TOOLCHAIN_BIN/riscv64-elf-$tool"
              elif [ -e ${riscvBinutils}/bin/riscv64-none-elf-$tool ]; then
                ln -sf ${riscvBinutils}/bin/riscv64-none-elf-$tool "$TOOLCHAIN_BIN/riscv64-elf-$tool"
              fi
            done
            export PATH="$TOOLCHAIN_BIN:$PATH"

            echo "rvsim devshell ready"
            echo "  rust:      $(rustc --version 2>/dev/null)"
            echo "  go:        $(go version 2>/dev/null)"
            echo "  riscv-gcc: $(command -v riscv64-elf-gcc)"
            echo "  native cc: $(command -v gcc)"
            echo "  dtc:       $(command -v dtc)"
          '';
        };
      });
}
