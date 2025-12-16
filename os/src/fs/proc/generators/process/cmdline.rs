use alloc::{sync::Weak, vec::Vec};

use crate::{fs::proc::ContentGenerator, kernel::TaskStruct, sync::SpinLock, vfs::FsError};

/// 为指定任务生成 /proc/\[pid\]/cmdline 内容的生成器
pub struct CmdlineGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl CmdlineGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentGenerator for CmdlineGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let task = task_arc.lock();

        // TODO: 从任务中获取真实的命令行参数
        // 目前简化实现：返回空内容或者进程名
        // Linux cmdline 格式: 参数之间用 \0 分隔，最后也以 \0 结尾

        // 简化实现：返回 "task_<tid>\0"
        let mut content = alloc::format!("task_{}", task.tid).into_bytes();
        content.push(0); // null terminator

        Ok(content)
    }
}
