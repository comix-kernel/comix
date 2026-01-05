//! LoongArch64 系统调用模块

mod syscall_number;

pub use syscall_number::*;

use crate::arch::trap::TrapFrame;
use crate::kernel::syscall::*;
use crate::uapi::errno::ENOSYS;

/// 分发系统调用
pub fn dispatch_syscall(frame: &mut TrapFrame) {
    let syscall_id = frame.syscall_id();

    match syscall_id {
        nr::READ => sys_read(frame),
        nr::WRITE => sys_write(frame),
        nr::EXIT => sys_exit(frame),
        nr::EXIT_GROUP => sys_exit_group(frame),
        nr::BRK => sys_brk(frame),
        nr::MMAP => sys_mmap(frame),
        nr::MUNMAP => sys_munmap(frame),
        _ => {
            frame.set_syscall_ret((-ENOSYS) as usize);
        }
    }
}

/// 宏：实现系统调用函数的自动包装器 (LoongArch 版)
///
/// 该宏会生成一个名为 `sys_name` 的函数，该函数签名固定为 `(frame: &mut TrapFrame)`
/// 1. 从TrapFrame中提取参数
/// 2. 调用对应的系统调用处理函数
/// 3. 将返回值写回TrapFrame
#[cfg(target_arch = "loongarch64")]
#[macro_export]
macro_rules! impl_syscall {
    // noreturn, 0 args
    ($sys_name:ident, $kernel:path, noreturn, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(_frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            $kernel()
        }
    };
    // noreturn, 1 arg
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.regs[4] as $t0; // LoongArch a0 = $r4
            $kernel(a0)
        }
    };
    // noreturn, 2 args
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            $kernel(a0, a1)
        }
    };

    // returning, 0 args
    ($sys_name:ident, $kernel:path, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let ret = $kernel();
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 1 arg
    ($sys_name:ident, $kernel:path, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let ret = $kernel(a0);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 2 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let ret = $kernel(a0, a1);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 3 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let ret = $kernel(a0, a1, a2);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 4 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let ret = $kernel(a0, a1, a2, a3);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 5 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let a4 = frame.regs[8] as $t4;
            let ret = $kernel(a0, a1, a2, a3, a4);
            frame.regs[4] = ret as isize as usize;
        }
    };
    // returning, 6 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.regs[4] as $t0;
            let a1 = frame.regs[5] as $t1;
            let a2 = frame.regs[6] as $t2;
            let a3 = frame.regs[7] as $t3;
            let a4 = frame.regs[8] as $t4;
            let a5 = frame.regs[9] as $t5;
            let ret = $kernel(a0, a1, a2, a3, a4, a5);
            frame.regs[4] = ret as isize as usize;
        }
    };
}
