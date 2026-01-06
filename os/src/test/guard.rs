use core::sync::atomic::{AtomicPtr, Ordering};

use crate::println;
static MOCK_HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

pub enum TestEnvironment {
    None,
    Interrupt(fn()),
}
pub struct TestEnvGuard {
    prev_flags: usize,
    prev_handler: Option<fn()>,
}

impl TestEnvGuard {
    pub fn enter(env: TestEnvironment) -> Self {
        unsafe fn read_flags() -> usize {
            // 在 `unsafe fn` 内调用 unsafe 函数也需要显式 unsafe 块
            unsafe { crate::arch::intr::read_and_disable_interrupts() }
        }

        // 在设置新环境前，先保存当前的状态
        let current_handler_ptr = MOCK_HANDLER.load(Ordering::Relaxed);
        let prev_handler = if current_handler_ptr.is_null() {
            None
        } else {
            // 将裸指针转换回函数指针 fn() 以便保存
            Some(unsafe { core::mem::transmute(current_handler_ptr) })
        };

        let mut guard = TestEnvGuard {
            prev_flags: unsafe { read_flags() },
            prev_handler, // 保存旧的 handler
        };

        match env {
            TestEnvironment::None => {}
            TestEnvironment::Interrupt(handler) => unsafe {
                crate::arch::intr::enable_interrupts();
            },
        }

        guard
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        if let Some(old_handler) = self.prev_handler {
            let old_ptr = old_handler as *mut ();
            println!("[mock] restoring previous interrupt handler {:p}", old_ptr);
            MOCK_HANDLER.store(old_ptr, core::sync::atomic::Ordering::Relaxed);
        } else {
            MOCK_HANDLER.store(core::ptr::null_mut(), core::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub fn trigger_mock_interrupt() {
    let ptr = MOCK_HANDLER.load(Ordering::Relaxed);
    if !ptr.is_null() {
        let f: fn() = unsafe { core::mem::transmute(ptr) };
        println!("[mock] triggering fake interrupt...");
        f();
    } else {
        println!("[mock] no handler registered");
    }
}
