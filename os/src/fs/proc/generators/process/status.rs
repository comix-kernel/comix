use alloc::{format, sync::Weak, vec::Vec};

use crate::{
    fs::proc::ContentGenerator,
    kernel::{TaskState, TaskStruct},
    sync::SpinLock,
    vfs::FsError,
};

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
        let task = task_arc.lock();

        // 状态字符串映射
        let state_char = match task.state {
            TaskState::Running => 'R',
            TaskState::Interruptible => 'S',
            TaskState::Uninterruptible => 'D',
            TaskState::Stopped => 'T',
            TaskState::Zombie => 'Z',
        };

        // 获取进程名称（简化实现，暂时使用 "task_<tid>"）
        let name = format!("task_{}", task.tid);

        // 构建 status 内容（遵循 Linux ABI 格式）
        let content = format!(
            "Name:\t{}\n\
             State:\t{} ({})\n\
             Tgid:\t{}\n\
             Pid:\t{}\n\
             PPid:\t{}\n\
             TracerPid:\t0\n\
             Uid:\t0\t0\t0\t0\n\
             Gid:\t0\t0\t0\t0\n",
            name,
            state_char,
            state_name(state_char),
            task.pid,
            task.tid,
            task.ppid,
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
