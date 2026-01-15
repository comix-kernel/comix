# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Comix is an educational/experimental operating system kernel written in Rust. It targets RISC-V 64-bit QEMU virt platform (LoongArch is scaffold work). The goal is to build a self-hosting, clean-structured kernel ecosystem compatible with a Linux ABI subset.

### Architecture

The kernel has a layered architecture:

- **Architecture Layer** (`os/src/arch/`): Architecture-specific code isolated under `arch/`. Use `cfg(target_arch)` only in `arch/mod.rs` to select between `riscv/` and `loongarch/`. All other code should access arch-specific functionality through `crate::arch::*` to avoid scattering `cfg` attributes.
  - `boot/`: Boot entry, CPU initialization, secondary CPU startup
  - `mm/`: Page tables, address translation
  - `trap/`: Trap handling, context switching
  - `intr/`: Interrupt handling, softirq, IPI
  - `syscall/`: System call entry points
  - `platform/`: Device tree, platform drivers (PLIC, UART, timer)

- **Kernel Subsystems** (`os/src/kernel/`):
  - `task/`: Task management, `TaskStruct`, process state
  - `scheduler/`: CPU scheduler, task switching
  - `syscall/`: System call implementations (portions arch-specific)
  - `cpu/`, `timer/`, `time/`: CPU and time management

- **Memory Management** (`os/src/mm/`):
  - `frame_allocator/`: Physical frame allocation
  - `global_allocator/`: Kernel heap allocator (talc)
  - `memory_space/`: Virtual address spaces (`MemorySpace`)
  - `page_table/`: Page table abstraction (SV39 for RISC-V)
  - `address/`: Address types (Vaddr, Paddr, Ppn)

- **Storage Layers**:
  - `os/src/vfs/`: Virtual File System with dentry cache, mount table, FD table, file locks
  - `os/src/fs/`: Concrete filesystem implementations (tmpfs, procfs, sysfs, ext4, simple_fs)
  - `os/src/device/`: Device drivers (VirtIO-MMIO, UART, RTC, block devices)

- **IPC** (`os/src/ipc/`): Signals, pipes, message queues, shared memory

- **User API** (`os/src/uapi/`): Linux-compatible types, errno, constants

- **User Programs** (`user/`): RISC-V ELF programs built and embedded into root filesystem

## Common Commands

### Building and Running

```bash
# Build kernel (automatically builds user programs and packs fs.img)
make build
cd os && make build

# Run in QEMU
make run
cd os && make run

# Build for specific architecture (riscv is default)
make build ARCH=loongarch
make run ARCH=loongarch

# Clean
make clean        # Clean OS build only
make clean-all    # Clean OS + user programs
```

### Testing

```bash
# Run kernel tests in QEMU
cd os && make test

# Debug tests (terminal 1)
cd os && make test-qemu

# Debug tests (terminal 2) - connect GDB
cd os && make test-gdb
```

### Debugging

```bash
# Terminal 1: Start QEMU waiting for GDB
cd os && make debug

# Terminal 2: Connect GDB
cd os && make gdb
```

### Linting and Formatting

```bash
# Format code
make fmt
cd os && cargo fmt

# Clippy
cd os && cargo clippy --target riscv64gc-unknown-none-elf

# Quick style check
cd os && make quick_check_style
```

### User Programs

User programs are automatically built by `build.rs`. Manual rebuild:

```bash
cd user && make
```

Add new user programs:
1. Create crate in `user/<prog>/`
2. Build produces ELF at `user/target/riscv64gc-unknown-none-elf/release/<prog>`
3. Programs are automatically packed into rootfs at `/home/user/bin/`

## Build System Details

### Build Pipeline

The `os/build.rs` script automatically:
1. Builds user programs via `make` in `user/` directory
2. Creates `simple_fs.img` (currently empty placeholder)
3. Creates ext4 test images for testing
4. Creates full runtime `fs-riscv.img` or `fs-loongarch.img` (4GB, first run is slow)

Environment variables:
- `ARCH`: Target architecture (riscv or loongarch)
- `TEST`: Set to 1 for test mode (creates smaller 8MB ext4 image)
- `LOG`: Path to save QEMU output log

### Architecture Targets

- **RISC-V**: `riscv64gc-unknown-none-elf` (default, mature)
- **LoongArch**: `loongarch64-unknown-none` (scaffold, WIP)

## Key Architecture Patterns

### Memory Space Design

The kernel uses a `MemorySpace` abstraction that represents a virtual address space:
- Created on fork/exec via `MemorySpace::new_user()`
- Activated via `mm::activate(root_ppn)` which writes to SATP (RISC-V)
- Global kernel space shared across all CPUs via `GLOBAL_KERNEL_SPACE`
- User mappings start at `USER_STACK_TOP` (configurable per arch)

### VFS Layering

The VFS has a four-layer architecture:
1. **FD Table Layer**: Per-process file descriptor table
2. **File Layer**: Session interface with offset/flags (`RegFile`, `PipeFile`, etc.)
3. **Dentry Layer**: Path resolution, mount points, directory cache
4. **Inode Layer**: Stateless storage operations

### Trap Handling Flow

1. Hardware trap → `entry.S` → save registers to `TrapFrame`
2. Jump to `rust_trap_handler`
3. Dispatch based on cause (syscall, interrupt, exception)
4. For syscalls: `trap::syscall_dispatch()` → `kernel::syscall::*`
5. Restore context via `trap::trap_return()`

### Boot Sequence

RISC-V (`arch/riscv/boot/`):
1. `entry.S` → Set stack pointer, jump to `rust_main`
2. `rust_main()` → `arch::boot::main()`
3. Early console, heap, frame allocator, page tables
4. Driver init (UART, PLIC, VirtIO devices)
5. Scheduler, task manager, filesystem mount
6. Spawn `/init` from rootfs

LoongArch follows similar pattern but is incomplete.

## Testing Conventions

- Unit tests in `*/tests/` submodules using `#[cfg(test)]`
- Integration tests run in QEMU via `make test`
- Test executables built with `TEST=1` environment variable
- Test ELF found via `find $(TARGET_DIR)/deps -name "os-*"`

## Code Style Guidelines

From `os/Cargo.toml` lints:
- `missing_docs = "deny"` - All public items must be documented
- `todo = "warn"` - TODO comments trigger clippy warnings
- `needless_borrow = "deny"` - Unnecessary reference operations
- `redundant_clone = "deny"` - Unnecessary clones

Documentation uses Chinese for explanatory comments, code/technical terms in English.

## Important File Locations

- `os/src/main.rs`: Kernel entry (`rust_main`)
- `os/src/config.rs`: Platform-agnostic constants
- `os/build.rs`: Build script for user programs and images
- `user/Makefile`: User program build rules
- `Makefile`: Top-level convenience targets
- `os/Makefile`: OS-level build/run/test targets
- `os/qemu-run.sh`: QEMU launch script
- `document/`: Design docs (mdBook structure)
