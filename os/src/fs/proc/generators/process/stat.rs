use alloc::{format, sync::Weak, vec::Vec};

use crate::{
    fs::proc::ContentGenerator,
    kernel::{TaskState, TaskStruct},
    sync::SpinLock,
    vfs::FsError,
};

/// 为指定任务生成 /proc/\[pid\]/stat 内容的生成器
pub struct StatGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl StatGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentGenerator for StatGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let task = task_arc.lock();

        // 状态字符
        let state_char = match task.state {
            TaskState::Running => 'R',
            TaskState::Interruptible => 'S',
            TaskState::Uninterruptible => 'D',
            TaskState::Stopped => 'T',
            TaskState::Zombie => 'Z',
        };

        // 获取进程名称（简化实现）
        let name = format!("task_{}", task.tid);

        // Linux /proc/\[pid\]/stat 格式（简化版）
        // 格式参考: man 5 proc
        let content = format!(
            "{} ({}) {} {} {} {} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
            task.tid,   // (1) pid
            name,       // (2) comm (进程名)
            state_char, // (3) state
            task.ppid,  // (4) ppid
            task.pgid,  // (5) pgrp
            0,          // (6) session
                        // 后续字段暂时用 0 填充
        );

        Ok(content.into_bytes())
    }
}
