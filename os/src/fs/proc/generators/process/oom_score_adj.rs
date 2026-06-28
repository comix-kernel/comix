use alloc::{format, sync::Weak, vec::Vec};

use crate::{
    fs::proc::inode::{ContentGenerator, ContentWriter},
    kernel::TaskStruct,
    sync::SpinLock,
    vfs::FsError,
};

const OOM_SCORE_ADJ_MIN: i32 = -1000;
const OOM_SCORE_ADJ_MAX: i32 = 1000;

pub struct OomScoreAdjGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl OomScoreAdjGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentGenerator for OomScoreAdjGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let oom_score_adj = task_arc.lock().oom_score_adj;
        Ok(format!("{}\n", oom_score_adj).into_bytes())
    }
}

pub struct OomScoreAdjWriter {
    task: Weak<SpinLock<TaskStruct>>,
}

impl OomScoreAdjWriter {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

impl ContentWriter for OomScoreAdjWriter {
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let input = core::str::from_utf8(buf).map_err(|_| FsError::InvalidArgument)?;
        let value = input
            .trim()
            .parse::<i32>()
            .map_err(|_| FsError::InvalidArgument)?;
        if !(OOM_SCORE_ADJ_MIN..=OOM_SCORE_ADJ_MAX).contains(&value) {
            return Err(FsError::InvalidArgument);
        }

        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        task_arc.lock().oom_score_adj = value;
        Ok(buf.len())
    }
}
