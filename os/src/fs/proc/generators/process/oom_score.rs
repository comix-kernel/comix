use alloc::{format, sync::Weak, vec::Vec};

use crate::{
    config::PAGE_SIZE, fs::proc::ContentGenerator, kernel::TaskStruct,
    mm::frame_allocator::get_total_frames, sync::SpinLock, vfs::FsError,
};

use super::memory::collect_user_vm_stats;

pub struct OomScoreGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl OomScoreGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentGenerator for OomScoreGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let (oom_score_adj, memory_space) = {
            let task = task_arc.lock();
            (task.oom_score_adj, task.memory_space.clone())
        };

        let score = if oom_score_adj <= -1000 {
            0
        } else if let Some(memory_space) = memory_space {
            let stats = collect_user_vm_stats(&memory_space.lock());
            let accounted_bytes = stats.rss_bytes.max(stats.vm_size_bytes);
            let total_bytes = get_total_frames().saturating_mul(PAGE_SIZE);
            if total_bytes == 0 || accounted_bytes == 0 {
                0
            } else {
                let base = accounted_bytes.saturating_mul(1000) / total_bytes;
                (base as i32 + oom_score_adj).clamp(0, 1000)
            }
        } else {
            0
        };

        Ok(format!("{}\n", score).into_bytes())
    }
}
