use alloc::{format, sync::Weak, vec::Vec};

use crate::{
    fs::proc::ContentGenerator,
    kernel::{TaskState, TaskStruct},
    sync::SpinLock,
    vfs::FsError,
};

use super::memory::collect_user_vm_stats;

/// 为指定任务生成 /proc/\[pid\]/status 内容的生成器
pub struct StatusGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl StatusGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentGenerator for StatusGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let (pid, tid, ppid, state, name, mem_stats) = {
            let task = task_arc.lock();
            let name = format!("task_{}", task.tid);
            let mem_stats = task.memory_space.as_ref().map(|ms| {
                let ms = ms.lock();
                collect_user_vm_stats(&ms)
            });
            (
                task.pid,
                task.tid,
                task.ppid,
                task.state,
                name,
                mem_stats,
            )
        };

        // 状态字符串映射
        let state_char = match state {
            TaskState::Running => 'R',
            TaskState::Interruptible => 'S',
            TaskState::Uninterruptible => 'D',
            TaskState::Stopped => 'T',
            TaskState::Zombie => 'Z',
        };

        let (vm_size_kb, rss_kb, stack_kb, data_kb, exe_kb, mmap_kb) = mem_stats
            .map(|s| {
                let data_bytes = s.data_bytes
                    .saturating_add(s.bss_bytes)
                    .saturating_add(s.heap_bytes);
                (
                    s.vm_size_kb(),
                    s.rss_kb(),
                    s.stack_bytes / 1024,
                    data_bytes / 1024,
                    s.text_bytes / 1024,
                    s.mmap_bytes / 1024,
                )
            })
            .unwrap_or((0, 0, 0, 0, 0, 0));

        // 构建 status 内容（遵循 Linux ABI 格式）
        let content = format!(
            "Name:\t{}\n\
             State:\t{} ({})\n\
             Tgid:\t{}\n\
             Pid:\t{}\n\
             PPid:\t{}\n\
             TracerPid:\t0\n\
             Uid:\t0\t0\t0\t0\n\
             Gid:\t0\t0\t0\t0\n\
             VmSize:\t{:>8} kB\n\
             VmRSS:\t{:>8} kB\n\
             VmStk:\t{:>8} kB\n\
             VmData:\t{:>8} kB\n\
             VmExe:\t{:>8} kB\n\
             VmLib:\t{:>8} kB\n\
             VmPTE:\t{:>8} kB\n\
             VmSwap:\t{:>8} kB\n\
             VmMmap:\t{:>8} kB\n",
            name,
            state_char,
            state_name(state_char),
            pid,
            tid,
            ppid,
            vm_size_kb,
            rss_kb,
            stack_kb,
            data_kb,
            exe_kb,
            0usize, // VmLib (TODO: dynamic linker / shared libs)
            0usize, // VmPTE (TODO: account page tables)
            0usize, // VmSwap
            mmap_kb,
        );

        Ok(content.into_bytes())
    }
}

fn state_name(state: char) -> &'static str {
    match state {
        'R' => "running",
        'S' => "sleeping",
        'D' => "disk sleep",
        'T' => "stopped",
        'Z' => "zombie",
        _ => "unknown",
    }
}
