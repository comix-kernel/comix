use alloc::{format, vec::Vec};

use crate::{
    config::PAGE_SIZE,
    fs::proc::ContentGenerator,
    mm::frame_allocator::{get_free_frames, get_total_frames},
    vfs::FsError,
};

pub struct MeminfoGenerator;

impl ContentGenerator for MeminfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        // 从内存管理器获取真实数据
        let total_frames = get_total_frames();
        let free_frames = get_free_frames();

        // 转换为 kB（1024 字节）
        let total_kb = (total_frames * PAGE_SIZE) / 1024;
        let free_kb = (free_frames * PAGE_SIZE) / 1024;
        let available_kb = free_kb; // 简化实现：可用内存 = 空闲内存

        // 注意：格式严格遵循 Linux ABI
        let content = format!(
            "MemTotal:       {:>8} kB
MemFree:        {:>8} kB
MemAvailable:   {:>8} kB
Buffers:        {:>8} kB
Cached:         {:>8} kB
SwapCached:     {:>8} kB
Active:         {:>8} kB
Inactive:       {:>8} kB
Active(anon):   {:>8} kB
Inactive(anon): {:>8} kB
Active(file):   {:>8} kB
Inactive(file): {:>8} kB
Unevictable:    {:>8} kB
Mlocked:        {:>8} kB
SwapTotal:      {:>8} kB
SwapFree:       {:>8} kB
Dirty:          {:>8} kB
Writeback:      {:>8} kB
AnonPages:      {:>8} kB
Mapped:         {:>8} kB
Shmem:          {:>8} kB
",
            total_kb, free_kb, available_kb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        );

        Ok(content.into_bytes())
    }
}
