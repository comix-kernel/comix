/// 在发生调度时保存的上下文信息
/// 相较于TrapFram只保存切换所需的最少量寄存器
pub struct Context {
    /// 返回地址
    pub ra: usize,
    /// 栈指针
    pub sp: usize,
    /// 保存s0-s11寄存器
    pub s: [usize; 12],
}
