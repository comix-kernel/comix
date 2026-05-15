use crate::uapi::signal::MContextT;

/// Mock trap frame for host compilation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    pub sepc: usize,
    pub x1_ra: usize,
    pub x2_sp: usize,
    pub x3_gp: usize,
    pub x4_tp: usize,
    pub x5_t0: usize,
    pub x6_t1: usize,
    pub x7_t2: usize,
    pub x8_s0: usize,
    pub x9_s1: usize,
    pub x10_a0: usize,
    pub x11_a1: usize,
    pub x12_a2: usize,
    pub x13_a3: usize,
    pub x14_a4: usize,
    pub x15_a5: usize,
    pub x16_a6: usize,
    pub x17_a7: usize,
    pub x18_s2: usize,
    pub x19_s3: usize,
    pub x20_s4: usize,
    pub x21_s5: usize,
    pub x22_s6: usize,
    pub x23_s7: usize,
    pub x24_s8: usize,
    pub x25_s9: usize,
    pub x26_s10: usize,
    pub x27_s11: usize,
    pub x28_t3: usize,
    pub x29_t4: usize,
    pub x30_t5: usize,
    pub x31_t6: usize,
    pub sstatus: usize,
    pub kernel_sp: usize,
    pub cpu_ptr: usize,
}

impl TrapFrame {
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
            cpu_ptr: 0,
        }
    }

    pub fn get_sp(&self) -> usize {
        self.x2_sp
    }

    pub fn set_sp(&mut self, val: usize) {
        self.x2_sp = val;
    }

    pub fn get_a0(&self) -> usize {
        self.x10_a0
    }

    pub fn set_a0(&mut self, val: usize) {
        self.x10_a0 = val;
    }

    pub fn set_a1(&mut self, val: usize) {
        self.x11_a1 = val;
    }

    pub fn set_a2(&mut self, val: usize) {
        self.x12_a2 = val;
    }

    pub fn set_ra(&mut self, val: usize) {
        self.x1_ra = val;
    }

    pub fn set_sepc(&mut self, pc: usize) {
        self.sepc = pc;
    }

    pub fn get_sepc(&self) -> usize {
        self.sepc
    }

    pub fn set_kernel_trap_frame(&mut self, entry: usize, terminal: usize, kernel_sp: usize) {
        self.sepc = entry;
        self.x1_ra = terminal;
        self.x2_sp = kernel_sp;
        self.kernel_sp = kernel_sp;
    }

    pub unsafe fn set_clone_trap_frame(
        &mut self,
        parent_frame: &TrapFrame,
        kernel_sp: usize,
        user_sp: usize,
    ) {
        *self = *parent_frame;
        self.x10_a0 = 0;
        self.kernel_sp = kernel_sp;
        if user_sp != 0 {
            self.x2_sp = user_sp;
        }
    }

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
        self.sepc = entry;
        self.kernel_sp = kernel_sp;
        self.x2_sp = user_sp;
        self.x10_a0 = argc;
        self.x11_a1 = argv;
        self.x12_a2 = envp;
    }

    pub fn set_exec_trap_frame_from_layout(
        &mut self,
        entry: usize,
        kernel_sp: usize,
        layout: &crate::arch::task::ExecStackLayout,
    ) {
        self.set_exec_trap_frame(
            entry,
            layout.sp.as_usize(),
            kernel_sp,
            layout.argc,
            layout.argv.as_usize(),
            layout.envp.as_usize(),
        );
    }

    pub fn set_tls(&mut self, tls: usize) {
        self.x4_tp = tls;
    }

    pub unsafe fn set_fork_trap_frame(&mut self, parent_frame: &TrapFrame) {
        *self = *parent_frame;
        self.x10_a0 = 0;
    }

    pub fn to_mcontext(&self) -> MContextT {
        MContextT {
            gregs: [0; 32],
            fpregs: [0; 66],
        }
    }

    pub fn restore_from_mcontext(&mut self, _mcontext: &MContextT) {}
}

impl crate::kernel::syscall::syscall_frame::SyscallFrame for TrapFrame {
    fn syscall_id(&self) -> usize {
        self.x17_a7
    }

    fn arg0(&self) -> usize {
        self.x10_a0
    }
    fn arg1(&self) -> usize {
        self.x11_a1
    }
    fn arg2(&self) -> usize {
        self.x12_a2
    }
    fn arg3(&self) -> usize {
        self.x13_a3
    }
    fn arg4(&self) -> usize {
        self.x14_a4
    }
    fn arg5(&self) -> usize {
        self.x15_a5
    }

    fn set_ret(&mut self, val: usize) {
        self.x10_a0 = val;
    }
}

pub struct SumGuard;
impl SumGuard {
    pub fn new() -> Self {
        Self
    }
}

pub unsafe fn restore(_trap_frame: &TrapFrame) {}

pub fn sigreturn_trampoline_address() -> usize {
    super::constant::SV39_BOT_HALF_TOP & !(4096 - 1)
}

pub fn kernel_sigreturn_trampoline_bytes() -> &'static [u8] {
    &[]
}

pub unsafe fn set_trap_frame_cpu_ptr(trap_frame_ptr: *mut TrapFrame, cpu_ptr: usize) {
    if let Some(tf) = unsafe { trap_frame_ptr.as_mut() } {
        tf.cpu_ptr = cpu_ptr;
    }
}
