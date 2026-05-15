//! LoongArch64 架构相关的启动代码

use core::arch::global_asm;

global_asm!(include_str!("entry.S"));

use crate::{
    arch::{intr, platform, timer, trap},
    earlyprintln,
    kernel::{self, current_cpu, time},
    mm,
    sync::PreemptGuard,
    test::run_early_tests,
};

/// LoongArch 主核启动入口
pub fn main(hartid: usize) {
    kernel::boot::clear_bss();

    // Enable base floating-point instructions (EUEN.FPE). Many LoongArch Linux-ABI
    // user programs are built with floating-point enabled and may execute FP
    // instructions very early during startup.
    loongArch64::register::euen::set_fpe(true);

    run_early_tests();

    earlyprintln!("[Boot] Hello, world!");
    earlyprintln!("[Boot] LoongArch CPU {} is up!", hartid);

    let kernel_space = mm::init();

    // 激活内核地址空间
    {
        let _guard = PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    #[cfg(test)]
    crate::test_main();

    // 早期引导陷阱（覆盖平台初始化窗口）
    trap::init_boot_trap();
    platform::init();
    time::init();
    timer::init();

    // 创建 idle 并设为当前任务（KScratch0 就绪）
    let idle = kernel::boot::create_idle_task(0, idle_loop);
    {
        let _guard = PreemptGuard::new();
        current_cpu().idle_task = Some(idle.clone());
        current_cpu().switch_task(idle);
    }

    // 完整陷阱处理（KScratch0 已有效）
    trap::init();

    // 创建 init 任务并入队（中断仍禁用，避免竞争）
    kernel::boot::rest_init();

    // 启用中断并进入 idle 循环
    // 时钟中断触发后调度器自动选中 init 并切换上下文
    unsafe { intr::enable_interrupts() };
    idle_loop();
}

/// Idle 循环：idle 0 等待中断
fn idle_loop() -> ! {
    loop {
        if !crate::arch::intr::are_interrupts_enabled() {
            unsafe {
                crate::arch::intr::enable_interrupts();
            }
        }
        unsafe {
            core::arch::asm!("idle 0");
        }
    }
}
