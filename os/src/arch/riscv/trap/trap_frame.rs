use riscv::register::sstatus;

use crate::arch::kernel;

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

impl TrapFrame {
    /// 创建一个全零初始化的陷阱帧
    pub fn zero_init() -> Self {
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
            // kernel_satp: 0,
            // kernel_hartid: 0,
        }
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
        self.x1_ra = terminal;
        self.x2_sp = kernel_sp;
        self.kernel_sp = kernel_sp;
        // self.kernel_satp = kernel_satp;
        // self.kernel_hartid = kernel_hartid;
    }

    /// 设置用户态的 TrapFrame
    /// 参数:
    /// * `entry`: 用户程序入口地址
    /// * `user_sp`: 用户栈顶地址
    /// * `kernel_sp`: 内核栈顶地址
    /// * `argc`: 命令行参数个数
    /// * `argv`: 命令行参数指针数组地址
    /// * `envp`: 环境变量指针数组地址
    pub fn set_user_trap_frame(
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
        self.sepc = entry;
        self.sstatus = sstatus.bits();
        self.kernel_sp = kernel_sp;
        self.x2_sp = user_sp;
        self.x10_a0 = argc;
        self.x11_a1 = argv;
        self.x12_a2 = envp;
        // 清零 ra，避免意外返回路径，用户态程序应通过正常的退出机制结束
        self.x1_ra = 0;
    }
}
