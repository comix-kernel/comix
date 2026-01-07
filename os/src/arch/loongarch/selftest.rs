//! LoongArch64 early self-tests (pre-mm)

use loongArch64::register::{CpuMode, crmd};

use crate::{early_test, kassert, println};

early_test!(loongarch_early_smoke, {
    // Basic CSR sanity: should be in PLV0 at boot.
    let plv = crmd::read().plv();
    kassert!(matches!(plv, CpuMode::Ring0));

    // Trap trampoline symbol should be linked in.
    let sigret = crate::arch::trap::sigreturn_trampoline_address();
    kassert!(sigret != 0);
    kassert!(sigret & 0x3 == 0);

    // Syscall numbers: asm-generic aligned sanity checks.
    kassert!(crate::arch::syscall::SYS_READ == 63);
    kassert!(crate::arch::syscall::SYS_WRITE == 64);
    kassert!(crate::arch::syscall::SYS_EXIT == 93);
});
