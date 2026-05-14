//! Arch trait 实现生成宏
//!
//! 为不同架构生成 `VirtualMemory` 和 `Arch` 的通用方法实现。
//! 两个架构的绝大多数方法完全相同，仅 `restart` 有架构差异。

/// 为指定架构生成 `VirtualMemory` impl 和 `Arch` impl 的通用方法。
///
/// `restart` 方法不在此宏中生成——每个架构需单独提供。
#[macro_export]
macro_rules! impl_arch_common {
    ($arch:ty, $process_space:ty, $kernel_space:ty) => {
        use $crate::hal::virtual_memory::VirtualMemory;
        use $crate::mm::address::Ppn;
        use $crate::sync::SpinLock;

        lazy_static::lazy_static! {
            static ref KERN_ADDR_SPACE: SpinLock<$kernel_space> =
                SpinLock::new(<$kernel_space>::new());
        }

        impl VirtualMemory for $arch {
            type PageTableRoot = Ppn;
            type ProcessAddressSpace = $process_space;
            type KernelAddressSpace = $kernel_space;

            const PAGE_OFFSET: usize = mm::VADDR_START;

            fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace> {
                &KERN_ADDR_SPACE
            }
        }

        impl $crate::hal::arch::Arch for $arch {
            type UserContext = kernel::context::Context;

            fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext {
                let mut ctx = kernel::context::Context::zero_init();
                ctx.set_init_context(entry_point, stack_top);
                ctx
            }

            unsafe fn context_switch(
                old: *mut Self::UserContext,
                new: *const Self::UserContext,
            ) {
                unsafe { kernel::switch(old, new) };
            }

            unsafe fn copy_from_user(
                src: usize,
                dst: *mut u8,
                len: usize,
            ) -> Result<(), ()> {
                if src > constant::USER_TOP
                    || src.checked_add(len).ok_or(())? > constant::USER_TOP + 1
                {
                    return Err(());
                }
                let _guard = trap::SumGuard::new();
                unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
                Ok(())
            }

            unsafe fn try_copy_from_user(
                src: usize,
                dst: *mut u8,
                len: usize,
            ) -> Result<(), ()> {
                unsafe { Self::copy_from_user(src, dst, len) }
            }

            unsafe fn copy_to_user(
                src: *const u8,
                dst: usize,
                len: usize,
            ) -> Result<(), ()> {
                if dst > constant::USER_TOP
                    || dst.checked_add(len).ok_or(())? > constant::USER_TOP + 1
                {
                    return Err(());
                }
                let _guard = trap::SumGuard::new();
                unsafe { core::ptr::copy_nonoverlapping(src, dst as *mut u8, len) };
                Ok(())
            }

            unsafe fn copy_strn_from_user(
                src: usize,
                dst: *mut u8,
                max_len: usize,
            ) -> Result<usize, ()> {
                if src > constant::USER_TOP {
                    return Err(());
                }
                let _guard = trap::SumGuard::new();
                let mut i = 0;
                while i < max_len {
                    let byte =
                        unsafe { core::ptr::read_volatile((src + i) as *const u8) };
                    unsafe { *dst.add(i) = byte };
                    if byte == 0 {
                        return Ok(i);
                    }
                    i += 1;
                }
                Ok(max_len)
            }

            fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize) {
                kernel::cpu::on_task_switch(trap_frame_ptr, cpu_ptr)
            }

            fn get_ticks() -> usize {
                timer::get_ticks()
            }
            fn get_time() -> usize {
                timer::get_time()
            }
            fn get_time_ms() -> usize {
                timer::get_time_ms()
            }
            fn clock_freq() -> usize {
                timer::clock_freq()
            }

            fn send_reschedule_ipi(target_cpu: usize) {
                ipi::send_reschedule_ipi(target_cpu)
            }

            fn name() -> &'static str {
                constant::ARCH
            }

            fn console_putchar(c: u8) {
                lib::console_putchar(c as usize);
            }

            fn console_getchar() -> Option<u8> {
                let ch = lib::console_getchar();
                if ch == usize::MAX {
                    None
                } else {
                    Some(ch as u8)
                }
            }

            fn cpu_count() -> usize {
                unsafe { $crate::kernel::NUM_CPU }
            }

            fn get_cmdline() -> Option<alloc::string::String> {
                Some($crate::device::CMDLINE.read().clone())
            }

            fn power_off() -> ! {
                lib::shutdown(false)
            }

            fn restart() -> ! {
                lib::restart()
            }
        }
    };
}
