//! 信号相关的系统调用实现

use core::ffi::{c_int, c_uint, c_ulong};

use crate::{
    ipc::do_sigpending,
    kernel::{current_task, syscall::util::wait_for_signal},
    tool::user_buffer::{read_from_user, write_to_user},
    uapi::{
        errno::{EINVAL, ENOSYS},
        signal::{
            NSIG, SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK, SIGSET_SIZE, SaFlags, SigInfoT,
            SignalAction, SignalFlags,
        },
        time::timespec,
        types::SigSetT,
    },
};

/// 修改当前任务的信号屏蔽字
/// # 参数：
/// * `how` - 指示如何修改屏蔽字的操作（SIG_BLOCK、SIG_UNBLOCK、SIG_SETMASK）
/// * `set` - 指向用户空间缓冲区的指针，包含要设置的信号集合
/// * `oset` - 指向用户空间缓冲区的指针，用于存放旧的信号集合
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn rt_sigprocmask(
    how: c_int,
    set: *const SigSetT,
    oset: *mut SigSetT,
    sigsetsize: c_uint,
) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }
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
pub fn rt_sigpending(uset: *mut SigSetT, sigsetsize: c_uint) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }
    let pending = do_sigpending();
    unsafe {
        write_to_user(uset, pending.bits() as SigSetT);
    }
    0
}

/// 更改指定信号的处理动作
/// # 参数：
/// * `signum` - 信号编号
/// * `act` - 指向新的 SignalAction 结构体的指针（如果不为 NULL）
/// * `oldact` - 指向用于存放旧的 SignalAction 结构体的指针（如果不为 NULL）
/// # 返回值：
/// * 成功时返回 0
/// * 失败时返回负的错误码
pub fn rt_sigaction(signum: c_int, act: *const SignalAction, oldact: *mut SignalAction) -> c_int {
    if signum <= 0 || signum as usize > NSIG {
        return -EINVAL;
    }

    let task = crate::kernel::current_task();
    let t = task.lock();

    if !oldact.is_null() {
        let current_action = t.signal_handlers.lock().actions[signum as usize].clone();
        unsafe {
            write_to_user(oldact, current_action);
        }
    }

    if !act.is_null() {
        let new_action = unsafe { read_from_user(act) };
        let flag = if let Some(flag) = SaFlags::from_bits(new_action.sa_flags as u32) {
            flag
        } else {
            return -EINVAL;
        };
        if !flag.is_known() {
            return -EINVAL;
        }
        if !flag.is_supported() {
            return -ENOSYS;
        }
        t.signal_handlers
            .lock()
            .set_action(signum as usize, new_action);
    }

    0
}

/// 实现实时信号等待
/// # 参数：
/// * `set` - 指向用户空间缓冲区的指针，包含要等待的信号集合
/// * `info` - 指向用户空间缓冲区的指针，用于存放信号信息
/// * `timeout` - 指向用户空间缓冲区的指针，包含超时时间
/// * `sigsetsize` - 信号集合的大小
/// # 返回值：
/// * 成功时返回收到的信号编号
/// * 失败时返回负的错误码
pub fn rt_sigtimedwait(
    set: *const SigSetT,
    info: *mut SigInfoT,
    timeout: *const timespec,
    sigsetsize: c_uint,
) -> c_int {
    if sigsetsize as usize != SIGSET_SIZE {
        return -EINVAL;
    }

    let wait_set_bits = unsafe { read_from_user(set) };
    let wait_set = if let Some(flags) = SignalFlags::from_bits(wait_set_bits as usize) {
        flags
    } else {
        return -EINVAL;
    };

    let timeout_opt = if !timeout.is_null() {
        let ts = unsafe { read_from_user(timeout) };
        Some(ts)
    } else {
        None
    };

    match wait_for_signal(current_task(), wait_set, timeout_opt) {
        Ok((sig_num, sig_info)) => {
            if !info.is_null() {
                unsafe {
                    write_to_user(info, sig_info);
                }
            }
            sig_num as c_int
        }
        Err(err_code) => err_code,
    }
}
