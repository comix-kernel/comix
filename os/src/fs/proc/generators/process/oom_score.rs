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

fn calculate_oom_score(rss_bytes: usize, total_bytes: usize, oom_score_adj: i32) -> i32 {
    if oom_score_adj <= -1000 || total_bytes == 0 || rss_bytes == 0 {
        return 0;
    }

    let base = rss_bytes.saturating_mul(1000) / total_bytes;
    (base as i32 + oom_score_adj).clamp(0, 1000)
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
            let total_bytes = get_total_frames().saturating_mul(PAGE_SIZE);
            calculate_oom_score(stats.rss_bytes, total_bytes, oom_score_adj)
        } else {
            0
        };

        Ok(format!("{}\n", score).into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_oom_score;
    use crate::{config::PAGE_SIZE, kassert, test_case};

    test_case!(test_oom_score_uses_rss_bytes, {
        let total = 100 * PAGE_SIZE;
        let rss = 10 * PAGE_SIZE;

        kassert!(calculate_oom_score(rss, total, 0) == 100);
    });

    test_case!(test_oom_score_adj_is_applied_and_clamped, {
        let total = 100 * PAGE_SIZE;
        let rss = 10 * PAGE_SIZE;

        kassert!(calculate_oom_score(rss, total, 50) == 150);
        kassert!(calculate_oom_score(rss, total, 1000) == 1000);
        kassert!(calculate_oom_score(rss, total, -1000) == 0);
    });
}
