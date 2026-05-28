//! Arch / Platform trait 实现生成宏
//!
//! 为不同架构生成 `VirtualMemory`、`Arch` 和 `Platform` 的通用方法实现。
//! 两个架构的绝大多数方法完全相同，可通过宏复用。

/// 为指定架构生成 `VirtualMemory` impl 和 `Arch` impl。
#[macro_export]
macro_rules! impl_arch {
    ($arch:ty, $process_space:ty, $kernel_space:ty) => {
        use $crate::arch::virtual_memory::VirtualMemory;
        use $crate::mm::address::Ppn;
        use $crate::mm::page_table::PagingError;
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
            const USER_TOP: usize = constant::USER_TOP;

            fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace> {
                &KERN_ADDR_SPACE
            }
        }

        impl $crate::arch::arch::Arch for $arch {
            type UserContext = kernel::context::Context;
            type TrapFrame = trap::TrapFrame;

            fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext {
                let mut ctx = kernel::context::Context::zero_init();
                ctx.set_init_context(entry_point, stack_top);
                ctx
            }

            unsafe fn context_switch(old: *mut Self::UserContext, new: *const Self::UserContext) {
                unsafe { kernel::switch(old, new) };
            }

            unsafe fn copy_from_user(
                src: $crate::arch::address::UA,
                dst: *mut u8,
                len: usize,
            ) -> Result<(), PagingError> {
                let src = src.as_usize();
                validate_user_copy_range(src, len, false)?;
                if len != 0 && dst.is_null() {
                    return Err(PagingError::InvalidAddress);
                }
                let _guard = trap::SumGuard::new();
                unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
                Ok(())
            }

            unsafe fn try_copy_from_user(
                src: $crate::arch::address::UA,
                dst: *mut u8,
                len: usize,
            ) -> Result<(), PagingError> {
                unsafe { Self::copy_from_user(src, dst, len) }
            }

            unsafe fn copy_to_user(
                src: *const u8,
                dst: $crate::arch::address::UA,
                len: usize,
            ) -> Result<(), PagingError> {
                let dst = dst.as_usize();
                validate_user_copy_range(dst, len, true)?;
                if len != 0 && src.is_null() {
                    return Err(PagingError::InvalidAddress);
                }
                let _guard = trap::SumGuard::new();
                unsafe { core::ptr::copy_nonoverlapping(src, dst as *mut u8, len) };
                Ok(())
            }

            unsafe fn copy_strn_from_user(
                src: $crate::arch::address::UA,
                dst: *mut u8,
                max_len: usize,
            ) -> Result<usize, PagingError> {
                let src = src.as_usize();
                if !(constant::USER_BASE..=<$arch as VirtualMemory>::USER_TOP).contains(&src) {
                    return Err(PagingError::InvalidAddress);
                }
                if max_len != 0 && dst.is_null() {
                    return Err(PagingError::InvalidAddress);
                }
                let _guard = trap::SumGuard::new();
                let mut i = 0;
                while i < max_len {
                    let cur = src.checked_add(i).ok_or(PagingError::InvalidAddress)?;
                    validate_user_copy_range(cur, 1, false)?;
                    let byte = unsafe { core::ptr::read_volatile(cur as *const u8) };
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

            fn cpu_count() -> usize {
                $crate::kernel::num_cpu()
            }
        }

        fn validate_user_copy_range(
            start: usize,
            len: usize,
            write: bool,
        ) -> Result<(), PagingError> {
            use $crate::mm::address::{PageNum, VA, Vpn};
            use $crate::mm::page_table::{PageTableInner, UniversalPTEFlag};

            if len == 0 {
                return Ok(());
            }
            if !(constant::USER_BASE..=<$arch as VirtualMemory>::USER_TOP).contains(&start) {
                return Err(PagingError::InvalidAddress);
            }
            let end = start.checked_add(len).ok_or(PagingError::InvalidAddress)?;
            let last = end.checked_sub(1).ok_or(PagingError::InvalidAddress)?;
            if last > <$arch as VirtualMemory>::USER_TOP {
                return Err(PagingError::InvalidAddress);
            }

            let space = $crate::kernel::current_memory_space();
            let guard = space.lock();
            let mut cur = start;
            while cur < end {
                let vpn = Vpn::from_addr_floor(VA::from_usize(cur));
                let (_, _, flags) = guard.page_table().walk(vpn)?;
                let required = UniversalPTEFlag::VALID | UniversalPTEFlag::USER_ACCESSIBLE;
                if !flags.contains(required) {
                    return Err(PagingError::PermissionDenied);
                }
                if write {
                    if !flags.contains(UniversalPTEFlag::WRITEABLE) {
                        return Err(PagingError::PermissionDenied);
                    }
                } else if !flags.contains(UniversalPTEFlag::READABLE) {
                    return Err(PagingError::PermissionDenied);
                }
                let next_page = (cur & !($crate::config::PAGE_SIZE - 1))
                    .checked_add($crate::config::PAGE_SIZE)
                    .ok_or(PagingError::InvalidAddress)?;
                cur = core::cmp::min(next_page, end);
            }
            Ok(())
        }
    };
}

/// 为指定架构生成 `Platform` impl。
///
/// 此宏依赖 `lib` 和 `device` 模块提供底层实现。
#[macro_export]
macro_rules! impl_platform {
    ($arch:ty) => {
        impl $crate::arch::plat::Platform for $arch {
            fn console_putchar(c: u8) {
                lib::console_putchar(c);
            }

            fn console_getchar() -> Option<u8> {
                let ch = lib::console_getchar();
                if ch == usize::MAX {
                    None
                } else {
                    Some(ch as u8)
                }
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
