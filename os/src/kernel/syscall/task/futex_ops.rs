use super::*;
use crate::{arch::Arch, kernel::WaitQueue};

fn futex_paddr(uaddr: *mut u32) -> Result<usize, c_int> {
    let Some(memory_space) = current_task().lock().memory_space.clone() else {
        return Err(-EFAULT);
    };
    memory_space
        .lock()
        .translate(VA::from_usize(uaddr as usize))
        .map(|pa| pa.as_usize())
        .ok_or(-EFAULT)
}

fn read_futex_word(uaddr: *mut u32) -> Result<u32, c_int> {
    let mut val = core::mem::MaybeUninit::<u32>::uninit();
    let copy_result = unsafe {
        crate::arch::ArchImpl::copy_from_user(
            UA::from_usize(uaddr as usize),
            val.as_mut_ptr() as *mut u8,
            core::mem::size_of::<u32>(),
        )
    };
    if copy_result.is_err() {
        return Err(-EFAULT);
    }
    Ok(unsafe { val.assume_init() })
}

fn timespec_to_timeout_ticks(
    timeout: *const TimeSpec,
    absolute: bool,
    realtime: bool,
) -> Result<Option<usize>, c_int> {
    if timeout.is_null() {
        return Ok(None);
    }

    let ts = unsafe { read_from_user(timeout) };
    if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec > 999999999 {
        return Err(-EINVAL);
    }

    let ticks = ts.into_freq(clock_freq());
    if absolute {
        if realtime {
            let realtime_now_ticks = realtime_now().into_freq(clock_freq());
            let mono_now = get_time();
            let rel = ticks.saturating_sub(realtime_now_ticks);
            Ok(Some(mono_now.saturating_add(rel)))
        } else {
            Ok(Some(ticks))
        }
    } else {
        Ok(Some(get_time().saturating_add(ticks)))
    }
}

fn futex_wait_common(
    uaddr: *mut u32,
    val: u32,
    timeout: *const TimeSpec,
    absolute_timeout: bool,
    realtime: bool,
) -> c_int {
    let task = current_task();
    let paddr = match futex_paddr(uaddr) {
        Ok(paddr) => paddr,
        Err(e) => return e,
    };

    let user_val = match read_futex_word(uaddr) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if user_val != val {
        return -EAGAIN;
    }

    let trigger = match timespec_to_timeout_ticks(timeout, absolute_timeout, realtime) {
        Ok(trigger) => trigger,
        Err(e) => return e,
    };
    if let Some(trigger) = trigger
        && trigger <= get_time()
    {
        return -ETIMEDOUT;
    }

    {
        let mut fm = FUTEX_MANAGER.lock();
        fm.get_wait_queue(paddr).add_task(task.clone());
        let slept = sleep_task_prepare(task.clone(), true, |t| {
            t.pending.has_deliverable_signal(t.blocked)
                || t.shared_pending.lock().has_deliverable_signal(t.blocked)
        });
        if !slept {
            fm.get_wait_queue(paddr).remove_task(&task);
            return -EINTR;
        }
    }

    if let Some(trigger) = trigger {
        TIMER_QUEUE.lock().push(trigger, task.clone());
    }

    yield_task();

    let timer_was_pending = if trigger.is_some() {
        TIMER_QUEUE.lock().remove_task(&task).is_some()
    } else {
        false
    };

    let still_waiting = {
        let mut fm = FUTEX_MANAGER.lock();
        let waitq = fm.get_wait_queue(paddr);
        let still_waiting = waitq.contains(&task);
        if still_waiting {
            waitq.remove_task(&task);
        }
        still_waiting
    };

    if signal_pending(&task) {
        return -EINTR;
    }

    if trigger.is_some() && still_waiting && !timer_was_pending {
        return -ETIMEDOUT;
    }

    0
}

fn futex_wake_common(uaddr: *mut u32, val: u32) -> c_int {
    let paddr = match futex_paddr(uaddr) {
        Ok(paddr) => paddr,
        Err(e) => return e,
    };
    let mut fm = FUTEX_MANAGER.lock();
    let waitq = fm.get_wait_queue(paddr);
    let mut wake_count = 0;
    for _ in 0..val {
        if waitq.is_empty() {
            break;
        }
        waitq.wake_up_one();
        wake_count += 1;
    }
    wake_count
}

fn requeue_waiters(src: &mut WaitQueue, dst: &mut WaitQueue, count: u32) -> c_int {
    let mut moved = 0;
    for _ in 0..count {
        let Some(task) = src.pop_task_no_wake() else {
            break;
        };
        dst.add_task(task);
        moved += 1;
    }
    moved
}

/// Futex 系统调用实现
/// # 参数
/// - `uaddr`: 指向用户空间中 futex 变量的指针
/// - `op`: 操作码和标志
/// - `val`: 操作相关的值
/// - `_timeout`: 指向 TimeSpec 结构体的指针, 用于指定超时时间
/// - `_uaddr2`: 指向用户空间中第二个 futex 变量的指针
/// - `_val3`: 额外的操作相关值
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn futex(
    uaddr: *mut u32,
    op: c_int,
    val: u32,
    timeout: *const TimeSpec,
    uaddr2: *mut u32,
    val3: u32,
) -> c_int {
    let _private = (op & FUTEX_PRIVATE as c_int) != 0; // TODO: 目前不区分 PRIVATE 和 SHARED
    let realtime = (op & FUTEX_CLOCK_REALTIME as c_int) != 0;
    let op = op & !(FUTEX_PRIVATE as c_int) & !(FUTEX_CLOCK_REALTIME as c_int);
    match op as u32 {
        FUTEX_WAIT => futex_wait_common(uaddr, val, timeout, false, realtime),
        FUTEX_WAIT_BITSET => {
            if val3 == 0 {
                return -EINVAL;
            }
            futex_wait_common(uaddr, val, timeout, true, realtime)
        }
        FUTEX_WAKE | FUTEX_WAKE_BITSET => {
            if op as u32 == FUTEX_WAKE_BITSET && val3 == 0 {
                return -EINVAL;
            }
            futex_wake_common(uaddr, val)
        }
        FUTEX_REQUEUE | FUTEX_CMP_REQUEUE => {
            if uaddr2.is_null() {
                return -EFAULT;
            }
            let paddr1 = match futex_paddr(uaddr) {
                Ok(paddr) => paddr,
                Err(e) => return e,
            };
            let paddr2 = match futex_paddr(uaddr2) {
                Ok(paddr) => paddr,
                Err(e) => return e,
            };
            let requeue_count = timeout as usize as u32;
            if op as u32 == FUTEX_CMP_REQUEUE {
                let cmp_val = val3;
                match read_futex_word(uaddr) {
                    Ok(v) if v == cmp_val => {}
                    Ok(_) => return -EAGAIN,
                    Err(e) => return e,
                }
            }

            let mut fm = FUTEX_MANAGER.lock();
            if paddr1 == paddr2 {
                let waitq = fm.get_wait_queue(paddr1);
                let mut changed = 0;
                for _ in 0..val {
                    if waitq.is_empty() {
                        break;
                    }
                    waitq.wake_up_one();
                    changed += 1;
                }
                return changed;
            }

            let mut src = fm.take_wait_queue(paddr1);
            let dst = fm.get_wait_queue(paddr2);
            let mut changed = 0;
            for _ in 0..val {
                if src.is_empty() {
                    break;
                }
                src.wake_up_one();
                changed += 1;
            }
            changed += requeue_waiters(&mut src, dst, requeue_count);
            fm.put_wait_queue(paddr1, src);
            changed
        }
        _ => -ENOSYS,
    }
}

//    long syscall(SYS_get_robust_list, int pid,
//                 struct robust_list_head **head_ptr, size_t *sizep);
//    long syscall(SYS_set_robust_list,
//                 struct robust_list_head *head, size_t size);

/// 设置线程 ID 地址
/// # 参数
/// - `tidptr`: 指向存储线程 ID 的用户空间地址
/// # 返回值
/// - 返回当前线程的线程 ID (TID)
pub fn set_tid_address(tidptr: *mut c_int) -> c_int {
    let task = current_task();
    task.lock().clear_child_tid = Some(UA::from_usize(tidptr as usize));
    current_task().lock().tid as c_int
}

/// 获取线程的 robust futex 列表头指针和大小
/// # 参数
/// - `pid`: 目标进程 ID, 为 0 表示当前进程
/// - `head_ptr`: 指向存储 robust futex 列表头指针的用户空间地址
/// - `sizep`: 指向存储 robust futex 列表大小的用户空间地址
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn get_robust_list(pid: c_int, head_ptr: *mut *mut RobustListHead, sizep: *mut SizeT) -> c_int {
    let task = if pid == 0 {
        current_task()
    } else {
        let tm = TASK_MANAGER.lock();
        match tm.get_task(pid as u32) {
            Some(t) => t,
            None => return -ESRCH,
        }
    };
    let (head, size) = {
        let t = task.lock();
        let head = match t.robust_list {
            Some(h) => h.as_usize() as *mut RobustListHead,
            None => core::ptr::null_mut(),
        };
        let size = size_of::<RobustListHead>() as SizeT;
        (head, size)
    };
    unsafe {
        write_to_user(head_ptr, head);
        write_to_user(sizep, size);
    }
    0
}

/// 设置线程的 robust futex 列表头指针
/// # 参数
/// - `head`: 指向 robust futex 列表头的指针
/// - `size`: robust futex 列表的大小
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn set_robust_list(head: *const RobustListHead, size: SizeT) -> c_int {
    if size != size_of::<RobustListHead>() as SizeT {
        return -EINVAL;
    }
    let task = current_task();
    task.lock().robust_list = Some(UA::from_usize(head as usize));
    0
}
