//! LoongArch64 陷阱帧定义

use crate::uapi::signal::MContextT;

/// 陷阱帧结构
/// 保存进入陷阱时的所有寄存器
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// 通用寄存器 r0-r31
    pub regs: [usize; 32],
    /// 程序计数器 (异常前的 PC)
    pub era: usize,
    /// 异常状态
    pub estat: usize,
    /// 当前模式信息
    pub crmd: usize,
    /// 异常前模式信息
    pub prmd: usize,
    /// 内核栈指针
    pub kernel_sp: usize,
}

impl TrapFrame {
    /// 创建空的陷阱帧
    pub const fn empty() -> Self {
        Self {
            regs: [0; 32],
            era: 0,
            estat: 0,
            crmd: 0,
            prmd: 0,
            kernel_sp: 0,
        }
    }

    /// 创建全零初始化的陷阱帧
    pub fn zero_init() -> Self {
        Self::empty()
    }

    /// 获取系统调用号
    pub fn syscall_id(&self) -> usize {
        self.regs[11] // a7
    }

    /// 获取系统调用参数
    pub fn syscall_args(&self) -> [usize; 6] {
        [
            self.regs[4], // a0
            self.regs[5], // a1
            self.regs[6], // a2
            self.regs[7], // a3
            self.regs[8], // a4
            self.regs[9], // a5
        ]
    }

    /// 设置系统调用返回值
    pub fn set_syscall_ret(&mut self, ret: usize) {
        self.regs[4] = ret; // a0
    }

    /// 获取程序计数器（方法版本）
    pub fn sepc(&self) -> usize {
        self.era
    }

    /// 设置程序计数器
    pub fn set_sepc(&mut self, pc: usize) {
        self.era = pc;
    }

    // ===== 跨架构兼容的访问方法 =====

    /// 获取栈指针
    #[inline]
    pub fn get_sp(&self) -> usize {
        self.regs[3] // $sp = $r3
    }

    /// 设置栈指针
    #[inline]
    pub fn set_sp(&mut self, val: usize) {
        self.regs[3] = val;
    }

    /// 获取第一个参数寄存器 (a0)
    #[inline]
    pub fn get_a0(&self) -> usize {
        self.regs[4] // $a0 = $r4
    }

    /// 设置第一个参数寄存器 (a0)
    #[inline]
    pub fn set_a0(&mut self, val: usize) {
        self.regs[4] = val;
    }

    /// 获取程序计数器
    #[inline]
    pub fn get_sepc(&self) -> usize {
        self.era
    }

    /// 设置内核态陷阱帧
    pub fn set_kernel_trap_frame(&mut self, entry: usize, _terminal: usize, kernel_sp: usize) {
        self.era = entry;
        self.regs[3] = kernel_sp; // sp
        self.kernel_sp = kernel_sp;
    }

    /// 设置用户态陷阱帧（用于 execve）
    pub fn set_exec_trap_frame(
        &mut self,
        entry: usize,
        user_sp: usize,
        kernel_sp: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        *self = Self::zero_init();
        self.era = entry;
        self.regs[3] = user_sp; // sp
        self.regs[4] = argc; // a0
        self.regs[5] = argv; // a1
        self.regs[6] = envp; // a2
        self.kernel_sp = kernel_sp;
        // TODO: 设置 PRMD 为用户态
    }

    /// 设置克隆线程的 TrapFrame
    pub unsafe fn set_clone_trap_frame(
        &mut self,
        parent_frame: &TrapFrame,
        kernel_sp: usize,
        user_sp: usize,
    ) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                parent_frame as *const _ as *const u8,
                self as *mut _ as *mut u8,
                core::mem::size_of::<TrapFrame>(),
            );
        }
        self.regs[4] = 0; // a0 = 0，子进程返回 0
        self.kernel_sp = kernel_sp;
        if user_sp != 0 {
            self.regs[3] = user_sp;
        }
    }

    /// 设置 fork 后子进程的 TrapFrame
    pub unsafe fn set_fork_trap_frame(&mut self, parent_frame: &TrapFrame) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                parent_frame as *const _ as *const u8,
                self as *mut _ as *mut u8,
                core::mem::size_of::<TrapFrame>(),
            );
        }
        self.regs[4] = 0; // a0 = 0
    }

    /// 将 TrapFrame 转换为 MContextT
    pub fn to_mcontext(&self) -> MContextT {
        let mut gregs = [0u64; 32];
        for i in 0..32 {
            gregs[i] = self.regs[i] as u64;
        }
        MContextT {
            gregs,
            fpregs: [0; 66],
        }
    }

    /// 从 MContextT 恢复 TrapFrame
    pub fn restore_from_mcontext(&mut self, mcontext: &MContextT) {
        for i in 0..32 {
            self.regs[i] = mcontext.gregs[i] as usize;
        }
    }
}

impl Default for TrapFrame {
    fn default() -> Self {
        Self::empty()
    }
}
