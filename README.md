# comix
a common unix-like kernel for studing

## Prerequisites

### 1. Rust Toolchain

This project uses a specific Rust nightly toolchain with required components. The toolchain configuration is specified in `os/rust-toolchain.toml`, and rustup will automatically install the correct version and components when you enter the `os/` directory.

Required components (auto-installed):
- `rust-src` - Rust standard library source code
- `rustfmt` - Code formatter
- `clippy` - Linter
- `llvm-tools` - LLVM tools (including objcopy)

Target (auto-installed):
- `riscv64gc-unknown-none-elf` - RISC-V 64-bit bare-metal target

### 2. QEMU

You need QEMU with RISC-V support to run the OS.

**macOS:**
```bash
brew install qemu
```

**Ubuntu/Debian:**
```bash
sudo apt-get install qemu-system-misc
```

**Build from source (recommended for CI):**
```bash
wget https://download.qemu.org/qemu-9.2.1.tar.xz
tar -xf qemu-9.2.1.tar.xz
cd qemu-9.2.1
./configure --target-list=riscv64-softmmu
make -j$(nproc)
sudo make install
```

### 3. cargo-binutils

Required for binary manipulation (rust-objcopy, etc.):
```bash
cargo install cargo-binutils
```

## Quick Start

```bash
# Clone the repository
git clone <repository-url>
cd comix/os

# The first time you enter the directory, rustup will automatically
# install the required toolchain and components specified in rust-toolchain.toml

# Build and run
make run

# Run tests
make test

# Debug mode (in one terminal)
make debug
# Then in another terminal:
make gdb
```

## Project Structure

```
comix/
├── bootloader/
│   └── rustsbi-qemu.bin    # RustSBI bootloader
└── os/
    ├── src/                 # Kernel source code
    ├── Cargo.toml           # Rust project configuration
    ├── rust-toolchain.toml  # Rust toolchain specification
    ├── Makefile             # Build and run targets
    └── qemu-run.sh          # QEMU launch script
```
