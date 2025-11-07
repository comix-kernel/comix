use crate::arch::syscall::syscall_number::{SYS_EXIT, SYS_SHUTDOWN, SYS_WRITE};
use crate::kernel::syscall::*;

mod syscall_number;

pub fn dispatch_syscall(frame: &mut super::trap::TrapFrame) {
    match frame.x17_a7 {
        SYS_SHUTDOWN => sys_shutdown(frame),
        SYS_EXIT => sys_exit(frame),
        SYS_WRITE => sys_write(frame),
        _ => {
            // 未知的系统调用
            frame.x10_a0 = (-2isize) as usize; // -ENOSYS
            println!("Unknown syscall: {}", frame.x17_a7);
            panic!("Unknown syscall");
        }
    }
}

/// 宏：实现系统调用函数的自动包装器
///
/// 该宏会生成一个名为 `sys_name` 的函数，该函数签名固定为 `(frame: &mut TrapFrame)`
/// 1. 从TrapFrame中提取参数
/// 2. 调用对应的系统调用处理函数
/// 3. 将返回值写回TrapFrame
#[macro_export]
macro_rules! impl_syscall {
    // noreturn, 6 args
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let a5 = frame.x15_a5 as $t5;
            $kernel(a0, a1, a2, a3, a4, a5)
        }
    };

    // noreturn, 0..5 args (expanded)
    ($sys_name:ident, $kernel:path, noreturn, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(_frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            $kernel()
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            $kernel(a0)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            $kernel(a0, a1)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            $kernel(a0, a1, a2)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            $kernel(a0, a1, a2, a3)
        }
    };
    ($sys_name:ident, $kernel:path, noreturn, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) -> ! {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            $kernel(a0, a1, a2, a3, a4)
        }
    };

    // returning, 6 args
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty, $t5:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let a5 = frame.x15_a5 as $t5;
            let ret = $kernel(a0, a1, a2, a3, a4, a5);
            frame.x10_a0 = ret as isize as usize;
        }
    };

    // returning, 0..5 args (expanded)
    ($sys_name:ident, $kernel:path, ()) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let ret = $kernel();
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let ret = $kernel(a0);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let ret = $kernel(a0, a1);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut $crate::arch::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let ret = $kernel(a0, a1, a2);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let ret = $kernel(a0, a1, a2, a3);
            frame.x10_a0 = ret as isize as usize;
        }
    };
    ($sys_name:ident, $kernel:path, ($t0:ty, $t1:ty, $t2:ty, $t3:ty, $t4:ty)) => {
        #[allow(non_snake_case)]
        pub fn $sys_name(frame: &mut super::trap::TrapFrame) {
            let a0 = frame.x10_a0 as $t0;
            let a1 = frame.x11_a1 as $t1;
            let a2 = frame.x12_a2 as $t2;
            let a3 = frame.x13_a3 as $t3;
            let a4 = frame.x14_a4 as $t4;
            let ret = $kernel(a0, a1, a2, a3, a4);
            frame.x10_a0 = ret as isize as usize;
        }
    };
}
