use core::ptr;

use alloc::vec::Vec;
use riscv::register::sstatus;

use crate::arch::constant::STACK_ALIGN_MASK;

/// 为新任务设置用户栈布局，包含命令行参数和环境变量
/// 返回新的栈指针位置，以及 argc, argv, envp 的地址
pub fn setup_stack_layout(sp: usize, argv: &[&str], envp: &[&str]) -> (usize, usize, usize, usize) {
    let mut sp = sp;
    let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
    let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());
    unsafe {
        sstatus::set_sum();
    }

    // 环境变量 (envp)
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

    // 6. 最终 16 字节对齐（应用到最终栈指针 sp）
    sp &= !STACK_ALIGN_MASK;
    (sp, argc, argv_vec_ptr, envp_vec_ptr)
}
