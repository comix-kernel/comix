//! RISC-V 架构的 CPU 相关功能

/// 获取当前 CPU 的 ID
///
/// 使用 tp (x4) 寄存器存储 CPU ID。
/// 在启动时，entry.S 会将 hartid 写入 tp 寄存器。
///
/// # 返回值
/// - 当前 CPU 的 ID（0 到 NUM_CPU-1）
///
/// # Safety
/// 此函数假设 tp 寄存器已在启动时正确设置，且不会被意外修改。
///
/// # 注意
/// 由于用户程序可能修改 tp 寄存器，此函数会进行边界检查。
/// 如果 tp 值超出有效范围，返回 0（主核）。
#[inline]
pub fn cpu_id() -> usize {
    let id: usize;
    // SAFETY: 读取 tp 寄存器是安全的
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) id);
    }

    // 边界检查：如果 tp 值无效（被用户程序修改），返回 0
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    if id >= num_cpu {
        0  // 默认返回主核 ID
    } else {
        id
    }
}
