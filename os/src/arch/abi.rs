//! Architecture-specific ABI constants and ELF relocation classification.

/// RISC-V ELF machine number.
pub const EM_RISCV: u16 = 243;
/// LoongArch ELF machine number.
pub const EM_LOONGARCH: u16 = 258;

/// Absolute 64-bit relocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    /// Write `load_bias + addend`.
    Relative,
    /// Write `load_bias + symbol_value + addend`.
    Absolute64,
}

#[cfg(target_arch = "riscv64")]
const ELF_MACHINE: u16 = EM_RISCV;
#[cfg(target_arch = "loongarch64")]
const ELF_MACHINE: u16 = EM_LOONGARCH;
#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
const ELF_MACHINE: u16 = EM_RISCV;

#[cfg(target_arch = "loongarch64")]
const R_ABS64: u32 = 2;
#[cfg(target_arch = "loongarch64")]
const R_RELATIVE: u32 = 3;

#[cfg(not(target_arch = "loongarch64"))]
const R_ABS64: u32 = 2;
#[cfg(not(target_arch = "loongarch64"))]
const R_RELATIVE: u32 = 3;

/// Architecture-specific `getifaddrs` syscall number used by this kernel.
#[cfg(target_arch = "loongarch64")]
pub const SYS_GETIFADDRS: usize = 1000;
/// Architecture-specific `getifaddrs` syscall number used by this kernel.
#[cfg(not(target_arch = "loongarch64"))]
pub const SYS_GETIFADDRS: usize = 500;

/// Returns true if the ELF machine number matches the active target.
pub fn is_supported_elf_machine(machine: u16) -> bool {
    machine == ELF_MACHINE
}

/// Classifies an architecture-specific relocation type.
pub fn classify_relocation(r_type: u32) -> Option<RelocationKind> {
    match r_type {
        R_RELATIVE => Some(RelocationKind::Relative),
        R_ABS64 => Some(RelocationKind::Absolute64),
        _ => None,
    }
}

/// Resolves a relocation value once its optional symbol value is known.
pub fn resolve_relocation_value(
    kind: RelocationKind,
    load_bias: usize,
    symbol_value: usize,
    addend: isize,
) -> usize {
    let base = match kind {
        RelocationKind::Relative => load_bias,
        RelocationKind::Absolute64 => load_bias + symbol_value,
    };
    (base as isize + addend) as usize
}

/// Static `/proc/cpuinfo` content for the active target.
pub fn proc_cpuinfo_bytes() -> &'static [u8] {
    #[cfg(target_arch = "riscv64")]
    {
        b"processor\t: 0\n\
hart\t\t: 0\n\
isa\t\t: rv64imafdcsu\n\
mmu\t\t: sv39\n\
uarch\t\t: qemu,virt\n\n"
    }
    #[cfg(target_arch = "loongarch64")]
    {
        b"processor\t: 0\n\
arch\t\t: loongarch64\n\n"
    }
    #[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
    {
        b"processor\t: 0\narch\t\t: mock\n\n"
    }
}
