use super::*;

use crate::{
    arch::{Arch, ArchImpl, address::UA},
    kernel::task::Capabilities,
    uapi::{
        resource::ResourceId,
        sched::{
            SCHED_BATCH, SCHED_DEADLINE, SCHED_EXT, SCHED_FIFO, SCHED_IDLE, SCHED_NORMAL,
            SCHED_RESET_ON_FORK, SCHED_RR, SCHED_RT_PRIORITY_MAX, SCHED_RT_PRIORITY_MIN,
            SchedParam,
        },
    },
    util::user_buffer::{validate_user_ptr, validate_user_ptr_mut},
};

const CPU_SET_BYTES: usize = core::mem::size_of::<usize>();

fn get_target_task(pid: c_int) -> Result<SharedTask, c_int> {
    if pid < 0 {
        return Err(ESRCH);
    }
    if pid == 0 {
        return Ok(current_task());
    }
    TASK_MANAGER.lock().get_task(pid as u32).ok_or(ESRCH)
}

fn normalize_policy(policy: c_int) -> Result<(c_int, bool), c_int> {
    let reset = policy & SCHED_RESET_ON_FORK != 0;
    let base = policy & !SCHED_RESET_ON_FORK;
    match base {
        SCHED_NORMAL | SCHED_FIFO | SCHED_RR | SCHED_BATCH | SCHED_IDLE => Ok((base, reset)),
        SCHED_DEADLINE | SCHED_EXT => Err(EINVAL),
        _ => Err(EINVAL),
    }
}

fn validate_sched_param(policy: c_int, priority: c_int) -> Result<(), c_int> {
    match policy {
        SCHED_NORMAL | SCHED_BATCH | SCHED_IDLE => {
            if priority == 0 {
                Ok(())
            } else {
                Err(EINVAL)
            }
        }
        SCHED_FIFO | SCHED_RR => {
            if (SCHED_RT_PRIORITY_MIN..=SCHED_RT_PRIORITY_MAX).contains(&priority) {
                Ok(())
            } else {
                Err(EINVAL)
            }
        }
        _ => Err(EINVAL),
    }
}

fn check_sched_permission(target: &SharedTask, new_policy: c_int, new_priority: c_int) -> bool {
    let current = current_task();
    let (current_cred, current_rlimit) = {
        let cur = current.lock();
        (
            cur.credential,
            cur.rlimit.lock().limits[ResourceId::Rtprio as usize].rlim_cur as c_int,
        )
    };
    if current_cred.capabilities.has(Capabilities::SYS_NICE) {
        return true;
    }

    let (same_user, old_policy, old_priority, rtprio_limit) = {
        let target = target.lock();
        let same_user = current_cred.euid == target.credential.euid
            || current_cred.euid == target.credential.uid;
        (
            same_user,
            target.sched_policy,
            target.sched_priority,
            current_rlimit,
        )
    };

    if !same_user {
        return false;
    }

    match new_policy {
        SCHED_NORMAL | SCHED_BATCH | SCHED_IDLE => true,
        SCHED_FIFO | SCHED_RR => {
            rtprio_limit > 0
                && new_priority <= rtprio_limit
                && (old_policy == new_policy || new_priority <= old_priority || old_priority == 0)
        }
        _ => false,
    }
}

fn set_scheduler_common(pid: c_int, policy: Option<c_int>, param: *const SchedParam) -> c_int {
    if param.is_null() || !validate_user_ptr(param) {
        return -EFAULT;
    }
    let param = read_from_user(param);
    let task = match get_target_task(pid) {
        Ok(task) => task,
        Err(errno) => return -errno,
    };

    let (new_policy, reset_on_fork) = match policy {
        Some(raw_policy) => match normalize_policy(raw_policy) {
            Ok(v) => v,
            Err(errno) => return -errno,
        },
        None => {
            let t = task.lock();
            (t.sched_policy, t.sched_reset_on_fork)
        }
    };

    if let Err(errno) = validate_sched_param(new_policy, param.sched_priority) {
        return -errno;
    }
    if !check_sched_permission(&task, new_policy, param.sched_priority) {
        return -EPERM;
    }

    let mut t = task.lock();
    t.sched_policy = new_policy;
    t.sched_priority = param.sched_priority;
    t.priority = if param.sched_priority > 0 {
        (SCHED_RT_PRIORITY_MAX - param.sched_priority) as u8
    } else {
        0
    };
    if policy.is_some() {
        t.sched_reset_on_fork = reset_on_fork;
    }
    0
}

pub fn sched_setparam(pid: c_int, param: *const SchedParam) -> c_int {
    set_scheduler_common(pid, None, param)
}

pub fn sched_setscheduler(pid: c_int, policy: c_int, param: *const SchedParam) -> c_int {
    set_scheduler_common(pid, Some(policy), param)
}

pub fn sched_getscheduler(pid: c_int) -> c_int {
    let task = match get_target_task(pid) {
        Ok(task) => task,
        Err(errno) => return -errno,
    };
    let t = task.lock();
    let mut policy = t.sched_policy;
    if t.sched_reset_on_fork {
        policy |= SCHED_RESET_ON_FORK;
    }
    policy
}

pub fn sched_getparam(pid: c_int, param: *mut SchedParam) -> c_int {
    if param.is_null() || !validate_user_ptr_mut(param) {
        return -EFAULT;
    }
    let task = match get_target_task(pid) {
        Ok(task) => task,
        Err(errno) => return -errno,
    };
    let priority = task.lock().sched_priority;
    write_to_user(param, SchedParam {
        sched_priority: priority,
    });
    0
}

pub fn sched_setaffinity(pid: c_int, cpusetsize: usize, mask: *const u8) -> c_int {
    if cpusetsize == 0 || mask.is_null() {
        return -EINVAL;
    }
    let copy_len = core::cmp::min(cpusetsize, CPU_SET_BYTES);
    let mut raw = [0u8; CPU_SET_BYTES];
    // SAFETY: copy_from_user validates the user mapping while copying. `raw` is a
    // kernel stack buffer with at least `copy_len <= CPU_SET_BYTES` bytes.
    if unsafe {
        ArchImpl::copy_from_user(UA::from_usize(mask as usize), raw.as_mut_ptr(), copy_len)
    }
    .is_err()
    {
        return -EFAULT;
    }

    let requested = usize::from_ne_bytes(raw);
    let available = crate::kernel::online_cpu_mask();
    let normalized = requested & available;
    if normalized == 0 {
        return -EINVAL;
    }

    let task = match get_target_task(pid) {
        Ok(task) => task,
        Err(errno) => return -errno,
    };
    task.lock().cpu_affinity = normalized;
    0
}

pub fn sched_getaffinity(pid: c_int, cpusetsize: usize, mask: *mut u8) -> c_int {
    if cpusetsize < CPU_SET_BYTES || mask.is_null() {
        return -EINVAL;
    }
    let task = match get_target_task(pid) {
        Ok(task) => task,
        Err(errno) => return -errno,
    };
    let affinity = task.lock().cpu_affinity & crate::kernel::online_cpu_mask();
    let raw = affinity.to_ne_bytes();
    // SAFETY: `raw` is a live kernel buffer and copy_to_user validates the user
    // destination while copying `raw.len()` bytes.
    if unsafe { ArchImpl::copy_to_user(raw.as_ptr(), UA::from_usize(mask as usize), raw.len()) }
        .is_err()
    {
        return -EFAULT;
    }
    CPU_SET_BYTES as c_int
}
