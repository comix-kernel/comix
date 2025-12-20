//! LoongArch64 架构启动代码

use core::arch::global_asm;

global_asm!(include_str!("entry.S"));

/// LoongArch 架构初始化入口
/// 由 main.rs 中的 rust_main 调用
pub fn main(_hartid: usize) -> ! {
    clear_bss();

    crate::earlyprintln!("[Boot] Hello, LoongArch!");
    crate::earlyprintln!("[Boot] LoongArch64 kernel is starting...");

    // TODO: 初始化各子系统
    // - 内存管理
    // - 中断处理
    // - 定时器
    // - 任务调度器

    loop {
        // 暂时无限循环
        unsafe { core::arch::asm!("idle 0") };
    }
}

/// 清除 BSS 段
fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    unsafe {
        let start = sbss as usize;
        let end = ebss as usize;
        (start..end).for_each(|a| (a as *mut u8).write_volatile(0));
    }
}
