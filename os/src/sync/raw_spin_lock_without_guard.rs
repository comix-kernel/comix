//! Raw spin lock without guard, specifically designed for Global Allocator integration
//!
//! This module provides a spin lock implementation that integrates with `lock_api::RawMutex`
//! for use with the `talc` allocator's `Talck` type.
//!
//! # Key Differences from `RawSpinLock`
//!
//! - Implements `lock_api::RawMutex` trait
//! - Does not return a Guard from `lock()` method
//! - Stores interrupt state internally using `AtomicUsize`
//! - Unlock operation restores the interrupt state
//!
//! # Interrupt Safety
//!
//! This lock provides interrupt protection to prevent deadlocks when:
//! - A thread holds the allocator lock
//! - An interrupt occurs on the same CPU
//! - The interrupt handler tries to allocate memory
//!
//! Without interrupt protection, this would cause a deadlock.

use crate::arch::intr::{read_and_disable_interrupts, restore_interrupts};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Raw spin lock for Global Allocator, implementing `lock_api::RawMutex`
///
/// This lock combines spin-lock mechanism with interrupt protection,
/// storing the interrupt state internally for restoration on unlock.
///
/// # Usage
///
/// This type is specifically designed for use with `talc::Talck`:
/// ```ignore
/// use talc::{Talc, Talck};
/// static ALLOCATOR: Talck<RawSpinLockWithoutGuard, ClaimOnOom> = ...;
/// ```
pub struct RawSpinLockWithoutGuard {
    locked: AtomicBool,
    /// Stores the interrupt flags from the CPU when the lock was acquired
    /// Used to restore interrupt state on unlock
    saved_intr_flags: AtomicUsize,
}

impl RawSpinLockWithoutGuard {
    /// Create a new unlocked spin lock
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            saved_intr_flags: AtomicUsize::new(0),
        }
    }
}

unsafe impl lock_api::RawMutex for RawSpinLockWithoutGuard {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type GuardMarker = lock_api::GuardNoSend;

    fn lock(&self) {
        // 1. Disable interrupts and save the flags
        let flags = unsafe { read_and_disable_interrupts() };

        // 2. Spin until we acquire the lock
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        // 3. Store the interrupt flags for later restoration
        self.saved_intr_flags.store(flags, Ordering::Release);
    }

    fn try_lock(&self) -> bool {
        // 1. Disable interrupts and save the flags
        let flags = unsafe { read_and_disable_interrupts() };

        // 2. Try to acquire the lock
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // 3. Success: store the interrupt flags
            self.saved_intr_flags.store(flags, Ordering::Release);
            true
        } else {
            // 4. Failed: restore interrupts immediately
            unsafe { restore_interrupts(flags) };
            false
        }
    }

    unsafe fn unlock(&self) {
        // 1. Load the saved interrupt flags
        let flags = self.saved_intr_flags.load(Ordering::Acquire);

        // 2. Release the lock
        self.locked.store(false, Ordering::Release);

        // 3. Restore the interrupt state
        unsafe { restore_interrupts(flags) };
    }
}

// Safety: RawSpinLockWithoutGuard can be shared between threads
// (though in a single-core kernel, this is less relevant)
unsafe impl Send for RawSpinLockWithoutGuard {}
unsafe impl Sync for RawSpinLockWithoutGuard {}
