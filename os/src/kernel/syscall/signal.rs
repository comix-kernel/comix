//! 信号相关的系统调用实现

use core::ffi::{c_int, c_ulong};

use crate::{ipc::{SignalFlags, do_sigpending}, tool::user_buffer::{read_from_user, write_to_user}, uapi::{errno::EINVAL, signal::{SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK}}};

/// 修改当前任务的信号屏蔽字
/// # 参数：
/// * `how` - 指示如何修改屏蔽字的操作（SIG_BLOCK、SIG_UNBLOCK、SIG_SETMASK）
/// * `set` - 指向用户空间缓冲区的指针，包含要设置的信号集合
/// * `oset` - 指向用户空间缓冲区的指针，用于存放旧的信号集合
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn sigprocmask(how: c_int, set: *const c_ulong, oset: *mut c_ulong) -> c_int {
    let task = crate::kernel::current_task();
    let mut t = task.lock();

    if !oset.is_null() {
        let old_set = t.blocked.bits() as c_ulong;
        unsafe {
            write_to_user(oset, old_set);
        }
    }

    if !set.is_null() {
        let new_set = unsafe { read_from_user(set) };
        let new_flags = if let Some(flag) = SignalFlags::from_bits(new_set as usize) {
            flag
        } else {
            return -EINVAL;
        };

        match how {
            SIG_BLOCK => {
                t.blocked |= new_flags;
            }
            SIG_UNBLOCK => {
                t.blocked &= !new_flags;
            }
            SIG_SETMASK => {
                t.blocked = new_flags;
            }
            _ => {
                return -EINVAL;
            }
        }
    }

    0
}

/// 获取当前任务的待处理信号集合, 包括私有和共享的信号集合
/// # 参数：
/// * `uset` - 指向用户空间缓冲区的指针，用于存放待处理信号集合
pub fn sigpending(uset: *mut c_ulong) -> c_int {
    let pending = do_sigpending();
    unsafe {
        write_to_user(uset, pending.bits() as c_ulong);
    }
    0
}
