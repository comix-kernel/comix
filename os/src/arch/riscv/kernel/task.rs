//! RISC-V 架构的任务管理相关功能
use core::mem::size_of;
use core::ptr;

use alloc::vec::Vec;
use riscv::register::sstatus;

use crate::arch::constant::STACK_ALIGN_MASK;
use crate::arch::{
    address::VA,
    task::{ExecStackLayout, ExecTlsTemplate},
};
use crate::mm::memory_space::MemorySpace;

/// 为新任务设置用户栈布局，包含命令行参数和环境变量
/// 返回新的栈指针位置，以及 argc, argv, envp 的地址
pub fn setup_stack_layout(
    _space: &MemorySpace,
    sp: usize,
    argv: &[&str],
    envp: &[&str],
    phdr_addr: usize,
    phnum: usize,
    phent: usize,
    at_base: usize,
    at_entry: usize,
    _tls: Option<ExecTlsTemplate>,
) -> (usize, usize, usize, usize, usize) {
    // Linux leaves tp zero across execve on RISC-V. Static glibc builds the
    // initial TLS/TCB itself from PT_TLS and auxv; pre-seeding tp here can make
    // early pointer-guard users observe a different TCB from the final one.
    let tls_tp = 0usize;
    let mut sp = sp;
    let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
    let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());
    unsafe {
        sstatus::set_sum();
    }

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

    // --- 构建 argc, argv, envp 数组 (ABI 标准布局: [argc] -> [argv] -> [NULL] -> [envp] -> [NULL]) ---
    // 注意：栈向下增长，所以压栈顺序是从 envp NULL 往回压到 argc

    // 0. 写入 auxv (Auxiliary Vector)
    // 必须位于 envp NULL 之后（高地址），但在 envp 数组之前。
    // 常见的 auxv 条目：AT_PAGESZ(6), AT_NULL(0), AT_RANDOM(25)
    let random_bytes = [
        0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
        0x67,
    ];
    let random_ptr = sp - 16;
    unsafe { ptr::copy_nonoverlapping(random_bytes.as_ptr(), random_ptr as *mut u8, 16) };
    sp = random_ptr;

    // 2. Platform string "riscv64\0" (8 bytes)
    let platform = "riscv64\0";
    let platform_len = platform.len();
    sp -= platform_len;
    unsafe { ptr::copy_nonoverlapping(platform.as_ptr(), sp as *mut u8, platform_len) };
    let platform_ptr = sp;

    // 3. Align sp to 16 bytes (auxv requirement)
    sp &= !(size_of::<usize>() * 2 - 1); // Align to 16 bytes

    // 4. AT_EXECFN (use argv[0] if available)
    let execfn = arg_ptrs.last().copied().unwrap_or(0);

    let auxv = [
        (3, phdr_addr),     // AT_PHDR
        (4, phent),         // AT_PHENT
        (5, phnum),         // AT_PHNUM
        (6, 4096),          // AT_PAGESZ
        (7, at_base),       // AT_BASE
        (8, 0),             // AT_FLAGS
        (9, at_entry),      // AT_ENTRY
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

    // Debug print auxv
    for (i, (k, v)) in auxv.iter().enumerate() {
        crate::pr_debug!("auxv[{}]: type={}, val={:#x}", i, k, v);
    }
    crate::pr_debug!(
        "setup_stack_layout: sp={:#x}, random_ptr={:#x}, phdr_addr={:#x}, entry={:#x}",
        sp,
        random_ptr,
        phdr_addr,
        at_entry
    );

    // Calculate total size of the pointer block to ensure final sp is 16-byte aligned
    // Block includes: auxv[], padding, envp NULL, envp[], argv NULL, argv[], argc
    let total_size = auxv.len() * 2 * size_of::<usize>()
        + size_of::<usize>() // envp NULL
        + env_ptrs.len() * size_of::<usize>()
        + size_of::<usize>() // argv NULL
        + arg_ptrs.len() * size_of::<usize>()
        + size_of::<usize>(); // argc

    // Align the final stack pointer
    let sp_final = (sp - total_size) & !STACK_ALIGN_MASK;
    sp = sp_final + total_size;

    for (type_, val) in auxv.iter().rev() {
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *val) };
        sp -= size_of::<usize>();
        unsafe { ptr::write(sp as *mut usize, *type_) };
    }

    // 1. 写入 envp NULL 终止符
    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, 0);
    }

    // 2. 写入 envp 指针数组（逆序写入，使 envp[0] 处于最低地址）
    // env_ptrs 已经是逆序 (envp[n-1] ... envp[0])
    for &p in env_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe {
            ptr::write(sp as *mut usize, p);
        }
    }
    let envp_vec_ptr = sp; // envp 数组的起始地址 (envp[0] 的地址)

    // 3. 写入 argv NULL 终止符
    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, 0);
    }

    // 4. 写入 argv 指针数组（逆序写入，使 argv[0] 处于最低地址）
    // arg_ptrs 已经是逆序 (argv[n-1] ... argv[0])
    for &p in arg_ptrs.iter() {
        sp -= size_of::<usize>();
        unsafe {
            ptr::write(sp as *mut usize, p);
        }
    }
    let argv_vec_ptr = sp; // argv 数组的起始地址 (argv[0] 的地址)

    // 5. 写入 argc
    let argc = argv.len();
    sp -= size_of::<usize>();
    unsafe {
        ptr::write(sp as *mut usize, argc);
    }

    // 拷贝完成，恢复 SUM
    unsafe {
        sstatus::clear_sum();
    }

    // 6. 最终 sp 应该已经是 16 字节对齐的
    // sp &= !STACK_ALIGN_MASK;
    (sp, argc, argv_vec_ptr, envp_vec_ptr, tls_tp)
}

/// Architecture-neutral wrapper for `execve` stack setup.
pub fn setup_exec_stack_layout(
    space: &MemorySpace,
    sp: VA,
    argv: &[&str],
    envp: &[&str],
    phdr_addr: VA,
    phnum: usize,
    phent: usize,
    at_base: VA,
    at_entry: VA,
    tls: Option<ExecTlsTemplate>,
) -> ExecStackLayout {
    let (sp, argc, argv, envp, tls) = setup_stack_layout(
        space,
        sp.as_usize(),
        argv,
        envp,
        phdr_addr.as_usize(),
        phnum,
        phent,
        at_base.as_usize(),
        at_entry.as_usize(),
        tls,
    );
    ExecStackLayout {
        sp: VA::from_usize(sp),
        argc,
        argv: VA::from_usize(argv),
        envp: VA::from_usize(envp),
        tls: VA::from_usize(tls),
    }
}

/// Restore a freshly scheduled task for the first time.
pub unsafe fn forkret_restore(tf_ptr: *mut crate::arch::trap::TrapFrame, _is_kernel_thread: bool) {
    unsafe { crate::arch::trap::restore(&*tf_ptr) };
}

/// Initialize the trap frame used to enter a kernel task.
pub unsafe fn init_kernel_trap_frame(
    tf_ptr: *mut crate::arch::trap::TrapFrame,
    entry: usize,
    terminal: usize,
    kernel_sp: usize,
) {
    unsafe {
        core::ptr::write(tf_ptr, crate::arch::trap::TrapFrame::zero_init());
        (*tf_ptr).set_kernel_trap_frame(entry, terminal, kernel_sp);
        let cpu_ptr = {
            let _guard = crate::sync::PreemptGuard::new();
            crate::kernel::current_cpu() as *const _ as usize
        };
        crate::arch::trap::set_trap_frame_cpu_ptr(tf_ptr, cpu_ptr);
    }
}

/// Final architecture-specific preparation before restoring to user mode.
pub unsafe fn prepare_user_restore(
    tfp: *mut crate::arch::trap::TrapFrame,
    _initial_pc: VA,
    _user_sp_high: VA,
) {
    unsafe {
        crate::pr_info!(
            "[kernel_execve] trapframe: sepc={:#x}, sp={:#x}, sstatus={:#x}, a0={:#x}, a1={:#x}, a2={:#x}",
            (*tfp).sepc,
            (*tfp).x2_sp,
            (*tfp).sstatus,
            (*tfp).x10_a0,
            (*tfp).x11_a1,
            (*tfp).x12_a2,
        );
    }
}

fn write_user_usize(space: &MemorySpace, dst: usize, val: usize) {
    let paddr = space
        .translate(VA::from_usize(dst))
        .expect("write_user_usize: translate failed");
    unsafe {
        ptr::write(crate::arch::pa_to_va(paddr).as_usize() as *mut usize, val);
    }
}
