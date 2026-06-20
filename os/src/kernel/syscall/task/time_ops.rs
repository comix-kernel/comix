use super::*;

/// 高精度睡眠（纳秒级别）
/// # 参数
/// - `duration`: 指向 TimeSpec 结构体的指针, 包含睡眠的时间
/// - `rem`: 指向 TimeSpec 结构体的指针, 用于存储剩余的睡眠时间, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn nanosleep(duration: *const TimeSpec, rem: *mut TimeSpec) -> c_int {
    let req = unsafe { read_from_user(duration) };
    if req.tv_sec == 0 && req.tv_nsec == 0 {
        return 0;
    }
    if req.tv_sec < 0 || req.tv_nsec < 0 || req.tv_nsec > 999999999 {
        return -EINVAL;
    }
    let mut result = 0;
    let task = current_task();
    let trigger = get_time() + req.into_freq(clock_freq());

    let mut timer_q = TIMER_QUEUE.lock();
    timer_q.push(trigger, task.clone());
    sleep_task(task.clone(), true);
    drop(timer_q);
    yield_task();

    // 被唤醒后（可能是超时，也可能是信号），必须确保将任务从定时器队列中清理掉
    // 如果是超时唤醒，pop_due_task 已经移除了
    // 如果是信号唤醒，任务还在队列中，需要手动移除以避免 Arc 泄漏
    TIMER_QUEUE.lock().remove_task(&task);

    if !rem.is_null() {
        let dur = trigger.saturating_sub(get_time());
        let remaining_ticks = if dur > 0 {
            // XXX: 提前唤醒是否一定是因为信号？
            result = -EINTR;
            dur
        } else {
            0
        };
        let rem_ts = TimeSpec::from_freq(remaining_ticks, clock_freq());
        unsafe {
            write_to_user(rem, rem_ts);
        }
    }

    result
    // TODO: EFAULT
}

pub fn gettid() -> c_int {
    current_task().lock().tid as c_int
}

pub fn sched_yield() -> c_int {
    yield_task();
    0
}

/// 基于时钟的高精度睡眠
/// # 参数
/// - `clk_id`: 时钟 ID
/// - `flags`: 睡眠选项标志
/// - `req`: 指向 TimeSpec 结构体的指针, 包含睡眠的时间
/// - `rem`: 指向 TimeSpec 结构体的指针, 用于存储剩余的睡眠时间, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn clock_nanosleep(
    clk_id: c_int,
    flags: c_int,
    req: *const TimeSpec,
    rem: *mut TimeSpec,
) -> c_int {
    let time_req = unsafe { read_from_user(req) };
    let is_abstime = (flags & TIMER_ABSTIME) != 0;
    let sleep_ticks = time_req.into_freq(clock_freq());
    let trigger = if is_abstime {
        sleep_ticks
    } else {
        let now = match clk_id {
            CLOCK_REALTIME => REALTIME.read().into_freq(clock_freq()),
            CLOCK_MONOTONIC => get_time(),
            CLOCK_TAI | CLOCK_BOOTTIME | CLOCK_PROCESS_CPUTIME_ID => return -ENOSYS,
            _ => return -EINVAL,
        };
        now.saturating_add(sleep_ticks)
    };

    let mut result = 0;
    let task = current_task();

    let mut timer_q = TIMER_QUEUE.lock();
    timer_q.push(trigger, task.clone());
    sleep_task(task.clone(), true);
    drop(timer_q);
    yield_task();

    // 同 sys_nanosleep，防止信号唤醒导致的 Arc 泄漏
    TIMER_QUEUE.lock().remove_task(&task);

    if !rem.is_null() {
        let dur = trigger.saturating_sub(get_time());
        let remaining_ticks = if dur > 0 {
            // XXX: 提前唤醒是否一定是因为信号？
            result = -EINTR;
            dur
        } else {
            0
        };
        let rem_ts = TimeSpec::from_freq(remaining_ticks, clock_freq());
        unsafe {
            write_to_user(rem, rem_ts);
        }
    }

    result
}

/// 获取间隔定时器的当前值
/// # 参数
/// - `which`: 定时器 ID
/// - `curr_value`: 指向 Itimerval 结构体的指针, 用于存储当前定时器值
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn getitimer(which: c_int, curr_value: *mut Itimerval) -> c_int {
    match which {
        ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {}
        _ => return -EINVAL,
    }
    let sig = match which {
        ITIMER_REAL => NUM_SIGALRM,
        ITIMER_VIRTUAL => NUM_SIGVTALRM,
        ITIMER_PROF => NUM_SIGPROF,
        _ => unreachable!("getitimer: unreachable which case."),
    };
    // Linux semantics: ITIMER_* are per-process (thread group), not per-thread.
    // Use the thread-group leader (pid) as the timer owner.
    let owner = {
        let pid = current_task().lock().pid;
        TASK_MANAGER
            .lock()
            .get_task(pid)
            .unwrap_or_else(current_task)
    };
    let mut val = Itimerval::zero();
    if let Some(timer) = TIMER.lock().find_entry(&owner, sig) {
        let now = get_time();
        let remaining = (*timer.0).saturating_sub(now);
        let it_value = TimeSpec::from_freq(remaining, clock_freq()).to_timeval();
        let it_interval = timer.1.it_interval.to_timeval();
        val = Itimerval {
            it_value,
            it_interval,
        };
    }
    unsafe {
        write_to_user(curr_value, val);
    }
    0
}

/// 设置间隔定时器的值
/// # 参数
/// - `which`: 定时器 ID
/// - `new_value`: 指向 Itimerval 结构体的指针, 包含要设置的定时器值
/// - `old_value`: 指向 Itimerval 结构体的指针, 用于存储旧的定时器值, 可为 NULL
/// # 返回值
/// - 成功返回 0, 失败返回负错误码
pub fn setitimer(which: c_int, new_value: *const Itimerval, old_value: *mut Itimerval) -> c_int {
    match which {
        ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {}
        _ => return -EINVAL,
    }
    let sig = match which {
        ITIMER_REAL => NUM_SIGALRM,
        ITIMER_VIRTUAL => NUM_SIGVTALRM,
        ITIMER_PROF => NUM_SIGPROF,
        _ => unreachable!("setitimer: unreachable which case."),
    };

    // Linux semantics: ITIMER_* are per-process (thread group), not per-thread.
    let owner = {
        let pid = current_task().lock().pid;
        TASK_MANAGER
            .lock()
            .get_task(pid)
            .unwrap_or_else(current_task)
    };

    let mut binding = TIMER.lock();

    // Linux semantics: return the previous timer value, then replace it with the new one.
    let mut old = Itimerval::zero();
    if let Some(timer) = binding.find_entry(&owner, sig) {
        let now = get_time();
        let remaining = (*timer.0).saturating_sub(now);
        let it_value = TimeSpec::from_freq(remaining, clock_freq()).to_timeval();
        let it_interval = timer.1.it_interval.to_timeval();
        old = Itimerval {
            it_value,
            it_interval,
        };
    }

    // Disarm any existing timers for this (task, sig) so we don't accumulate duplicates.
    while binding.remove_entry(&owner, sig).is_some() {}

    let new_itimer = unsafe { read_from_user(new_value) };
    if !new_itimer.it_value.is_zero() {
        let trigger = get_time() + new_itimer.it_value.into_freq(clock_freq());
        let interval = new_itimer.it_interval.to_timespec();
        let entry = TimerEntry {
            task: owner,
            sig,
            it_interval: interval,
        };
        binding.push(trigger, entry);
    }
    if !old_value.is_null() {
        unsafe {
            write_to_user(old_value, old);
        }
    }

    0
}
