use alloc::{format, string::String, sync::Weak, vec::Vec};

use crate::{
    config::PAGE_SIZE,
    fs::proc::ContentGenerator,
    kernel::TaskStruct,
    mm::address::{PageNum, UsizeConvert},
    mm::memory_space::mapping_area::{AreaType, MapType},
    sync::SpinLock,
    vfs::FsError,
};

/// /proc/[pid]/maps (simplified): list user VMAs and their sizes.
pub struct MapsGenerator {
    task: Weak<SpinLock<TaskStruct>>,
}

impl MapsGenerator {
    pub fn new(task: Weak<SpinLock<TaskStruct>>) -> Self {
        Self { task }
    }
}

fn area_label(at: AreaType) -> &'static str {
    match at {
        AreaType::UserText => "[text]",
        AreaType::UserRodata => "[rodata]",
        AreaType::UserData => "[data]",
        AreaType::UserBss => "[bss]",
        AreaType::UserHeap => "[heap]",
        AreaType::UserStack => "[stack]",
        AreaType::UserMmap => "[mmap]",
        _ => "[kernel]",
    }
}

impl ContentGenerator for MapsGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task_arc = self.task.upgrade().ok_or(FsError::NotFound)?;
        let memory_space = {
            let t = task_arc.lock();
            t.memory_space.clone()
        };

        let Some(ms) = memory_space else {
            return Ok(Vec::new());
        };

        let ms = ms.lock();

        // Collect user areas and sort by start address.
        let mut areas: alloc::vec::Vec<_> = ms
            .areas()
            .iter()
            .filter(|a| {
                matches!(
                    a.area_type(),
                    AreaType::UserText
                        | AreaType::UserRodata
                        | AreaType::UserData
                        | AreaType::UserBss
                        | AreaType::UserStack
                        | AreaType::UserHeap
                        | AreaType::UserMmap
                )
            })
            .collect();

        areas.sort_by_key(|a| a.vpn_range().start().start_addr().as_usize());

        let mut out = String::new();
        for a in areas {
            let start = a.vpn_range().start().start_addr().as_usize();
            let end = a.vpn_range().end().start_addr().as_usize();

            let perm = a.permission();
            let r = if perm.contains(crate::mm::page_table::UniversalPTEFlag::READABLE) {
                'r'
            } else {
                '-'
            };
            let w = if perm.contains(crate::mm::page_table::UniversalPTEFlag::WRITEABLE) {
                'w'
            } else {
                '-'
            };
            let x = if perm.contains(crate::mm::page_table::UniversalPTEFlag::EXECUTABLE) {
                'x'
            } else {
                '-'
            };
            let p = 'p';
            let map_type = match a.map_type() {
                MapType::Direct => "direct",
                MapType::Framed => "framed",
                MapType::Reserved => "reserved",
            };
            let pages = a.vpn_range().len();
            let rss_pages = a.mapped_pages();

            out.push_str(&format!(
                "{:016x}-{:016x} {}{}{}{} {:>7}kB rss {:>7}kB {:>8} {}\n",
                start,
                end,
                r,
                w,
                x,
                p,
                (pages * PAGE_SIZE) / 1024,
                (rss_pages * PAGE_SIZE) / 1024,
                map_type,
                area_label(a.area_type()),
            ));
        }

        Ok(out.into_bytes())
    }
}
