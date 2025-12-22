//! LoongArch64 架构启动代码
//!
//! 启动流程：
//! 1. entry.S 配置 DMW 并跳转到虚拟地址
//! 2. rust_main 调用 main() 进行初始化
//! 3. 初始化内存管理、中断、定时器等子系统
//! 4. 创建第一个任务并开始调度

use core::arch::global_asm;

use crate::{
    arch::mm::{paddr_to_vaddr, vaddr_to_paddr},
    earlyprintln,
    test::run_early_tests,
};

global_asm!(include_str!("entry.S"));

/// LoongArch 架构初始化入口
/// 由 main.rs 中的 rust_main 调用
pub fn main(_hartid: usize) -> ! {
    clear_bss();

    run_early_tests();

    earlyprintln!("[Boot] Hello, LoongArch!");
    earlyprintln!("[Boot] LoongArch64 kernel is starting...");

    // 初始化内存管理
    crate::mm::init();

    #[cfg(test)]
    crate::test_main();

    // TODO: 初始化各子系统
    // - 中断处理
    // - 定时器
    // - 任务调度器

    earlyprintln!("[Boot] LoongArch64 initialization complete.");
    earlyprintln!("[Boot] Entering idle loop...");

    loop {
        // 暂时无限循环
        unsafe { core::arch::asm!("idle 0") };
    }
}

/// 清除 BSS 段，将其全部置零
/// BSS 段包含所有未初始化的静态变量
/// 在进入 Rust 代码之前调用此函数非常重要
///
/// 注意：此时已在虚拟地址空间运行，sbss/ebss 是虚拟地址
fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    // sbss 和 ebss 已经是虚拟地址（链接器脚本设置）
    // 转换为物理地址用于清零
    let sbss_paddr = unsafe { vaddr_to_paddr(sbss as usize) };
    let ebss_paddr = unsafe { vaddr_to_paddr(ebss as usize) };

    (sbss_paddr..ebss_paddr).for_each(|a| unsafe {
        // 访问物理地址需要通过 paddr_to_vaddr 转换
        let va = paddr_to_vaddr(a);
        (va as *mut u8).write_volatile(0)
    });
}
