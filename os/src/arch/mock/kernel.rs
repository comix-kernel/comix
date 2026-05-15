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
    use crate::arch::task::ExecStackLayout;
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
        sp: usize,
        argv: &[&str],
        envp: &[&str],
        phdr_addr: usize,
        phnum: usize,
        phent: usize,
        at_base: usize,
        at_entry: usize,
    ) -> ExecStackLayout {
        let (sp, argc, argv, envp) =
            setup_stack_layout(sp, argv, envp, phdr_addr, phnum, phent, at_base, at_entry);
        ExecStackLayout {
            sp,
            argc,
            argv,
            envp,
            tls: 0,
        }
    }

    pub unsafe fn forkret_restore(
        tf_ptr: *mut crate::arch::trap::TrapFrame,
        _is_kernel_thread: bool,
    ) {
        unsafe { crate::arch::trap::restore(&*tf_ptr) };
    }

    pub unsafe fn prepare_user_restore(
        _tfp: *mut crate::arch::trap::TrapFrame,
        _initial_pc: usize,
        _user_sp_high: usize,
    ) {
    }
}

pub unsafe fn switch(_old: *mut context::Context, _new: *const context::Context) {}
