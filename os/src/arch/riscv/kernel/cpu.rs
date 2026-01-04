//! RISC-V 架构的 CPU 相关功能

use riscv::register::sscratch;

/// 获取当前 CPU 的 ID
///
/// 从 tp 寄存器指向的 Cpu 结构体中读取 CPU ID。
/// 在内核态，tp 指向 Cpu 结构体；在用户态，tp 是 TLS 指针。
///
/// # 返回值
/// - 当前 CPU 的 ID（0 到 NUM_CPU-1）
///
/// # Safety
/// 此函数假设在内核态调用，tp 寄存器指向有效的 Cpu 结构体。
/// trap_entry 会在进入内核时设置 tp 指向 Cpu 结构体。
#[inline]
pub fn cpu_id() -> usize {
    let id: usize;
    // SAFETY: 在内核态，tp 指向 Cpu 结构体，第一个字段是 cpu_id
    unsafe {
        core::arch::asm!(
            "ld {}, 0(tp)",  // 读取 Cpu.cpu_id (偏移 0)
            out(reg) id
        );
    }
    id
}

/// 在切换到指定任务后执行的架构相关收尾工作。
///
/// - 更新 TrapFrame 中的 per-CPU 指针（供 trap_entry 恢复 tp）。
/// - 更新 sscratch 指向新任务的 TrapFrame（供陷阱保存/恢复使用）。
pub fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize) {
    if trap_frame_ptr == 0 {
        return;
    }

    // Safety: trap_frame_ptr 指向任务自有的 TrapFrame 缓冲区。
    unsafe {
        let tf = (trap_frame_ptr as *mut crate::arch::trap::TrapFrame)
            .as_mut()
            .expect("on_task_switch: null TrapFrame");
        tf.cpu_ptr = cpu_ptr;

        sscratch::write(trap_frame_ptr);
    }
}
