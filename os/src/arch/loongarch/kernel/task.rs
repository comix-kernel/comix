//! LoongArch64 任务相关（存根）

use super::context::TaskContext;
use crate::mm::frame_allocator::FrameTracker;

/// 初始化内核任务上下文
pub fn init_kernel_task_context(context: &mut TaskContext, entry: usize, kstack: usize) {
    context.ra = entry;
    context.sp = kstack;
}

/// 初始化 fork 后的上下文
pub fn init_fork_context(
    context: &mut TaskContext,
    kstack: usize,
    trap_frame_tracker: &FrameTracker,
) {
    // TODO: 实现
    context.sp = kstack;
    let _ = trap_frame_tracker;
}

/// 设置用户栈布局
/// 返回 (new_sp, argc, argv_ptr, envp_ptr)
pub fn setup_stack_layout(
    sp: usize,
    argv: &[&str],
    envp: &[&str],
    phdr_addr: usize,
    phnum: usize,
    phent: usize,
    entry_point: usize,
) -> (usize, usize, usize, usize) {
    use core::mem::size_of;
    use core::ptr;

    use alloc::vec::Vec;

    use crate::arch::constant::STACK_ALIGN_MASK;

    let mut sp = sp;
    let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
    let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());

    for &env in envp.iter().rev() {
        let bytes = env.as_bytes();
        sp -= bytes.len() + 1;
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), sp as *mut u8, bytes.len());
            (sp as *mut u8).add(bytes.len()).write(0);
        }
        env_ptrs.push(sp);
    }

    for &arg in argv.iter().rev() {
        let bytes = arg.as_bytes();
        sp -= bytes.len() + 1;
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), sp as *mut u8, bytes.len());
            (sp as *mut u8).add(bytes.len()).write(0);
        }
        arg_ptrs.push(sp);
    }

    sp &= !(size_of::<usize>() - 1);

    let random_bytes = [
        0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23,
        0x45, 0x67,
    ];
    let random_ptr = sp - 16;
    unsafe { ptr::copy_nonoverlapping(random_bytes.as_ptr(), random_ptr as *mut u8, 16) };
    sp = random_ptr;

    let platform = "loongarch64\0";
    let platform_len = platform.len();
    sp -= platform_len;
    unsafe { ptr::copy_nonoverlapping(platform.as_ptr(), sp as *mut u8, platform_len) };
    let platform_ptr = sp;

    sp &= !(size_of::<usize>() * 2 - 1);

    let execfn = if !arg_ptrs.is_empty() { arg_ptrs[0] } else { 0 };

    let auxv = [
        (3, phdr_addr),
        (4, phent),
        (5, phnum),
        (6, 4096),
        (7, 0),
        (8, 0),
        (9, entry_point),
        (11, 0),
        (12, 0),
        (13, 0),
        (14, 0),
        (15, platform_ptr),
        (16, 0),
        (17, 100),
        (23, 0),
        (25, random_ptr),
        (31, execfn),
        (0, 0),
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

    let total_size = auxv.len() * 2 * size_of::<usize>()
        + size_of::<usize>()
        + env_ptrs.len() * size_of::<usize>()
        + size_of::<usize>()
        + arg_ptrs.len() * size_of::<usize>()
        + size_of::<usize>();

    let sp_final = (sp - total_size) & !STACK_ALIGN_MASK;
    sp = sp_final + total_size;

    for (type_, val) in auxv.iter().rev() {
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *val) };
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *type_) };
    }

    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, 0);
    }

    for &p in env_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe {
            ptr::write(sp as *mut usize, p);
        }
    }
    let envp_vec_ptr = sp;

    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, 0);
    }

    for &p in arg_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe {
            ptr::write(sp as *mut usize, p);
        }
    }
    let argv_vec_ptr = sp;

    let argc = argv.len();
    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, argc);
    }

    (sp, argc, argv_vec_ptr, envp_vec_ptr)
}
