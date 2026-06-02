//! ComixOS - A multi-architecture operating system kernel
//!
//! This is the main crate for ComixOS, an operating system kernel written in Rust
//! for RISC-V and LoongArch architectures. It provides basic OS functionalities
//! including memory management, process scheduling, and system call handling.

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
// 下列特性在 nightly-2025-10-28 已稳定，但本项目固定的 nightly-2025-01-18
// (1.86-nightly，与评测机/参考项目 SanktaOS 对齐) 仍需显式开启。
#![feature(let_chains)]
#![feature(trait_upcasting)]
#![feature(unsigned_is_multiple_of)]
#![feature(ip_from)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

// === 基础层（始终编译） ===
mod arch;
mod config;
mod console;
mod kernel;
mod sync;
mod test;

// 轻量工具模块（仅依赖基础层）
mod uapi;
mod util;

// 日志模块：console 在 device 不可用时回退到 arch 路径
#[macro_use]
mod log;

mod device;
mod fs;
mod ipc;
mod mm;
mod net;
mod security;
mod vfs;

use crate::arch::Platform;
use core::panic::PanicInfo;
#[cfg(test)]
use test::test_runner;

/// Rust 内核主入口点
///
/// 这是从汇编代码跳转到的第一个 Rust 函数。它负责初始化内核的所有子系统,
/// 包括内存管理、中断处理、定时器和任务调度器。
///
/// # Safety
///
/// 此函数标记为 `#[unsafe(no_mangle)]` 以确保链接器可以找到它。
/// 它必须从正确初始化的汇编入口点调用。
#[unsafe(no_mangle)]
pub extern "C" fn rust_main(_hartid: usize) -> ! {
    arch::boot::main(_hartid);
    unreachable!("Unreachable in rust_main()");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        crate::console::emergency_print(format_args!(
            "Panicked at {}:{} {}\n",
            location.file(),
            location.line(),
            info.message()
        ));
    } else {
        crate::console::emergency_print(format_args!("Panicked: {}\n", info.message()));
    }

    crate::arch::ArchImpl::power_off()
}
