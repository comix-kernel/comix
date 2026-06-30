//! LoongArch64 任务相关

use alloc::vec::Vec;
use core::{mem::size_of, ptr};

use super::context::TaskContext;
use crate::{
    arch::{
        address::VA,
        constant::STACK_ALIGN_MASK,
        task::{ExecStackLayout, ExecTlsTemplate},
    },
    config::PAGE_SIZE,
    mm::{frame_allocator::FrameTracker, memory_space::MemorySpace},
};

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
    space: &MemorySpace,
    sp: usize,
    argv: &[&str],
    envp: &[&str],
    phdr_addr: usize,
    phnum: usize,
    phent: usize,
    at_base: usize,
    at_entry: usize,
) -> (usize, usize, usize, usize, usize) {
    // Reserve one page at the top of the user stack for TLS/TCB, and set $tp to
    // a stable address inside that page. This is required by many Linux-ABI user
    // programs on LoongArch which rely on $tp for TLS.
    // TLS lives in the top stack page: [tls_base, tls_base + PAGE_SIZE).
    let tls_page_size = PAGE_SIZE;
    let tls_base = (sp - 1) & !(tls_page_size - 1);
    // Place tp near the top of that page, 16-byte aligned, and within the mapping.
    let tls_tp = (sp & !0xf).wrapping_sub(0x10);

    // Ensure the TLS page is mapped (it should be within the mapped user stack range),
    // and initialize a minimal self-pointer at tp for libc expectations.
    write_user_usize(space, tls_tp, tls_tp);

    // Start placing argv/envp/auxv below the TLS page.
    let mut sp = tls_base;
    crate::pr_debug!(
        "[setup_stack_layout] sp_top=0x{:x}, phdr=0x{:x}, entry=0x{:x}",
        sp,
        phdr_addr,
        at_entry
    );
    let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
    let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());

    for &env in envp.iter().rev() {
        let bytes = env.as_bytes();
        sp -= bytes.len() + 1; // 预留 NUL
        write_user_bytes(&space, sp, bytes);
        write_user_bytes(&space, sp + bytes.len(), &[0]); // NUL 终止符
        env_ptrs.push(sp); // 存储字符串的地址
    }

    // 命令行参数 (argv)
    for &arg in argv.iter().rev() {
        let bytes = arg.as_bytes();
        sp -= bytes.len() + 1; // 预留 NUL
        write_user_bytes(&space, sp, bytes);
        write_user_bytes(&space, sp + bytes.len(), &[0]); // NUL 终止符
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
    write_user_bytes(&space, random_ptr, &random_bytes);
    sp = random_ptr;

    // platform string
    let platform = "loongarch64\0";
    let platform_len = platform.len();
    sp -= platform_len;
    write_user_bytes(&space, sp, platform.as_bytes());
    let platform_ptr = sp;

    // 16 字节对齐（auxv 要求）
    sp &= !(size_of::<usize>() * 2 - 1);

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
        write_user_usize(&space, sp, *val);
        sp -= size_of::<usize>();
        write_user_usize(&space, sp, *type_);
    }

    // envp NULL
    sp -= size_of::<usize>();
    write_user_usize(&space, sp, 0);

    for &p in env_ptrs.iter() {
        sp -= size_of::<usize>();
        write_user_usize(&space, sp, p);
    }
    let envp_vec_ptr = sp;

    // argv NULL
    sp -= size_of::<usize>();
    write_user_usize(&space, sp, 0);

    for &p in arg_ptrs.iter() {
        sp -= size_of::<usize>();
        write_user_usize(&space, sp, p);
    }
    let argv_vec_ptr = sp;

    let argc = argv.len();
    sp -= size_of::<usize>();
    write_user_usize(&space, sp, argc);

    crate::pr_debug!(
        "[setup_stack_layout] sp_final=0x{:x}, argc={}, argv=0x{:x}, envp=0x{:x}",
        sp,
        argc,
        argv_vec_ptr,
        envp_vec_ptr
    );
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
    _tls: Option<ExecTlsTemplate>,
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
    if unsafe { is_kernel_entry(tf_ptr) } {
        let (entry, sp, ra) = unsafe { ((*tf_ptr).era, (*tf_ptr).kernel_sp, (*tf_ptr).regs[1]) };
        unsafe {
            core::arch::asm!(
                "addi.d $sp, {sp}, 0",
                "addi.d $ra, {ra}, 0",
                "jirl $zero, {entry}, 0",
                sp = in(reg) sp,
                ra = in(reg) ra,
                entry = in(reg) entry,
                options(noreturn)
            );
        }
    }
    crate::arch::trap::restore(unsafe { &*tf_ptr });
}

unsafe fn is_kernel_entry(tf_ptr: *mut crate::arch::trap::TrapFrame) -> bool {
    unsafe { (*tf_ptr).era >= crate::arch::constant::KERNEL_BASE }
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
    initial_pc: VA,
    user_sp_high: VA,
) {
    if tfp.is_null() {
        crate::pr_err!("[kernel_execve] trap_frame_ptr is null");
        panic!("kernel_execve: null trap_frame_ptr");
    }
    crate::pr_debug!("[kernel_execve] trap_frame_ptr={:#x}", tfp as usize);
    unsafe {
        crate::pr_debug!(
            "[kernel_execve] trapframe: era={:#x}, sp={:#x}, prmd={:#x}, crmd={:#x}, a0={:#x}, a1={:#x}, a2={:#x}",
            (*tfp).get_sepc(),
            (*tfp).get_sp(),
            (*tfp).prmd,
            (*tfp).crmd,
            (*tfp).get_a0(),
            (*tfp).regs[5],
            (*tfp).regs[6],
        );
    }

    use crate::mm::address::PageNum;
    let tlbrent: usize;
    let crmd: usize;
    let pgdl: usize;
    let pgdh: usize;
    let ecfg: usize;
    let ks0: usize;
    let asid: usize;
    let tlbrehi: usize;
    let tlbrelo0: usize;
    let tlbrelo1: usize;
    unsafe {
        core::arch::asm!("csrrd {0}, 0x88", out(reg) tlbrent, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x0", out(reg) crmd, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x19", out(reg) pgdl, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x1a", out(reg) pgdh, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x4", out(reg) ecfg, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x30", out(reg) ks0, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x18", out(reg) asid, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x8e", out(reg) tlbrehi, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x8c", out(reg) tlbrelo0, options(nostack, preserves_flags));
        core::arch::asm!("csrrd {0}, 0x8d", out(reg) tlbrelo1, options(nostack, preserves_flags));
    }
    let space = crate::kernel::current_memory_space();
    let space = space.lock();
    let root_ppn = space.root_ppn();
    let root_paddr = root_ppn.start_addr().as_usize();
    let entry_va = initial_pc;
    let sp_va = user_sp_high;
    unsafe extern "C" {
        fn tlb_refill_entry();
    }
    let tlbr_entry_vaddr = tlb_refill_entry as usize;
    let tlbr_entry_paddr =
        unsafe { crate::arch::va_to_pa(VA::from_usize(tlbr_entry_vaddr)) }.as_usize() & !0xfff;
    let tlbr_entry_dm_vaddr =
        crate::arch::pa_to_va(crate::arch::address::PA::from_usize(tlbr_entry_paddr)).as_usize();
    crate::pr_debug!(
        "[kernel_execve] va translate: entry={:?}, sp={:?}",
        space.translate(entry_va),
        space.translate(sp_va)
    );
    use crate::mm::address::Vpn;
    use crate::mm::page_table::PageTableInner;
    let entry_vpn = Vpn::from_addr_floor(entry_va);
    if let Ok((ppn, _, flags)) = space.page_table().walk(entry_vpn) {
        crate::pr_debug!(
            "[kernel_execve] entry PTE: vpn={:#x}, ppn={:#x}, flags={:?}",
            entry_vpn.0,
            ppn.0,
            flags
        );
    } else {
        crate::pr_err!("[kernel_execve] entry page not mapped!");
    }
    crate::pr_debug!(
        "[kernel_execve] root_ppn={:#x}, root_paddr={:#x}",
        root_ppn.0,
        root_paddr
    );
    crate::pr_debug!(
        "[kernel_execve] tlbrent={:#x}, crmd={:#x}, pgdl={:#x}, pgdh={:#x}, ecfg={:#x}, ks0={:#x}",
        tlbrent,
        crmd,
        pgdl,
        pgdh,
        ecfg,
        ks0
    );
    crate::pr_debug!(
        "[kernel_execve] asid={:#x} (full_csr={:#x}), tlbrehi={:#x}, tlbrelo0={:#x}, tlbrelo1={:#x}",
        asid & 0x3ff,
        asid,
        tlbrehi,
        tlbrelo0,
        tlbrelo1
    );
    crate::pr_debug!(
        "[kernel_execve] tlb_refill_entry: vaddr={:#x}, paddr={:#x}, dm_vaddr={:#x}",
        tlbr_entry_vaddr,
        tlbr_entry_paddr,
        tlbr_entry_dm_vaddr
    );
    unsafe {
        core::arch::asm!("csrwr {0}, 0x30", in(reg) tfp as usize, options(nostack, preserves_flags));
        core::arch::asm!("csrwr $zero, 0x8b", options(nostack, preserves_flags));
    }
}

fn write_user_usize(space: &MemorySpace, dst: usize, val: usize) {
    let bytes = val.to_ne_bytes();
    write_user_bytes(space, dst, &bytes);
}

fn write_user_bytes(space: &MemorySpace, dst: usize, data: &[u8]) {
    let mut offset = 0usize;
    while offset < data.len() {
        let vaddr = VA::from_usize(dst + offset);
        let paddr = space
            .translate(vaddr)
            .expect("write_user_bytes: translate failed");
        let page_off = (dst + offset) & (PAGE_SIZE - 1);
        let chunk = core::cmp::min(PAGE_SIZE - page_off, data.len() - offset);
        unsafe {
            let dst_va = crate::arch::pa_to_va(paddr);
            let dst_ptr = dst_va.as_usize() as *mut u8;
            let src_ptr = data.as_ptr().add(offset);
            for i in 0..chunk {
                ptr::write(dst_ptr.add(i), ptr::read(src_ptr.add(i)));
            }
        }
        offset += chunk;
    }
}
