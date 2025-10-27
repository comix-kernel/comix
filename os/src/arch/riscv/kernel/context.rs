/// 在发生调度时保存的上下文信息
/// 相较于TrapFram只保存切换所需的最少量寄存器
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// 返回地址
    pub ra: usize,
    /// 栈指针
    pub sp: usize,
    /// 保存s0-s11寄存器
    pub s: [usize; 12],
}

impl Context {
    /// 创建一个全零初始化的上下文
    pub fn zero_init() -> Self {
        Context {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
}
