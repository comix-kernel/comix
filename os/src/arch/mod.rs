mod loongarch;
mod riscv;

#[cfg(target_arch = "loongarch64")]
pub use self::loongarch::mm;

#[cfg(target_arch = "riscv64")]
pub use self::riscv::mm;