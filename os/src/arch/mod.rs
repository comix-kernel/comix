mod loongarch;
mod riscv;

#[cfg(target_arch = "loongarch64")]
pub use self::loongarch::*;

#[cfg(target_arch = "riscv64")]
pub use riscv::*;