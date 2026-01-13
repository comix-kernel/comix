//! LoongArch64 陷阱帧定义

use crate::arch::constant::{CSR_CRMD_PLV_MASK, PRMD_PIE, PRMD_PPLV_MASK, PRMD_PPLV_USER};
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
    /// 当前 CPU 结构体指针（供 trap_entry 设置 tp）
    pub cpu_ptr: usize,
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
            cpu_ptr: 0,
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

    /// 设置第二个参数寄存器 (a1)
    #[inline]
    pub fn set_a1(&mut self, val: usize) {
        self.regs[5] = val;
    }

    /// 设置第三个参数寄存器 (a2)
    #[inline]
    pub fn set_a2(&mut self, val: usize) {
        self.regs[6] = val;
    }

    /// 设置返回地址寄存器 (ra)
    #[inline]
    pub fn set_ra(&mut self, val: usize) {
        self.regs[1] = val;
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
        tls_tp: usize,
    ) {
        *self = Self::zero_init();
        self.era = entry;
        self.regs[2] = tls_tp; // tp (thread pointer / TLS)
        self.regs[3] = user_sp; // sp
        self.regs[4] = argc; // a0
        self.regs[5] = argv; // a1
        self.regs[6] = envp; // a2
        self.kernel_sp = kernel_sp;
        // 设置返回用户态的 PRMD 和 CRMD
        self.prmd = (PRMD_PPLV_USER & PRMD_PPLV_MASK) | PRMD_PIE;
        // CRMD: 清除 PLV 和 DA，设置 PG（映射模式）
        use crate::arch::constant::{CSR_CRMD_DA, CSR_CRMD_PG};
        self.crmd = (read_crmd() & !CSR_CRMD_PLV_MASK & !CSR_CRMD_DA) | CSR_CRMD_PG;
        crate::pr_debug!(
            "[exec_trap_frame] era={:#x}, sp={:#x}, tp={:#x}, prmd={:#x}, crmd={:#x}, a0={:#x}, a1={:#x}, a2={:#x}",
            self.era,
            self.regs[3],
            self.regs[2],
            self.prmd,
            self.crmd,
            self.regs[4],
            self.regs[5],
            self.regs[6],
        );
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

#[inline(always)]
fn read_crmd() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!(
            "csrrd {value}, 0x0",
            value = out(reg) value,
            options(nostack, preserves_flags)
        );
    }
    value
}

impl Default for TrapFrame {
    fn default() -> Self {
        Self::empty()
    }
}
