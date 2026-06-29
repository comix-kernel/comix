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

fn parse_oom_score_adj(buf: &[u8]) -> Result<i32, FsError> {
    let input = core::str::from_utf8(buf).map_err(|_| FsError::InvalidArgument)?;
    let nul_terminated = input.split('\0').next().unwrap_or("");
    let value = nul_terminated
        .trim()
        .parse::<i32>()
        .map_err(|_| FsError::InvalidArgument)?;
    if !(OOM_SCORE_ADJ_MIN..=OOM_SCORE_ADJ_MAX).contains(&value) {
        return Err(FsError::InvalidArgument);
    }
    Ok(value)
}

impl ContentWriter for OomScoreAdjWriter {
    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let value = parse_oom_score_adj(buf)?;

        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        task_arc.lock().oom_score_adj = value;
        Ok(buf.len())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_oom_score_adj;
    use crate::{kassert, test_case, vfs::FsError};

    test_case!(test_parse_oom_score_adj_accepts_nul_terminated_input, {
        kassert!(parse_oom_score_adj(b"123\0").unwrap() == 123);
        kassert!(parse_oom_score_adj(b" -250\0ignored").unwrap() == -250);
    });

    test_case!(test_parse_oom_score_adj_keeps_existing_validation, {
        kassert!(parse_oom_score_adj(b"1001").is_err());
        kassert!(matches!(
            parse_oom_score_adj(b"abc"),
            Err(FsError::InvalidArgument)
        ));
    });
}
