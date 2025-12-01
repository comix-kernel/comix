use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::{FsError, MOUNT_TABLE};

pub struct MountsGenerator;

impl ContentGenerator for MountsGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let mut content = String::new();

        // 获取所有挂载点
        let mounts = MOUNT_TABLE.list_all();

        for (path, mount_point) in mounts {
            let device = mount_point.device.as_deref().unwrap_or("none");
            let fs_type = mount_point.fs.fs_type();

            // 构建挂载选项
            let mut options = Vec::new();
            if mount_point
                .flags
                .contains(crate::vfs::MountFlags::READ_ONLY)
            {
                options.push("ro");
            } else {
                options.push("rw");
            }
            options.push("relatime");

            let line = format!(
                "{} {} {} {} 0 0\n",
                device,
                path,
                fs_type,
                options.join(",")
            );

            content.push_str(&line);
        }

        Ok(content.into_bytes())
    }
}
