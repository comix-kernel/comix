//! 架构 mock stub 模块
//!
//! 当目标不是 RISC-V 或 LoongArch 时（例如 x86_64 宿主测试），
//! 提供 mock 实现使得架构无关代码可以编译和测试。

use core::sync::atomic::{AtomicUsize, Ordering};

// ---- constant ----

pub mod constant {
    pub const SSTATUS_SIE: usize = 1;
    pub const ARCH: &str = "mock";
    pub const USER_TOP: usize = 0x7fff_ffff_ffff;
    pub const SUPERVISOR_EXTERNAL: usize = 0;
    pub const SV39_BOT_HALF_TOP: usize = 0x8000_0000_0000;
    pub const STACK_ALIGN_MASK: usize = !0xf;
}

// ---- intr ----

pub mod intr {
    pub fn are_interrupts_enabled() -> bool {
        false
    }

    pub unsafe fn read_and_disable_interrupts() -> usize {
        0
    }

    pub unsafe fn restore_interrupts(_flags: usize) {}

    pub unsafe fn enable_interrupts() {}

    pub unsafe fn disable_interrupts() {}

    pub unsafe fn read_and_enable_interrupts() -> usize {
        0
    }

    pub fn enable_irq(_irq: usize) {}

    pub fn enable_software_interrupt() {}

    pub fn enable_timer_interrupt() {}
}

// ---- mm ----

pub mod mm {
    use crate::mm::address::{Paddr, Ppn, UsizeConvert, Vaddr, Vpn};
    use crate::mm::page_table::{
        PageSize, PageTableEntry as PageTableEntryTrait, PageTableInner as PageTableInnerTrait,
        PagingError, PagingResult, UniversalPTEFlag,
    };

    pub fn paddr_to_vaddr(pa: usize) -> usize {
        pa + super::constant::SV39_BOT_HALF_TOP
    }

    pub unsafe fn vaddr_to_paddr(va: usize) -> usize {
        va - super::constant::SV39_BOT_HALF_TOP
    }

    // ---- Mock PageTableEntry ----

    #[derive(Debug, Clone, Copy)]
    pub struct PageTableEntry {
        bits: u64,
    }

    impl PageTableEntryTrait for PageTableEntry {
        type Bits = u64;

        fn from_bits(bits: u64) -> Self {
            Self { bits }
        }

        fn to_bits(&self) -> u64 {
            self.bits
        }

        fn empty() -> Self {
            Self { bits: 0 }
        }

        fn new_leaf(ppn: Ppn, flags: UniversalPTEFlag) -> Self {
            Self {
                bits: ((ppn.as_usize() as u64) << 10) | (flags.bits() as u64),
            }
        }

        fn new_table(ppn: Ppn) -> Self {
            Self {
                bits: ((ppn.as_usize() as u64) << 10) | (UniversalPTEFlag::VALID.bits() as u64),
            }
        }

        fn is_valid(&self) -> bool {
            self.bits & (UniversalPTEFlag::VALID.bits() as u64) != 0
        }

        fn is_huge(&self) -> bool {
            false
        }

        fn is_empty(&self) -> bool {
            self.bits == 0
        }

        fn ppn(&self) -> Ppn {
            Ppn::from_usize(((self.bits >> 10) & ((1u64 << 44) - 1)) as usize)
        }

        fn flags(&self) -> UniversalPTEFlag {
            UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize)
        }

        fn set_ppn(&mut self, ppn: Ppn) {
            let flags = self.bits & 0xff;
            self.bits = ((ppn.as_usize() as u64) << 10) | flags;
        }

        fn set_flags(&mut self, flags: UniversalPTEFlag) {
            let ppn_bits = self.bits & !0xff;
            self.bits = ppn_bits | (flags.bits() as u64);
        }

        fn clear(&mut self) {
            self.bits = 0;
        }

        fn remove_flags(&mut self, flags: UniversalPTEFlag) {
            let current = UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize);
            let updated = current.difference(flags);
            self.bits = (self.bits & !0xff) | (updated.bits() as u64);
        }

        fn add_flags(&mut self, flags: UniversalPTEFlag) {
            let current = UniversalPTEFlag::from_bits_truncate((self.bits & 0xff) as usize);
            let updated = current.union(flags);
            self.bits = (self.bits & !0xff) | (updated.bits() as u64);
        }
    }

    // ---- Mock PageTableInner ----

    #[derive(Debug)]
    pub struct PageTableInner {
        root: Ppn,
        is_user: bool,
    }

    impl PageTableInnerTrait<PageTableEntry> for PageTableInner {
        const LEVELS: usize = 3;
        const MAX_VA_BITS: usize = 39;
        const MAX_PA_BITS: usize = 56;

        fn tlb_flush(_vpn: Vpn) {}

        fn tlb_flush_all() {}

        fn is_user_table(&self) -> bool {
            self.is_user
        }

        fn activate(_ppn: Ppn) {}

        fn activating_table_ppn() -> Ppn {
            Ppn::from_usize(0)
        }

        fn new() -> Self {
            Self {
                root: Ppn::from_usize(0x80000),
                is_user: true,
            }
        }

        fn from_ppn(ppn: Ppn) -> Self {
            Self {
                root: ppn,
                is_user: true,
            }
        }

        fn new_as_kernel_table() -> Self {
            Self {
                root: Ppn::from_usize(0x80000),
                is_user: false,
            }
        }

        fn root_ppn(&self) -> Ppn {
            self.root
        }

        fn get_entry(&self, _vpn: Vpn, _level: usize) -> Option<(PageTableEntry, PageSize)> {
            None
        }

        fn translate(&self, _vaddr: Vaddr) -> Option<Paddr> {
            None
        }

        fn map(
            &mut self,
            _vpn: Vpn,
            _ppn: Ppn,
            _page_size: PageSize,
            _flags: UniversalPTEFlag,
        ) -> PagingResult<()> {
            Ok(())
        }

        fn unmap(&mut self, _vpn: Vpn) -> PagingResult<()> {
            Ok(())
        }

        fn mvmap(
            &mut self,
            _vpn: Vpn,
            target_ppn: Ppn,
            _page_size: PageSize,
            _flags: UniversalPTEFlag,
        ) -> PagingResult<()> {
            self.root = target_ppn;
            Ok(())
        }

        fn update_flags(&mut self, _vpn: Vpn, _flags: UniversalPTEFlag) -> PagingResult<()> {
            Ok(())
        }

        fn walk(&self, _vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)> {
            Err(PagingError::NotMapped)
        }
    }

    // Batch methods (non-trait, architecture-specific helpers)

    impl PageTableInner {
        pub fn map_with_batch(
            &mut self,
            vpn: Vpn,
            ppn: Ppn,
            page_size: PageSize,
            flags: UniversalPTEFlag,
            _batch: Option<&mut TlbBatchContext>,
        ) -> PagingResult<()> {
            <Self as PageTableInnerTrait<PageTableEntry>>::map(self, vpn, ppn, page_size, flags)
        }

        pub fn unmap_with_batch(
            &mut self,
            vpn: Vpn,
            _batch: Option<&mut TlbBatchContext>,
        ) -> PagingResult<()> {
            <Self as PageTableInnerTrait<PageTableEntry>>::unmap(self, vpn)
        }

        pub fn update_flags_with_batch(
            &mut self,
            vpn: Vpn,
            flags: UniversalPTEFlag,
            _batch: Option<&mut TlbBatchContext>,
        ) -> PagingResult<()> {
            <Self as PageTableInnerTrait<PageTableEntry>>::update_flags(self, vpn, flags)
        }
    }

    // ---- TlbBatchContext ----

    pub struct TlbBatchContext {
        enabled: bool,
    }

    impl TlbBatchContext {
        pub fn new() -> Self {
            Self { enabled: false }
        }

        pub fn execute<F, R>(f: F) -> R
        where
            F: FnOnce(&mut Self) -> R,
        {
            let mut ctx = Self::new();
            let result = f(&mut ctx);
            drop(ctx);
            result
        }

        pub fn flush(&mut self) {}
    }

    impl Drop for TlbBatchContext {
        fn drop(&mut self) {}
    }
}

// ---- trap ----

pub mod trap {
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
}

// ---- kernel ----

pub mod kernel {
    pub mod context {
        /// 上下文信息 — 在 mock 中，字段与 RISC-V 版本一致以保证布局
        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        pub struct Context {
            pub ra: usize,
            pub sp: usize,
            pub s: [usize; 12],
        }

        impl Context {
            pub fn zero_init() -> Self {
                Context {
                    ra: 0,
                    sp: 0,
                    s: [0; 12],
                }
            }

            pub fn set_init_context(&mut self, entry: usize, kstack_top: usize) {
                self.sp = kstack_top;
                self.ra = entry;
            }
        }
    }

    pub mod cpu {
        pub fn cpu_id() -> usize {
            0
        }

        pub fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize) {
            if trap_frame_ptr != 0 {
                unsafe {
                    if let Some(tf) = (trap_frame_ptr as *mut super::super::trap::TrapFrame).as_mut()
                    {
                        tf.cpu_ptr = cpu_ptr;
                    }
                }
            }
        }
    }

    pub mod task {
        pub fn setup_stack_layout(
            sp: usize,
            _argv: &[&str],
            _envp: &[&str],
            _phdr_addr: usize,
            _phnum: usize,
            _phent: usize,
            _at_base: usize,
            _at_entry: usize,
        ) -> (usize, usize, usize, usize) {
            let sp = sp & !(core::mem::size_of::<usize>() - 1);
            (sp - 1024, 0, 0, 0)
        }
    }

    pub unsafe fn switch(_old: *mut context::Context, _new: *const context::Context) {}
}

// ---- timer ----

pub mod timer {
    use core::sync::atomic::AtomicUsize;

    pub const TICKS_PER_SEC: usize = 100;
    pub const MSEC_PER_SEC: usize = 1000;
    pub static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

    pub fn get_ticks() -> usize {
        TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_time() -> usize {
        0
    }

    pub fn get_time_ms() -> usize {
        0
    }

    pub fn clock_freq() -> usize {
        10_000_000
    }

    pub fn set_next_trigger() {}

    pub fn init() {}
}

// ---- platform ----

pub mod platform {
    pub const MEMORY_END: usize = 0x8800_0000;
    pub const VIRT_CPUS_MAX: usize = 4;

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub enum VirtDevice {
        VirtDebug,
        VirtMrom,
        VirtTest,
        VirtRtc,
        VirtClint,
        VirtAclintSswi,
        VirtPlic,
        VirtAplicM,
        VirtAplicS,
        VirtUart0,
        VirtVirtio,
        VirtFwCfg,
        VirtImsicM,
        VirtImsicS,
        VirtFlash,
        VirtPciePio,
        VirtIommuSys,
        VirtPlatformBus,
        VirtPcieEcam,
        VirtPcieMmio,
        VirtDram,
    }

    pub fn mmio_of(_dev: VirtDevice) -> Option<(usize, usize)> {
        None
    }

    pub fn init() {}

    /// 兼容性模块别名
    pub mod virt {
        pub use super::*;
    }
}

// ---- lib ----

pub mod lib {
    pub fn shutdown(_failure: bool) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    pub fn send_ipi(_hart_mask: usize) {}

    pub fn set_timer(_time: usize) {}
}

// ---- syscall ----

pub mod syscall {
    #![allow(dead_code)]
    pub const SYS_IO_SETUP: usize = 0;
    pub const SYS_IO_DESTROY: usize = 1;
    pub const SYS_IO_SUBMIT: usize = 2;
    pub const SYS_IO_CANCEL: usize = 3;
    pub const SYS_IO_GETEVENTS: usize = 4;
    pub const SYS_SETXATTR: usize = 5;
    pub const SYS_LSETXATTR: usize = 6;
    pub const SYS_FSETXATTR: usize = 7;
    pub const SYS_GETXATTR: usize = 8;
    pub const SYS_LGETXATTR: usize = 9;
    pub const SYS_FGETXATTR: usize = 10;
    pub const SYS_LISTXATTR: usize = 11;
    pub const SYS_LLISTXATTR: usize = 12;
    pub const SYS_FLISTXATTR: usize = 13;
    pub const SYS_REMOVEXATTR: usize = 14;
    pub const SYS_LREMOVEXATTR: usize = 15;
    pub const SYS_FREMOVEXATTR: usize = 16;
    pub const SYS_GETCWD: usize = 17;
    pub const SYS_LOOKUP_DCOOKIE: usize = 18;
    pub const SYS_EVENTFD2: usize = 19;
    pub const SYS_EPOLL_CREATE1: usize = 20;
    pub const SYS_EPOLL_CTL: usize = 21;
    pub const SYS_EPOLL_PWAIT: usize = 22;
    pub const SYS_DUP: usize = 23;
    pub const SYS_DUP3: usize = 24;
    pub const SYS_FCNTL: usize = 25;
    pub const SYS_INOTIFY_INIT1: usize = 26;
    pub const SYS_INOTIFY_ADD_WATCH: usize = 27;
    pub const SYS_INOTIFY_RM_WATCH: usize = 28;
    pub const SYS_IOCTL: usize = 29;
    pub const SYS_IOPRIO_SET: usize = 30;
    pub const SYS_IOPRIO_GET: usize = 31;
    pub const SYS_FLOCK: usize = 32;
    pub const SYS_MKNODAT: usize = 33;
    pub const SYS_MKDIRAT: usize = 34;
    pub const SYS_UNLINKAT: usize = 35;
    pub const SYS_SYMLINKAT: usize = 36;
    pub const SYS_LINKAT: usize = 37;
    pub const SYS_UMOUNT2: usize = 39;
    pub const SYS_MOUNT: usize = 40;
    pub const SYS_PIVOT_ROOT: usize = 41;
    pub const SYS_NFSSERVCTL: usize = 42;
    pub const SYS_STATFS: usize = 43;
    pub const SYS_FSTATFS: usize = 44;
    pub const SYS_TRUNCATE: usize = 45;
    pub const SYS_FTRUNCATE: usize = 46;
    pub const SYS_FALLOCATE: usize = 47;
    pub const SYS_FACCESSAT: usize = 48;
    pub const SYS_CHDIR: usize = 49;
    pub const SYS_FCHDIR: usize = 50;
    pub const SYS_CHROOT: usize = 51;
    pub const SYS_FCHMOD: usize = 52;
    pub const SYS_FCHMODAT: usize = 53;
    pub const SYS_FCHOWNAT: usize = 54;
    pub const SYS_FCHOWN: usize = 55;
    pub const SYS_OPENAT: usize = 56;
    pub const SYS_CLOSE: usize = 57;
    pub const SYS_VHANGUP: usize = 58;
    pub const SYS_PIPE2: usize = 59;
    pub const SYS_QUOTACTL: usize = 60;
    pub const SYS_GETDENTS64: usize = 61;
    pub const SYS_LSEEK: usize = 62;
    pub const SYS_READ: usize = 63;
    pub const SYS_WRITE: usize = 64;
    pub const SYS_READV: usize = 65;
    pub const SYS_WRITEV: usize = 66;
    pub const SYS_PREAD64: usize = 67;
    pub const SYS_PWRITE64: usize = 68;
    pub const SYS_PREADV: usize = 69;
    pub const SYS_PWRITEV: usize = 70;
    pub const SYS_SENDFILE: usize = 71;
    pub const SYS_PSELECT6: usize = 72;
    pub const SYS_PPOLL: usize = 73;
    pub const SYS_SIGNALFD4: usize = 74;
    pub const SYS_VMSPLICE: usize = 75;
    pub const SYS_SPLICE: usize = 76;
    pub const SYS_TEE: usize = 77;
    pub const SYS_READLINKAT: usize = 78;
    pub const SYS_FSTATAT: usize = 79;
    pub const SYS_FSTAT: usize = 80;
    pub const SYS_SYNC: usize = 81;
    pub const SYS_FSYNC: usize = 82;
    pub const SYS_FDATASYNC: usize = 83;
    pub const SYS_SYNC_FILE_RANGE: usize = 84;
    pub const SYS_TIMERFD_CREATE: usize = 85;
    pub const SYS_TIMERFD_SETTIME: usize = 86;
    pub const SYS_TIMERFD_GETTIME: usize = 87;
    pub const SYS_UTIMENSAT: usize = 88;
    pub const SYS_ACCT: usize = 89;
    pub const SYS_CAPGET: usize = 90;
    pub const SYS_CAPSET: usize = 91;
    pub const SYS_PERSONALITY: usize = 92;
    pub const SYS_EXIT: usize = 93;
    pub const SYS_EXIT_GROUP: usize = 94;
    pub const SYS_WAITID: usize = 95;
    pub const SYS_SET_TID_ADDRESS: usize = 96;
    pub const SYS_UNSHARE: usize = 97;
    pub const SYS_FUTEX: usize = 98;
    pub const SYS_SET_ROBUST_LIST: usize = 99;
    pub const SYS_GET_ROBUST_LIST: usize = 100;
    pub const SYS_NANOSLEEP: usize = 101;
    pub const SYS_GETITIMER: usize = 102;
    pub const SYS_SETITIMER: usize = 103;
    pub const SYS_KEXEC_LOAD: usize = 104;
    pub const SYS_INIT_MODULE: usize = 105;
    pub const SYS_DELETE_MODULE: usize = 106;
    pub const SYS_TIMER_CREATE: usize = 107;
    pub const SYS_TIMER_GETTIME: usize = 108;
    pub const SYS_TIMER_GETOVERRUN: usize = 109;
    pub const SYS_TIMER_SETTIME: usize = 110;
    pub const SYS_TIMER_DELETE: usize = 111;
    pub const SYS_CLOCK_SETTIME: usize = 112;
    pub const SYS_CLOCK_GETTIME: usize = 113;
    pub const SYS_CLOCK_GETRES: usize = 114;
    pub const SYS_CLOCK_NANOSLEEP: usize = 115;
    pub const SYS_SYSLOG: usize = 116;
    pub const SYS_PTRACE: usize = 117;
    pub const SYS_SCHED_SETPARAM: usize = 118;
    pub const SYS_SCHED_SETSCHEDULER: usize = 119;
    pub const SYS_SCHED_GETSCHEDULER: usize = 120;
    pub const SYS_SCHED_GETPARAM: usize = 121;
    pub const SYS_SCHED_SETAFFINITY: usize = 122;
    pub const SYS_SCHED_GETAFFINITY: usize = 123;
    pub const SYS_SCHED_YIELD: usize = 124;
    pub const SYS_SCHED_GET_PRIORITY_MAX: usize = 125;
    pub const SYS_SCHED_GET_PRIORITY_MIN: usize = 126;
    pub const SYS_SCHED_RR_GET_INTERVAL: usize = 127;
    pub const SYS_RESTART_SYSCALL: usize = 128;
    pub const SYS_KILL: usize = 129;
    pub const SYS_TKILL: usize = 130;
    pub const SYS_TGKILL: usize = 131;
    pub const SYS_SIGALTSTACK: usize = 132;
    pub const SYS_RT_SIGSUSPEND: usize = 133;
    pub const SYS_RT_SIGACTION: usize = 134;
    pub const SYS_RT_SIGPROCMASK: usize = 135;
    pub const SYS_RT_SIGPENDING: usize = 136;
    pub const SYS_RT_SIGTIMEDWAIT: usize = 137;
    pub const SYS_RT_SIGQUEUEINFO: usize = 138;
    pub const SYS_RT_SIGRETURN: usize = 139;
    pub const SYS_SETPRIORITY: usize = 140;
    pub const SYS_GETPRIORITY: usize = 141;
    pub const SYS_REBOOT: usize = 142;
    pub const SYS_SETREGID: usize = 143;
    pub const SYS_SETGID: usize = 144;
    pub const SYS_SETREUID: usize = 145;
    pub const SYS_SETUID: usize = 146;
    pub const SYS_SETRESUID: usize = 147;
    pub const SYS_GETRESUID: usize = 148;
    pub const SYS_SETRESGID: usize = 149;
    pub const SYS_GETRESGID: usize = 150;
    pub const SYS_SETFSUID: usize = 151;
    pub const SYS_SETFSGID: usize = 152;
    pub const SYS_TIMES: usize = 153;
    pub const SYS_SETPGID: usize = 154;
    pub const SYS_GETPGID: usize = 155;
    pub const SYS_GETSID: usize = 156;
    pub const SYS_SETSID: usize = 157;
    pub const SYS_GETGROUPS: usize = 158;
    pub const SYS_SETGROUPS: usize = 159;
    pub const SYS_UNAME: usize = 160;
    pub const SYS_SETHOSTNAME: usize = 161;
    pub const SYS_SETDOMAINNAME: usize = 162;
    pub const SYS_GETRLIMIT: usize = 163;
    pub const SYS_SETRLIMIT: usize = 164;
    pub const SYS_GETRUSAGE: usize = 165;
    pub const SYS_UMASK: usize = 166;
    pub const SYS_PRCTL: usize = 167;
    pub const SYS_GETCPU: usize = 168;
    pub const SYS_GETTIMEOFDAY: usize = 169;
    pub const SYS_SETTIMEOFDAY: usize = 170;
    pub const SYS_ADJTIMEX: usize = 171;
    pub const SYS_GETPID: usize = 172;
    pub const SYS_GETPPID: usize = 173;
    pub const SYS_GETUID: usize = 174;
    pub const SYS_GETEUID: usize = 175;
    pub const SYS_GETGID: usize = 176;
    pub const SYS_GETEGID: usize = 177;
    pub const SYS_GETTID: usize = 178;
    pub const SYS_SYSINFO: usize = 179;
    pub const SYS_MQ_OPEN: usize = 180;
    pub const SYS_MQ_UNLINK: usize = 181;
    pub const SYS_MQ_TIMEDSEND: usize = 182;
    pub const SYS_MQ_TIMEDRECEIVE: usize = 183;
    pub const SYS_MQ_NOTIFY: usize = 184;
    pub const SYS_MQ_GETSETATTR: usize = 185;
    pub const SYS_MSGGET: usize = 186;
    pub const SYS_MSGCTL: usize = 187;
    pub const SYS_MSGRCV: usize = 188;
    pub const SYS_MSGSND: usize = 189;
    pub const SYS_SEMGET: usize = 190;
    pub const SYS_SEMCTL: usize = 191;
    pub const SYS_SEMTIMEDOP: usize = 192;
    pub const SYS_SEMOP: usize = 193;
    pub const SYS_SHMGET: usize = 194;
    pub const SYS_SHMCTL: usize = 195;
    pub const SYS_SHMAT: usize = 196;
    pub const SYS_SHMDT: usize = 197;
    pub const SYS_SOCKET: usize = 198;
    pub const SYS_SOCKETPAIR: usize = 199;
    pub const SYS_BIND: usize = 200;
    pub const SYS_LISTEN: usize = 201;
    pub const SYS_ACCEPT: usize = 202;
    pub const SYS_CONNECT: usize = 203;
    pub const SYS_GETSOCKNAME: usize = 204;
    pub const SYS_GETPEERNAME: usize = 205;
    pub const SYS_SENDTO: usize = 206;
    pub const SYS_RECVFROM: usize = 207;
    pub const SYS_SETSOCKOPT: usize = 208;
    pub const SYS_GETSOCKOPT: usize = 209;
    pub const SYS_SHUTDOWN: usize = 210;
    pub const SYS_SENDMSG: usize = 211;
    pub const SYS_RECVMSG: usize = 212;
    pub const SYS_READAHEAD: usize = 213;
    pub const SYS_BRK: usize = 214;
    pub const SYS_MUNMAP: usize = 215;
    pub const SYS_MREMAP: usize = 216;
    pub const SYS_ADD_KEY: usize = 217;
    pub const SYS_REQUEST_KEY: usize = 218;
    pub const SYS_KEYCTL: usize = 219;
    pub const SYS_CLONE: usize = 220;
    pub const SYS_EXECVE: usize = 221;
    pub const SYS_MMAP: usize = 222;
    pub const SYS_FADVISE64: usize = 223;
    pub const SYS_SWAPON: usize = 224;
    pub const SYS_SWAPOFF: usize = 225;
    pub const SYS_MPROTECT: usize = 226;
    pub const SYS_MSYNC: usize = 227;
    pub const SYS_MLOCK: usize = 228;
    pub const SYS_MUNLOCK: usize = 229;
    pub const SYS_MLOCKALL: usize = 230;
    pub const SYS_MUNLOCKALL: usize = 231;
    pub const SYS_MINCORE: usize = 232;
    pub const SYS_MADVISE: usize = 233;
    pub const SYS_REMAP_FILE_PAGES: usize = 234;
    pub const SYS_MBIND: usize = 235;
    pub const SYS_GET_MEMPOLICY: usize = 236;
    pub const SYS_SET_MEMPOLICY: usize = 237;
    pub const SYS_MIGRATE_PAGES: usize = 238;
    pub const SYS_MOVE_PAGES: usize = 239;
    pub const SYS_RT_TGSIGQUEUEINFO: usize = 240;
    pub const SYS_PERF_EVENT_OPEN: usize = 241;
    pub const SYS_ACCEPT4: usize = 242;
    pub const SYS_RECVMMSG: usize = 243;
    pub const SYS_ARCH_SPECIFIC_SYSCALL: usize = 244;
    pub const SYS_WAIT4: usize = 260;
    pub const SYS_PRLIMIT64: usize = 261;
    pub const SYS_FANOTIFY_INIT: usize = 262;
    pub const SYS_FANOTIFY_MARK: usize = 263;
    pub const SYS_NAME_TO_HANDLE_AT: usize = 264;
    pub const SYS_OPEN_BY_HANDLE_AT: usize = 265;
    pub const SYS_CLOCK_ADJTIME: usize = 266;
    pub const SYS_SYNCFS: usize = 267;
    pub const SYS_SETNS: usize = 268;
    pub const SYS_SENDMMSG: usize = 269;
    pub const SYS_PROCESS_VM_READV: usize = 270;
    pub const SYS_PROCESS_VM_WRITEV: usize = 271;
    pub const SYS_KCMP: usize = 272;
    pub const SYS_FINIT_MODULE: usize = 273;
    pub const SYS_SCHED_SETATTR: usize = 274;
    pub const SYS_SCHED_GETATTR: usize = 275;
    pub const SYS_RENAMEAT2: usize = 276;
    pub const SYS_SECCOMP: usize = 277;
    pub const SYS_GETRANDOM: usize = 278;
    pub const SYS_MEMFD_CREATE: usize = 279;
    pub const SYS_BPF: usize = 280;
    pub const SYS_EXECVEAT: usize = 281;
    pub const SYS_USERFAULTFD: usize = 282;
    pub const SYS_MEMBARRIER: usize = 283;
    pub const SYS_MLOCK2: usize = 284;
    pub const SYS_COPY_FILE_RANGE: usize = 285;
    pub const SYS_PREADV2: usize = 286;
    pub const SYS_PWRITEV2: usize = 287;
    pub const SYS_PKEY_MPROTECT: usize = 288;
    pub const SYS_PKEY_ALLOC: usize = 289;
    pub const SYS_PKEY_FREE: usize = 290;
    pub const SYS_STATX: usize = 291;
    pub const SYS_SYSRISCV: usize = 244;
    pub const SYS_RISCV_FLUSH_ICACHE: usize = 259;
    pub const SYS_GETIFADDRS: usize = 500;
}

// ---- ipi ----

pub mod ipi {
    pub fn send_reschedule_ipi(_cpu: usize) {}

    pub fn send_tlb_flush_ipi_all() {}

    pub fn handle_ipi() {}
}

// ---- boot ----

pub mod boot {
    pub fn main(_hartid: usize) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}
