//! LoongArch64 架构模块
//!
//! 包含 LoongArch64 处理器架构相关的实现。

pub mod boot;
pub mod constant;
pub mod cpu_ops;
pub mod intr;
pub mod ipi;
pub mod kernel;
pub mod lib;
pub mod memory;
pub mod mm;
pub mod platform;
pub mod syscall;
pub mod timer;
pub mod trap;

use crate::hal::virtual_memory::VirtualMemory;
use crate::mm::address::Ppn;
use crate::sync::SpinLock;
use memory::LoongArch64ProcessAddressSpace;
use memory::LoongArch64KernelAddressSpace;
use lazy_static::lazy_static;

lazy_static! {
    static ref KERN_ADDR_SPACE: SpinLock<LoongArch64KernelAddressSpace> =
        SpinLock::new(LoongArch64KernelAddressSpace);
}

impl VirtualMemory for cpu_ops::LoongArch64 {
    type PageTableRoot = Ppn;
    type ProcessAddressSpace = LoongArch64ProcessAddressSpace;
    type KernelAddressSpace = LoongArch64KernelAddressSpace;

    const PAGE_OFFSET: usize = mm::VADDR_START;

    fn kern_address_space() -> &'static SpinLock<Self::KernelAddressSpace> {
        &KERN_ADDR_SPACE
    }
}

use crate::hal::arch::Arch;
use kernel::context::Context;

impl Arch for cpu_ops::LoongArch64 {
    type UserContext = Context;

    fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext {
        let mut ctx = Context::zero_init();
        ctx.set_init_context(entry_point, stack_top);
        ctx
    }

    unsafe fn context_switch(old: *mut Self::UserContext, new: *const Self::UserContext) {
        unsafe { kernel::switch(old, new) };
    }

    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
        if src > constant::USER_TOP || src.checked_add(len).ok_or(())? > constant::USER_TOP + 1 {
            return Err(());
        }
        let _guard = trap::SumGuard::new();
        unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
        Ok(())
    }

    unsafe fn try_copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
        Self::copy_from_user(src, dst, len)
    }

    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
        if dst > constant::USER_TOP || dst.checked_add(len).ok_or(())? > constant::USER_TOP + 1 {
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
            let byte = unsafe { core::ptr::read_volatile((src + i) as *const u8) };
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
        unsafe { crate::kernel::NUM_CPU }
    }

    fn get_cmdline() -> Option<alloc::string::String> {
        Some(crate::device::CMDLINE.read().clone())
    }

    fn power_off() -> ! {
        lib::shutdown(false)
    }

    fn restart() -> ! {
        lib::restart()
    }
}
