{
  description = "suba dev environment (Nix)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        riscv = pkgs.pkgsCross.riscv64-embedded;
        loongarch = pkgs.pkgsCross.loongarch64-linux;
        riscvAliases = pkgs.symlinkJoin {
          name = "riscv64-unknown-elf-aliases";
          paths = [];
          postBuild = ''
            mkdir -p $out/bin
            for tool in gcc g++ gdb objcopy objdump ar ranlib nm; do
              cat > "$out/bin/riscv64-unknown-elf-$tool" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
for candidate in riscv64-none-elf-TOOL riscv64-elf-TOOL; do
  if command -v "$candidate" >/dev/null 2>&1; then
    exec "$candidate" "$@"
  fi
done
echo "riscv64-unknown-elf-TOOL: no suitable tool found in PATH" >&2
exit 127
EOF
              chmod +x "$out/bin/riscv64-unknown-elf-$tool"
              substituteInPlace "$out/bin/riscv64-unknown-elf-$tool" \
                --replace "TOOL" "$tool"
            done
          '';
        };
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            autoconf
            automake
            bison
            cacert
            cargo-binutils
            clang
            cmake
            curl
            dtc
            expat
            fish
            flex
            gawk
            gcc
            gdb
            git
            gmp
            gnumake
            gperf
            help2man
            libslirp
            libtool
            lldb
            mpfr
            mpc
            ncurses
            ninja
            nodejs
            openssh
            openssl
            patchutils
            pixman
            pkg-config
            python3
            python3Packages.pip
            python3Packages.tomli
            qemu
            readline
            rust-analyzer
            rustup
            SDL2
            sudo
            texinfo
            tmux
            unzip
            wget
            xz
            zlib
            glib
            llvmPackages.bintools
            llvmPackages.llvm
            riscv.buildPackages.binutils
            riscv.buildPackages.gcc
            riscv.buildPackages.gdb
            riscvAliases
            # LoongArch64 cross-compilation tools
            loongarch.buildPackages.binutils
            loongarch.buildPackages.gcc
            loongarch.buildPackages.gdb
          ];

          shellHook = ''
            export CARGO_HOME="''${CARGO_HOME:-$HOME/.cargo}"
            export RUSTUP_HOME="''${RUSTUP_HOME:-$HOME/.rustup}"
            export PATH="$CARGO_HOME/bin:$PATH"
            export RUSTUP_TOOLCHAIN="nightly-2025-10-28"

            if ! rustup toolchain list | grep -q "nightly-2025-10-28"; then
              echo "Rust nightly 2025-10-28 is not installed."
              echo "Run:"
              echo "  rustup toolchain install nightly-2025-10-28"
              echo "  rustup default nightly-2025-10-28"
              echo "  rustup component add rustfmt clippy rust-src rust-analyzer llvm-tools"
              echo "  rustup target add \\"
              echo "    riscv64gc-unknown-none-elf \\"
              echo "    riscv64imac-unknown-none-elf \\"
              echo "    loongarch64-unknown-linux-gnu \\"
              echo "    loongarch64-unknown-none \\"
              echo "    x86_64-unknown-linux-gnu"
            fi

            # Configure cargo sparse index (matching devcontainer setup)
            if [ ! -f "$CARGO_HOME/config.toml" ]; then
              mkdir -p "$CARGO_HOME"
              echo '[source.crates-io]' > "$CARGO_HOME/config.toml"
              echo 'registry = "sparse+https://index.crates.io/"' >> "$CARGO_HOME/config.toml"
            fi

            if [ -z "$FISH_VERSION" ]; then
              exec fish
            fi
          '';
        };
      });
}
