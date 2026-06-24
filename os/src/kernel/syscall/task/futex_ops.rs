use super::*;
use crate::arch::Arch;

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
    _uaddr2: *mut u32,
    _val3: u32,
) -> c_int {
    let _private = (op & FUTEX_PRIVATE as c_int) != 0; // TODO: 目前不区分 PRIVATE 和 SHARED
    let realtime = (op & FUTEX_CLOCK_REALTIME as c_int) != 0;
    let op = op & !(FUTEX_PRIVATE as c_int) & !(FUTEX_CLOCK_REALTIME as c_int);
    // HACK: 其实只需要锁定与 uaddr 对应的 Futex 等待队列
    let mut fm = FUTEX_MANAGER.lock();
    match op as u32 {
        FUTEX_WAIT => {
            // 必须保证获取锁 → 定位等待队列 → 读取用户数据 → 比较 → 入队/释放锁 的序列是原子的
            let task = current_task();
            let Some(memory_space) = task.lock().memory_space.clone() else {
                return -EFAULT;
            };
            let paddr = if let Some(paddr) = memory_space
                .lock()
                .translate(VA::from_usize(uaddr as usize))
            {
                paddr.as_usize()
            } else {
                return -EFAULT;
            };
            let user_val = {
                let mut val = core::mem::MaybeUninit::<u32>::uninit();
                let copy_result = unsafe {
                    crate::arch::ArchImpl::copy_from_user(
                        UA::from_usize(uaddr as usize),
                        val.as_mut_ptr() as *mut u8,
                        core::mem::size_of::<u32>(),
                    )
                };
                if copy_result.is_err() {
                    return -EFAULT;
                }
                unsafe { val.assume_init() }
            };
            if user_val != val {
                return -EAGAIN;
            }

            let waitq = fm.get_wait_queue(paddr);
            waitq.sleep(task.clone());
            sleep_task(task.clone(), true);

            if !timeout.is_null() {
                let ts = unsafe { read_from_user(timeout) };
                if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec > 999999999 {
                    return -EINVAL;
                }
                let sleep_ticks = ts.into_freq(clock_freq());
                let trigger = if realtime {
                    let now = realtime_now().into_freq(clock_freq());
                    now.saturating_add(sleep_ticks)
                } else {
                    let now = get_time();
                    now.saturating_add(sleep_ticks)
                };
                TIMER_QUEUE.lock().push(trigger, task.clone());
                drop(fm);
                yield_task();
                if TIMER_QUEUE.lock().remove_task(&task).is_none() {
                    // 超时唤醒
                    let mut fm = FUTEX_MANAGER.lock();
                    let waitq = fm.get_wait_queue(paddr);
                    // 虽然任务已经被唤醒, 但仍然需要从等待队列中移除
                    waitq.remove_task(&task);
                    return -ETIMEDOUT;
                }
            } else {
                drop(fm);
                yield_task();
            }
            if signal_pending(&task) {
                // 信号唤醒
                let mut fm = FUTEX_MANAGER.lock();
                let waitq = fm.get_wait_queue(paddr);
                waitq.remove_task(&task);
                return -EINTR;
            }
            // 正常唤醒
            // NOTE: 此时任务已经不在等待队列中
            0
        }
        FUTEX_WAKE => {
            let mut wake_count = 0;
            let paddr = {
                let Some(memory_space) = current_task().lock().memory_space.clone() else {
                    return -EFAULT;
                };
                if let Some(paddr) = memory_space
                    .lock()
                    .translate(VA::from_usize(uaddr as usize))
                {
                    paddr.as_usize()
                } else {
                    return -EFAULT;
                }
            };
            let mut fm = FUTEX_MANAGER.lock();
            let waitq = fm.get_wait_queue(paddr);
            for _ in 0..val {
                waitq.wake_up_one();
                wake_count += 1;
            }
            wake_count
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
