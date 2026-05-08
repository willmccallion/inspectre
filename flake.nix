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
        riscv = pkgs.pkgsCross.riscv64-embedded.buildPackages;
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

            riscv.gcc
            riscv.binutils

            git curl
          ];

          shellHook = ''
            export TOOLCHAIN_BIN="$PWD/.nix-toolchain-bin"
            mkdir -p "$TOOLCHAIN_BIN"
            for tool in gcc g++ as ld objdump objcopy strip ar nm ranlib readelf; do
              src=$(command -v riscv64-none-elf-$tool 2>/dev/null) || continue
              ln -sf "$src" "$TOOLCHAIN_BIN/riscv64-elf-$tool"
            done
            export PATH="$TOOLCHAIN_BIN:$PATH"

            echo "rvsim devshell ready"
            echo "  rust:      $(rustc --version 2>/dev/null)"
            echo "  go:        $(go version 2>/dev/null)"
            echo "  riscv-gcc: $(command -v riscv64-elf-gcc)"
            echo "  dtc:       $(command -v dtc)"
          '';
        };
      });
}
