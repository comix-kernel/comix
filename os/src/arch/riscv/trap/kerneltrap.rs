// 64 位 RISC-V，usize = 8 字节

use core::sync::atomic::Ordering;

use riscv::register::scause::{self, Trap};
use riscv::register::{sepc, sstatus};

use crate::arch::timer::TIMER_TICKS;

// XXX: CSR可能因调度或中断被修改？
#[unsafe(no_mangle)]
pub extern "C" fn kerneltrap(_trap_frame: &mut KernelTrapFrame) {
    // 陷阱帧的地址（sp）被隐式地作为参数 a0 传递给了 kerneltrap
    // 在这里，trap_frame 指向了栈上保存的 KernelTrapFrame 结构体

    // 保存进入中断时的状态
    let sstatus_old = sstatus::read();
    let sepc_old = sepc::read();

    // 临时禁用中断以防嵌套
    unsafe {
        sstatus::clear_sie();
    }

    let scause = scause::read();

    match scause.cause() {
        Trap::Interrupt(5) => {
            // 处理时钟中断
            crate::arch::timer::set_next_trigger();
            check_timer();
            // 恢复sepc，确保正确返回
            unsafe {
                sepc::write(sepc_old);
            }
        }
        Trap::Exception(e) => panic!(
            "Unexpected exception in kernel: {:?}, sepc = {:#x}, sstatus = {:#x}",
            e,
            sepc_old,
            sstatus_old.bits()
        ),
        trap => panic!(
            "Unexpected trap in kernel: {:?}, sepc = {:#x}, sstatus = {:#x}",
            trap,
            sepc_old,
            sstatus_old.bits()
        ),
    }

    // 恢复进入中断前的状态
    unsafe {
        sstatus::write(sstatus_old);
    }
}

#[repr(C)] // 确保 Rust 不会重新排列字段
#[derive(Debug, Clone, Copy)]
pub struct KernelTrapFrame {
    pub x1_ra: usize,   // 0(sp)
    pub x2_sp: usize,   // 8(sp)
    pub x3_gp: usize,   // 16(sp)
    pub x4_tp: usize,   // 24(sp)
    pub x5_t0: usize,   // 32(sp)
    pub x6_t1: usize,   // 40(sp)
    pub x7_t2: usize,   // 48(sp)
    pub x8_s0: usize,   // 56(sp)
    pub x9_s1: usize,   // 64(sp)
    pub x10_a0: usize,  // 72(sp)
    pub x11_a1: usize,  // 80(sp)
    pub x12_a2: usize,  // 88(sp)
    pub x13_a3: usize,  // 96(sp)
    pub x14_a4: usize,  // 104(sp)
    pub x15_a5: usize,  // 112(sp)
    pub x16_a6: usize,  // 120(sp)
    pub x17_a7: usize,  // 128(sp)
    pub x18_s2: usize,  // 136(sp)
    pub x19_s3: usize,  // 144(sp)
    pub x20_s4: usize,  // 152(sp)
    pub x21_s5: usize,  // 160(sp)
    pub x22_s6: usize,  // 168(sp)
    pub x23_s7: usize,  // 176(sp)
    pub x24_s8: usize,  // 184(sp)
    pub x25_s9: usize,  // 192(sp)
    pub x26_s10: usize, // 200(sp)
    pub x27_s11: usize, // 208(sp)
    pub x28_t3: usize,  // 216(sp)
    pub x29_t4: usize,  // 224(sp)
    pub x30_t5: usize,  // 232(sp)
    pub x31_t6: usize,  // 240(sp)
                        // 总共 31 个寄存器，总大小 31 * 8 = 248 字节。
                        // 汇编代码分配了 256 字节，最后 8 字节未使用。
}

/// 处理时钟中断
pub fn check_timer() {
    let _ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

    // TODO: 这里可以添加更多的时钟中断处理逻辑
    // 比如：
    // - 更新系统时间
    // - 检查是否需要进行任务调度
    // - 处理定时任务等
}

#[allow(dead_code)]
/// TODO: 处理设备中断
pub fn check_device() {}
