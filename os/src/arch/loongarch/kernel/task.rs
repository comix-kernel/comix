//! LoongArch64 任务相关

use alloc::vec::Vec;
use core::{mem::size_of, ptr};

use super::context::TaskContext;
use crate::{arch::constant::STACK_ALIGN_MASK, mm::frame_allocator::FrameTracker};

/// 初始化内核任务上下文
pub fn init_kernel_task_context(context: &mut TaskContext, entry: usize, kstack: usize) {
    context.ra = entry;
    context.sp = kstack;
}

/// 初始化 fork 后的上下文（当前仅设置栈指针）
pub fn init_fork_context(
    context: &mut TaskContext,
    kstack: usize,
    trap_frame_tracker: &FrameTracker,
) {
    context.sp = kstack;
    let _ = trap_frame_tracker;
}

/// 为新任务设置用户栈布局，包含命令行参数和环境变量
/// 返回新的栈指针位置，以及 argc, argv, envp 的地址
pub fn setup_stack_layout(
    sp: usize,
    argv: &[&str],
    envp: &[&str],
    phdr_addr: usize,
    phnum: usize,
    phent: usize,
    entry_point: usize,
) -> (usize, usize, usize, usize) {
    let mut sp = sp;
    let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
    let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());

    for &env in envp.iter().rev() {
        let bytes = env.as_bytes();
        sp -= bytes.len() + 1; // 预留 NUL
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), sp as *mut u8, bytes.len());
            (sp as *mut u8).add(bytes.len()).write(0); // NUL 终止符
        }
        env_ptrs.push(sp); // 存储字符串的地址
    }

    // 命令行参数 (argv)
    for &arg in argv.iter().rev() {
        let bytes = arg.as_bytes();
        sp -= bytes.len() + 1; // 预留 NUL
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), sp as *mut u8, bytes.len());
            (sp as *mut u8).add(bytes.len()).write(0); // NUL 终止符
        }
        arg_ptrs.push(sp); // 存储字符串的地址
    }

    // --- 对齐到字大小 (确保指针数组从对齐的地址开始) ---
    sp &= !(size_of::<usize>() - 1);

    // --- 构建 argc, argv, envp 数组 ---

    // AT_RANDOM 数据
    let random_bytes = [
        0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
        0x67,
    ];
    let random_ptr = sp - 16;
    unsafe { ptr::copy_nonoverlapping(random_bytes.as_ptr(), random_ptr as *mut u8, 16) };
    sp = random_ptr;

    // platform string
    let platform = "loongarch64\0";
    let platform_len = platform.len();
    sp -= platform_len;
    unsafe { ptr::copy_nonoverlapping(platform.as_ptr(), sp as *mut u8, platform_len) };
    let platform_ptr = sp;

    // 16 字节对齐（auxv 要求）
    sp &= !(size_of::<usize>() * 2 - 1);

    let execfn = arg_ptrs.last().copied().unwrap_or(0);

    let auxv = [
        (3, phdr_addr),     // AT_PHDR
        (4, phent),         // AT_PHENT
        (5, phnum),         // AT_PHNUM
        (6, 4096),          // AT_PAGESZ
        (7, 0),             // AT_BASE
        (8, 0),             // AT_FLAGS
        (9, entry_point),   // AT_ENTRY
        (11, 0),            // AT_UID
        (12, 0),            // AT_EUID
        (13, 0),            // AT_GID
        (14, 0),            // AT_EGID
        (15, platform_ptr), // AT_PLATFORM
        (16, 0),            // AT_HWCAP
        (17, 100),          // AT_CLKTCK
        (23, 0),            // AT_SECURE
        (25, random_ptr),   // AT_RANDOM
        (31, execfn),       // AT_EXECFN
        (0, 0),             // AT_NULL
    ];

    for (i, (k, v)) in auxv.iter().enumerate() {
        crate::pr_debug!("auxv[{}]: type={}, val={:#x}", i, k, v);
    }
    crate::pr_debug!(
        "setup_stack_layout: sp={:#x}, random_ptr={:#x}, phdr_addr={:#x}, entry={:#x}",
        sp,
        random_ptr,
        phdr_addr,
        entry_point
    );

    // 计算指针块大小确保最终 16 字节对齐
    let total_size = auxv.len() * 2 * size_of::<usize>()
        + size_of::<usize>() // envp NULL
        + env_ptrs.len() * size_of::<usize>()
        + size_of::<usize>() // argv NULL
        + arg_ptrs.len() * size_of::<usize>()
        + size_of::<usize>(); // argc

    let sp_final = (sp - total_size) & !STACK_ALIGN_MASK;
    sp = sp_final + total_size;

    for (type_, val) in auxv.iter().rev() {
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *val) };
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *type_) };
    }

    // envp NULL
    sp -= size_of::<usize>();
    unsafe { ptr::write(sp as *mut usize, 0) };

    for &p in env_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, p) };
    }
    let envp_vec_ptr = sp;

    // argv NULL
    sp -= size_of::<usize>();
    unsafe { ptr::write(sp as *mut usize, 0) };

    for &p in arg_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, p) };
    }
    let argv_vec_ptr = sp;

    let argc = argv.len();
    sp -= size_of::<usize>();
    unsafe { ptr::write(sp as *mut usize, argc) };

    (sp, argc, argv_vec_ptr, envp_vec_ptr)
}
