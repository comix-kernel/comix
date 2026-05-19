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
                if let Some(tf) = (trap_frame_ptr as *mut crate::arch::trap::TrapFrame).as_mut() {
                    tf.cpu_ptr = cpu_ptr;
                }
            }
        }
    }
}

pub mod task {
    use crate::arch::{address::VA, task::ExecStackLayout};
    use crate::mm::memory_space::MemorySpace;

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

    pub fn setup_exec_stack_layout(
        _space: &MemorySpace,
        sp: VA,
        argv: &[&str],
        envp: &[&str],
        phdr_addr: VA,
        phnum: usize,
        phent: usize,
        at_base: VA,
        at_entry: VA,
    ) -> ExecStackLayout {
        let (sp, argc, argv, envp) = setup_stack_layout(
            sp.as_usize(),
            argv,
            envp,
            phdr_addr.as_usize(),
            phnum,
            phent,
            at_base.as_usize(),
            at_entry.as_usize(),
        );
        ExecStackLayout {
            sp: VA::from_usize(sp),
            argc,
            argv: VA::from_usize(argv),
            envp: VA::from_usize(envp),
            tls: VA::null(),
        }
    }

    pub unsafe fn forkret_restore(
        tf_ptr: *mut crate::arch::trap::TrapFrame,
        _is_kernel_thread: bool,
    ) {
        unsafe { crate::arch::trap::restore(&*tf_ptr) };
    }

    pub unsafe fn init_kernel_trap_frame(
        tf_ptr: *mut crate::arch::trap::TrapFrame,
        entry: usize,
        terminal: usize,
        kernel_sp: usize,
    ) {
        unsafe {
            core::ptr::write(tf_ptr, crate::arch::trap::TrapFrame::zero_init());
            (*tf_ptr).set_kernel_trap_frame(entry, terminal, kernel_sp);
            crate::arch::trap::set_trap_frame_cpu_ptr(tf_ptr, 0);
        }
    }

    pub unsafe fn prepare_user_restore(
        _tfp: *mut crate::arch::trap::TrapFrame,
        _initial_pc: VA,
        _user_sp_high: VA,
    ) {
    }
}

pub unsafe fn switch(_old: *mut context::Context, _new: *const context::Context) {}
