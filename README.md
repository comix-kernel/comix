# comix
a common unix-like kernel for studing

## Development Environment Setup

You can set up the development environment in two ways:

### Option 1: Local Development (Recommended for macOS/Linux)

#### Prerequisites

##### 1. Rust Toolchain

This project uses a specific Rust nightly toolchain with required components. The toolchain configuration is specified in `os/rust-toolchain.toml`, and rustup will automatically install the correct version and components when you enter the `os/` directory.

Required components (auto-installed):
- `rust-src` - Rust standard library source code
- `rustfmt` - Code formatter
- `clippy` - Linter
- `llvm-tools` - LLVM tools (including objcopy)

Target (auto-installed):
- `riscv64gc-unknown-none-elf` - RISC-V 64-bit bare-metal target

##### 2. QEMU

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

##### 3. cargo-binutils

Required for binary manipulation (rust-objcopy, etc.):
```bash
cargo install cargo-binutils
```

### Option 2: Docker/Dev Container (Recommended for consistency)

This project provides a complete Docker-based development environment with all dependencies pre-installed.

#### What's Included in the Container

- ✅ Ubuntu 24.04 base image
- ✅ Rust nightly-2025-10-28 with all required components
- ✅ QEMU 9.2.1 (RISC-V + LoongArch support)
- ✅ GDB 13.1 (RISC-V + LoongArch support)
- ✅ cargo-binutils
- ✅ All build dependencies
- ✅ Fish shell with tmux
- ✅ VS Code extensions (rust-analyzer, LLDB debugger, etc.)

#### Using VS Code Dev Containers

**Prerequisites:**
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) or Docker Engine
- [Visual Studio Code](https://code.visualstudio.com/)
- [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)

**Steps:**

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd comix
   ```

2. Open in VS Code:
   ```bash
   code .
   ```

3. When prompted, click **"Reopen in Container"** or:
   - Press `F1`
   - Type "Dev Containers: Reopen in Container"
   - Press Enter

4. Wait for the container to build (first time only, ~5-10 minutes)

5. Once inside the container, open a terminal and run:
   ```bash
   cd os
   make run
   ```

#### Using Docker CLI Directly

If you prefer to use Docker without VS Code:

**Build the Docker image:**
```bash
docker build -t comix-dev .
```

**Run the container:**
```bash
docker run -it --rm \
  -v $(pwd):/workspace \
  -w /workspace \
  --network=host \
  comix-dev
```

**Inside the container:**
```bash
cd os
make run
```

#### Dev Container Features

The `.devcontainer/devcontainer.json` configuration includes:

- **SSH Key Mounting**: Your `~/.ssh` directory is mounted for git operations
- **Network Access**: `--network=host` for proxy support
- **VS Code Extensions**: Pre-configured with Rust development tools
- **Post-Create Command**: Automatically runs `rustup show` to verify setup
- **Safe Directory**: Git is configured to trust the workspace directory

#### Proxy Configuration

If you need to use a proxy, the container is pre-configured with proxy settings. You can modify them in `.devcontainer/devcontainer.json`:

```json
"remoteEnv": {
  "HTTP_PROXY": "http://127.0.0.1:7890",
  "HTTPS_PROXY": "http://127.0.0.1:7890"
}
```

Or disable them by commenting out the `remoteEnv` section.

## Quick Start

### Local Development

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

### Docker/Dev Container

```bash
# Option 1: VS Code (recommended)
code .
# Then: F1 → "Dev Containers: Reopen in Container"

# Option 2: Docker CLI
docker build -t comix-dev .
docker run -it --rm -v $(pwd):/workspace -w /workspace comix-dev
cd os && make run
```

## Project Structure

```
comix/
├── .devcontainer/
│   └── devcontainer.json    # VS Code Dev Container configuration
├── Dockerfile               # Docker image for development environment
├── bootloader/
│   └── rustsbi-qemu.bin    # RustSBI bootloader
└── os/
    ├── src/                 # Kernel source code
    ├── Cargo.toml           # Rust project configuration
    ├── rust-toolchain.toml  # Rust toolchain specification
    ├── Makefile             # Build and run targets
    └── qemu-run.sh          # QEMU launch script
```

## Troubleshooting

### Local Development

**Issue: `rust-objcopy` not found**
```bash
rustup component add llvm-tools --toolchain nightly-2025-10-28
```

**Issue: QEMU not found**
```bash
# macOS
brew install qemu

# Linux
sudo apt-get install qemu-system-misc
```

### Docker/Dev Container

**Issue: Container build fails**
- Ensure Docker is running
- Check your internet connection (container downloads ~2GB of dependencies)
- Try building with `docker build --no-cache -t comix-dev .`

**Issue: Permission denied inside container**
```bash
# The container runs as user 'vscode' with sudo access
sudo <command>
```

**Issue: Changes not persisted**
- Ensure you're mounting the project directory: `-v $(pwd):/workspace`
- Work inside `/workspace` directory in the container
