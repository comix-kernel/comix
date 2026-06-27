//! 任务相关的系统调用实现

use core::{
    ffi::{c_char, c_int, c_ulong, c_void},
    sync::atomic::Ordering,
};

use alloc::{string::ToString, sync::Arc, vec::Vec};

use crate::{
    arch::{
        HwTrapFrame, TrapFrame,
        address::UA,
        timer::{clock_freq, get_time},
    },
    ipc::{SignalHandlerTable, SignalPending, signal_pending},
    kernel::{
        FUTEX_MANAGER, Scheduler, SharedTask, TASK_MANAGER, TIMER, TIMER_QUEUE, TaskExitStatus,
        TaskManagerTrait, TaskState, TaskStruct, TimerEntry, current_cpu, current_task,
        exit_process, schedule, sleep_task, sleep_task_prepare,
        syscall::util::{get_args_safe, get_path_safe},
        time::{REALTIME, realtime_now},
        yield_task,
    },
    mm::{
        address::VA,
        frame_allocator::{alloc_contig_frames, alloc_frame},
        memory_space::MemorySpace,
    },
    sync::SpinLock,
    uapi::{
        errno::{
            EACCES, EAGAIN, EFAULT, EINTR, EINVAL, EIO, EISDIR, ENOENT, ENOEXEC, ENOMEM, ENOSYS,
            EPERM, ESRCH, ETIMEDOUT,
        },
        futex::{FUTEX_CLOCK_REALTIME, FUTEX_PRIVATE, FUTEX_WAIT, FUTEX_WAKE, RobustListHead},
        resource::{RLIM_NLIMITS, Rlimit, Rusage},
        sched::CloneFlags,
        signal::{NUM_SIGALRM, NUM_SIGPROF, NUM_SIGVTALRM},
        time::{
            Itimerval, TimeSpec,
            clock_flags::TIMER_ABSTIME,
            clock_id::{
                CLOCK_BOOTTIME, CLOCK_MONOTONIC, CLOCK_PROCESS_CPUTIME_ID, CLOCK_REALTIME,
                CLOCK_TAI,
            },
            itimer_id::{ITIMER_PROF, ITIMER_REAL, ITIMER_VIRTUAL},
        },
        types::{SizeT, StackT},
        wait::{WaitFlags, WaitStatus},
    },
    util::user_buffer::{read_from_user, write_to_user},
    vfs::FsError,
};

mod clone_ops;
mod exec_ops;
mod exit_ops;
mod futex_ops;
mod process_ops;
mod sched_ops;
mod session_ops;
mod time_ops;
mod wait_ops;

pub use clone_ops::*;
pub use exec_ops::*;
pub use exit_ops::*;
pub use futex_ops::*;
pub use process_ops::*;
pub use sched_ops::*;
pub use session_ops::*;
pub use time_ops::*;
pub use wait_ops::*;
