use alloc::{format, string::String, vec::Vec};

use crate::{
    fs::proc::ContentGenerator,
    fs::proc::generators::process::collect_user_vm_stats,
    kernel::{TASK_MANAGER, TaskManagerTrait},
    vfs::FsError,
};

/// /proc/psmem - per-process memory snapshot (debug-friendly, Linux-ish)
pub struct PsmemGenerator;

impl ContentGenerator for PsmemGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let pids = TASK_MANAGER.lock().list_process_pids_snapshot();

        let mut out = String::new();
        out.push_str("PID\tVmSize(kB)\tVmRSS(kB)\tStack(kB)\tHeap+Data(kB)\tMmap(kB)\tExe(kB)\tName\n");

        for pid in pids {
            let Some(task) = TASK_MANAGER.lock().get_task(pid) else { continue };
            let (name, stats_opt) = {
                let t = task.lock();
                let name = t
                    .exe_path
                    .clone()
                    .unwrap_or_else(|| format!("task_{}", t.tid));
                let stats_opt = t.memory_space.as_ref().map(|ms| {
                    let ms = ms.lock();
                    collect_user_vm_stats(&ms)
                });
                (name, stats_opt)
            };

            let (vm_size_kb, rss_kb, stack_kb, data_kb, mmap_kb, exe_kb) = stats_opt
                .map(|s| {
                    let data_bytes = s.data_bytes
                        .saturating_add(s.bss_bytes)
                        .saturating_add(s.heap_bytes);
                    (
                        s.vm_size_kb(),
                        s.rss_kb(),
                        s.stack_bytes / 1024,
                        data_bytes / 1024,
                        s.mmap_bytes / 1024,
                        s.text_bytes / 1024,
                    )
                })
                .unwrap_or((0, 0, 0, 0, 0, 0));

            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                pid, vm_size_kb, rss_kb, stack_kb, data_kb, mmap_kb, exe_kb, name
            ));
        }

        Ok(out.into_bytes())
    }
}

