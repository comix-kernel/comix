use riscv::register::sstatus;

use crate::uapi::signal::MContextT;

/// 陷阱帧结构体，保存寄存器状态
#[repr(C)] // 确保 Rust 不会重新排列字段
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// 程序计数器
    /// 在发生陷阱时，sepc 寄存器的值应保存到这里
    pub sepc: usize, // 0(sp)
    pub x1_ra: usize,     // 8(sp)
    pub x2_sp: usize,     // 16(sp)
    pub x3_gp: usize,     // 24(sp)
    pub x4_tp: usize,     // 32(sp)
    pub x5_t0: usize,     // 40(sp)
    pub x6_t1: usize,     // 48(sp)
    pub x7_t2: usize,     // 56(sp)
    pub x8_s0: usize,     // 64(sp)
    pub x9_s1: usize,     // 72(sp)
    pub x10_a0: usize,    // 80(sp)
    pub x11_a1: usize,    // 88(sp)
    pub x12_a2: usize,    // 96(sp)
    pub x13_a3: usize,    // 104(sp)
    pub x14_a4: usize,    // 112(sp)
    pub x15_a5: usize,    // 120(sp)
    pub x16_a6: usize,    // 128(sp)
    pub x17_a7: usize,    // 136(sp)
    pub x18_s2: usize,    // 144(sp)
    pub x19_s3: usize,    // 152(sp)
    pub x20_s4: usize,    // 160(sp)
    pub x21_s5: usize,    // 168(sp)
    pub x22_s6: usize,    // 176(sp)
    pub x23_s7: usize,    // 184(sp)
    pub x24_s8: usize,    // 192(sp)
    pub x25_s9: usize,    // 200(sp)
    pub x26_s10: usize,   // 208(sp)
    pub x27_s11: usize,   // 216(sp)
    pub x28_t3: usize,    // 224(sp)
    pub x29_t4: usize,    // 232(sp)
    pub x30_t5: usize,    // 240(sp)
    pub x31_t6: usize,    // 248(sp)
    pub sstatus: usize,   // 256(sp)
    pub kernel_sp: usize, // 264(sp)
    /// 指向当前 CPU 结构体的指针
    /// 用于在 trap entry 时快速获取 CPU 信息并设置 tp
    pub cpu_ptr: usize, // 272(sp)
}

impl TrapFrame {
    /// 创建一个全零初始化的陷阱帧
    ///
    /// 注意：cpu_ptr 会自动初始化为当前 CPU 的指针
    pub fn zero_init() -> Self {
        // 获取当前 CPU 的指针
        let cpu_ptr = {
            use crate::sync::PreemptGuard;
            let _guard = PreemptGuard::new();
            crate::kernel::current_cpu() as *const _ as usize
        };

        TrapFrame {
            sepc: 0,
            x1_ra: 0,
            x2_sp: 0,
            x3_gp: 0,
            x4_tp: 0,
            x5_t0: 0,
            x6_t1: 0,
            x7_t2: 0,
            x8_s0: 0,
            x9_s1: 0,
            x10_a0: 0,
            x11_a1: 0,
            x12_a2: 0,
            x13_a3: 0,
            x14_a4: 0,
            x15_a5: 0,
            x16_a6: 0,
            x17_a7: 0,
            x18_s2: 0,
            x19_s3: 0,
            x20_s4: 0,
            x21_s5: 0,
            x22_s6: 0,
            x23_s7: 0,
            x24_s8: 0,
            x25_s9: 0,
            x26_s10: 0,
            x27_s11: 0,
            x28_t3: 0,
            x29_t4: 0,
            x30_t5: 0,
            x31_t6: 0,
            sstatus: 0,
            kernel_sp: 0,
            cpu_ptr,
        }
    }

    // ===== 跨架构兼容的访问方法 =====

    /// 获取栈指针
    #[inline]
    pub fn get_sp(&self) -> usize {
        self.x2_sp
    }

    /// 设置栈指针
    #[inline]
    pub fn set_sp(&mut self, val: usize) {
        self.x2_sp = val;
    }

    /// 获取第一个参数寄存器 (a0)
    #[inline]
    pub fn get_a0(&self) -> usize {
        self.x10_a0
    }

    /// 设置第一个参数寄存器 (a0)
    #[inline]
    pub fn set_a0(&mut self, val: usize) {
        self.x10_a0 = val;
    }

    /// 设置第二个参数寄存器 (a1)
    #[inline]
    pub fn set_a1(&mut self, val: usize) {
        self.x11_a1 = val;
    }

    /// 设置第三个参数寄存器 (a2)
    #[inline]
    pub fn set_a2(&mut self, val: usize) {
        self.x12_a2 = val;
    }

    /// 设置返回地址寄存器 (ra)
    #[inline]
    pub fn set_ra(&mut self, val: usize) {
        self.x1_ra = val;
    }

    /// 设置程序计数器
    #[inline]
    pub fn set_sepc(&mut self, pc: usize) {
        self.sepc = pc;
    }

    /// 获取程序计数器
    #[inline]
    pub fn get_sepc(&self) -> usize {
        self.sepc
    }

    /// 设置内核线程的初始陷阱帧
    /// 参数:
    /// * `entry`: 线程入口地址
    /// * `terminal`: 线程结束时跳转地址
    /// * `kernel_sp`: 内核栈顶地址
    pub fn set_kernel_trap_frame(
        &mut self,
        entry: usize,
        terminal: usize,
        kernel_sp: usize,
        // kernel_satp: usize,
        // kernel_hartid: usize,
    ) {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(sstatus::SPP::Supervisor);
        sstatus.set_sie(false);
        sstatus.set_spie(true);
        self.sepc = entry;
        self.sstatus = sstatus.bits();
        self.kernel_sp = kernel_sp;
        self.x1_ra = terminal;
        self.x2_sp = kernel_sp;
        // 设置 tp 指向当前 CPU 结构体
        self.x4_tp = {
            use crate::sync::PreemptGuard;
            let _guard = PreemptGuard::new();
            crate::kernel::current_cpu() as *const _ as usize
        };
        // self.kernel_satp = kernel_satp;
        // self.kernel_hartid = kernel_hartid;
    }

    /// 设置克隆线程的 TrapFrame
    /// 参数:
    /// * `parent_frame`: 父线程的 TrapFrame 引用
    /// * `entry`: 线程入口地址
    /// * `args`: 传递给线程函数的参数
    /// * `kernel_sp`: 内核栈顶地址
    /// * `user_sp`: 用户栈顶地址
    /// # 安全性
    /// - `parent_frame` 必须指向一个完全初始化的、有效的 `TrapFrame`
    /// - `parent_frame` 必须在整个复制期间保持有效
    /// - `self` 必须指向一个可写的内存区域，大小至少为 `size_of::<TrapFrame>()`
    /// - `self` 和 `parent_frame` 不能内存重叠
    /// - 调用后 `self` 将包含 `parent_frame` 的精确副本（除了修改的字段）
    pub unsafe fn set_clone_trap_frame(
        &mut self,
        parent_frame: &TrapFrame,
        kernel_sp: usize,
        user_sp: usize,
    ) {
        // SAFETY: 调用者确保：
        // 1. parent_frame 有效且可读
        // 2. self 有效且可写
        // 3. 两者不重叠
        // 4. 两者都正确对齐
        unsafe {
            core::ptr::copy_nonoverlapping(
                parent_frame as *const _ as *const u8,
                self as *mut _ as *mut u8,
                core::mem::size_of::<TrapFrame>(),
            );
        }
        // 子进程返回 0
        self.x10_a0 = 0;
        self.kernel_sp = kernel_sp;
        // 如果提供了新栈，使用新栈；否则使用父进程的栈
        if user_sp != 0 {
            self.x2_sp = user_sp;
        }
        // sepc 不变，子进程从当前位置继续执行（类似 fork）
    }

    /// 设置用户态的 TrapFrame
    /// 用于execve新程序
    /// 参数:
    /// * `entry`: 用户程序入口地址
    /// * `user_sp`: 用户栈顶地址
    /// * `kernel_sp`: 内核栈顶地址
    /// * `argc`: 命令行参数个数
    /// * `argv`: 命令行参数指针数组地址
    /// * `envp`: 环境变量指针数组地址
    pub fn set_exec_trap_frame(
        &mut self,
        entry: usize,
        user_sp: usize,
        kernel_sp: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(sstatus::SPP::User);
        sstatus.set_sie(false);
        sstatus.set_spie(true);

        // Clear all registers first
        *self = Self::zero_init();

        self.sepc = entry;
        self.sstatus = sstatus.bits();
        self.kernel_sp = kernel_sp;
        self.x2_sp = user_sp;

        // Set arguments
        self.x10_a0 = argc;
        self.x11_a1 = argv;
        self.x12_a2 = envp;

        // x1_ra is 0
    }

    /// 设置 fork 后子进程的 TrapFrame
    /// # 参数:
    /// * `tpr`: 父进程的 TrapFrame 引用
    /// # 安全性
    /// - `parent_frame` 必须指向一个完全初始化的、有效的 `TrapFrame`
    /// - `parent_frame` 必须在整个复制期间保持有效
    /// - `self` 必须指向一个可写的内存区域，大小至少为 `size_of::<TrapFrame>()`
    /// - `self` 和 `parent_frame` 不能内存重叠
    /// - 调用后 `self` 将包含 `parent_frame` 的精确副本（除了修改的字段）
    pub unsafe fn set_fork_trap_frame(&mut self, parent_frame: &TrapFrame) {
        // SAFETY: 调用者确保：
        // 1. parent_frame 有效且可读
        // 2. self 有效且可写
        // 3. 两者不重叠
        // 4. 两者都正确对齐
        unsafe {
            core::ptr::copy_nonoverlapping(
                parent_frame as *const _ as *const u8,
                self as *mut _ as *mut u8,
                core::mem::size_of::<TrapFrame>(),
            );
        }
        // 子进程返回值为0
        self.x10_a0 = 0;
    }

    /// 将 TrapFrame 转换为 MContextT 结构体
    pub fn to_mcontext(&self) -> MContextT {
        MContextT {
            gregs: [
                self.sepc as u64,
                self.x1_ra as u64,
                self.x2_sp as u64,
                self.x3_gp as u64,
                self.x4_tp as u64,
                self.x5_t0 as u64,
                self.x6_t1 as u64,
                self.x7_t2 as u64,
                self.x8_s0 as u64,
                self.x9_s1 as u64,
                self.x10_a0 as u64,
                self.x11_a1 as u64,
                self.x12_a2 as u64,
                self.x13_a3 as u64,
                self.x14_a4 as u64,
                self.x15_a5 as u64,
                self.x16_a6 as u64,
                self.x17_a7 as u64,
                self.x18_s2 as u64,
                self.x19_s3 as u64,
                self.x20_s4 as u64,
                self.x21_s5 as u64,
                self.x22_s6 as u64,
                self.x23_s7 as u64,
                self.x24_s8 as u64,
                self.x25_s9 as u64,
                self.x26_s10 as u64,
                self.x27_s11 as u64,
                self.x28_t3 as u64,
                self.x29_t4 as u64,
                self.x30_t5 as u64,
                self.x31_t6 as u64,
            ],
            fpregs: [0; 66],
        }
    }

    /// 从 MContextT 恢复 TrapFrame
    pub fn restore_from_mcontext(&mut self, mcontext: &MContextT) {
        self.sepc = mcontext.gregs[0] as usize;
        self.x1_ra = mcontext.gregs[1] as usize;
        self.x2_sp = mcontext.gregs[2] as usize;
        self.x3_gp = mcontext.gregs[3] as usize;
        self.x4_tp = mcontext.gregs[4] as usize;
        self.x5_t0 = mcontext.gregs[5] as usize;
        self.x6_t1 = mcontext.gregs[6] as usize;
        self.x7_t2 = mcontext.gregs[7] as usize;
        self.x8_s0 = mcontext.gregs[8] as usize;
        self.x9_s1 = mcontext.gregs[9] as usize;
        self.x10_a0 = mcontext.gregs[10] as usize;
        self.x11_a1 = mcontext.gregs[11] as usize;
        self.x12_a2 = mcontext.gregs[12] as usize;
        self.x13_a3 = mcontext.gregs[13] as usize;
        self.x14_a4 = mcontext.gregs[14] as usize;
        self.x15_a5 = mcontext.gregs[15] as usize;
        self.x16_a6 = mcontext.gregs[16] as usize;
        self.x17_a7 = mcontext.gregs[17] as usize;
        self.x18_s2 = mcontext.gregs[18] as usize;
        self.x19_s3 = mcontext.gregs[19] as usize;
        self.x20_s4 = mcontext.gregs[20] as usize;
        self.x21_s5 = mcontext.gregs[21] as usize;
        self.x22_s6 = mcontext.gregs[22] as usize;
        self.x23_s7 = mcontext.gregs[23] as usize;
        self.x24_s8 = mcontext.gregs[24] as usize;
        self.x25_s9 = mcontext.gregs[25] as usize;
        self.x26_s10 = mcontext.gregs[26] as usize;
        self.x27_s11 = mcontext.gregs[27] as usize;
        self.x28_t3 = mcontext.gregs[28] as usize;
        self.x29_t4 = mcontext.gregs[29] as usize;
        self.x30_t5 = mcontext.gregs[30] as usize;
        self.x31_t6 = mcontext.gregs[31] as usize;
    }
}
