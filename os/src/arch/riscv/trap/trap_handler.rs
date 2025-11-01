//! 64 位 RISC-V，usize = 8 字节

use core::sync::atomic::Ordering;

use riscv::register::scause::{self, Trap};
use riscv::register::{sepc, sstatus};

use crate::arch::timer::TIMER_TICKS;
use crate::kernel::{SCHEDULER, schedule};

/// 陷阱帧结构体，保存寄存器状态
#[repr(C)] // 确保 Rust 不会重新排列字段
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// 程序计数器
    /// 在发生陷阱时，sepc 寄存器的值应保存到这里
    pub sepc: usize, // 0(sp)
    pub x1_ra: usize,   // 8(sp)
    pub x2_sp: usize,   // 16(sp)
    pub x3_gp: usize,   // 24(sp)
    pub x4_tp: usize,   // 32(sp)
    pub x5_t0: usize,   // 40(sp)
    pub x6_t1: usize,   // 48(sp)
    pub x7_t2: usize,   // 56(sp)
    pub x8_s0: usize,   // 64(sp)
    pub x9_s1: usize,   // 72(sp)
    pub x10_a0: usize,  // 80(sp)
    pub x11_a1: usize,  // 88(sp)
    pub x12_a2: usize,  // 96(sp)
    pub x13_a3: usize,  // 104(sp)
    pub x14_a4: usize,  // 112(sp)
    pub x15_a5: usize,  // 120(sp)
    pub x16_a6: usize,  // 128(sp)
    pub x17_a7: usize,  // 136(sp)
    pub x18_s2: usize,  // 144(sp)
    pub x19_s3: usize,  // 152(sp)
    pub x20_s4: usize,  // 160(sp)
    pub x21_s5: usize,  // 168(sp)
    pub x22_s6: usize,  // 176(sp)
    pub x23_s7: usize,  // 184(sp)
    pub x24_s8: usize,  // 192(sp)
    pub x25_s9: usize,  // 200(sp)
    pub x26_s10: usize, // 208(sp)
    pub x27_s11: usize, // 216(sp)
    pub x28_t3: usize,  // 224(sp)
    pub x29_t4: usize,  // 232(sp)
    pub x30_t5: usize,  // 240(sp)
    pub x31_t6: usize,  // 248(sp)
    pub sstatus: usize, // 256(sp)
    pub kernel_sp: usize, // 264(sp)
                        // pub kernel_satp: usize, // 272(sp)
                        // pub kernel_hartid: usize, // 280(sp)
}

// XXX: CSR可能因调度或中断被修改？
/// 内核陷阱处理程序
/// 从 kernelvec 跳转到这里时，
/// 陷阱帧的地址（sp）被隐式地作为参数 a0 传递给了 kerneltrap，
/// 在这里，trap_frame 指向了栈上保存的 KernelTrapFrame 结构体。
/// 此外，在通过 stvec 跳转到 kernelvec 时，SSIE 被自动清除，
/// 因此在内核陷阱处理程序开始时，中断是被禁用的。
/// 必须从这个函数正常返回后，通过 kernelvec 中的 sret 指令才能正确恢复中断状态。
#[unsafe(no_mangle)]
pub extern "C" fn trap_handler(_trap_frame: &mut TrapFrame) {
    // 保存进入中断时的状态
    let sstatus_old = sstatus::read();
    let sepc_old = sepc::read();
    let scause = scause::read();

    match scause.cause() {
        Trap::Interrupt(5) => {
            // 处理时钟中断
            crate::arch::timer::set_next_trigger();
            check_timer();
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
}

/// 处理来自用户态的陷阱（系统调用、中断、异常）
#[allow(dead_code)]
pub fn user_trap() {
    unimplemented!()
}

#[allow(dead_code)]
pub fn kernel_trap() {
    unimplemented!()
}

/// 处理时钟中断
pub fn check_timer() {
    let _ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    println!("[kernel timer interrupt] ticks = {}", _ticks + 1);
    if SCHEDULER.lock().update_time_slice() {
        schedule();
    }
}

#[allow(dead_code)]
/// TODO: 处理设备中断
pub fn check_device() {}
