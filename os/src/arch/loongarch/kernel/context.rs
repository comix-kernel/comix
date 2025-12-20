//! LoongArch64 任务上下文

/// 任务上下文结构
/// 用于上下文切换时保存/恢复 callee-saved 寄存器
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskContext {
    /// 返回地址 ($r1 / $ra)
    pub ra: usize,
    /// 栈指针 ($r3 / $sp)
    pub sp: usize,
    /// callee-saved 寄存器 s0-s8 ($r23-$r31)
    pub s: [usize; 9],
}

impl TaskContext {
    /// 创建空的任务上下文
    pub const fn empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 9],
        }
    }

    /// 创建全零初始化的上下文
    pub fn zero_init() -> Self {
        Self::empty()
    }

    /// 设置线程的初始上下文
    pub fn set_init_context(&mut self, entry: usize, kstack_top: usize) {
        self.sp = kstack_top;
        self.ra = entry;
    }
}

/// Context 类型别名（用于兼容 RISC-V 代码的导入路径）
pub type Context = TaskContext;
