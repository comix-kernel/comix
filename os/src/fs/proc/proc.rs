use crate::{
    fs::proc::ProcInode,
    vfs::{FileMode, FileSystem, FsError, Inode, StatFs},
};
use alloc::sync::Arc;

pub struct ProcFS {
    root_inode: Arc<ProcInode>,
}

impl ProcFS {
    /// 创建新的 ProcFS 实例
    pub fn new() -> Arc<Self> {
        // 创建根目录
        let root = ProcInode::new_proc_root_directory(FileMode::from_bits_truncate(
            0o555 | FileMode::S_IFDIR.bits(),
        ));

        Arc::new(Self { root_inode: root })
    }

    /// 初始化 proc 文件系统树结构
    pub fn init_tree(self: &Arc<Self>) -> Result<(), FsError> {
        use crate::fs::proc::generators::{
            CpuinfoGenerator, MeminfoGenerator, MountsGenerator, UptimeGenerator,
        };
        use crate::kernel::current_task;

        let root = &self.root_inode;

        // 创建 /proc/meminfo
        let meminfo = ProcInode::new_dynamic_file(
            "meminfo",
            alloc::sync::Arc::new(MeminfoGenerator),
            FileMode::from_bits_truncate(0o444), // r--r--r--
        );
        root.add_child("meminfo", meminfo)?;

        // 创建 /proc/uptime
        let uptime = ProcInode::new_dynamic_file(
            "uptime",
            alloc::sync::Arc::new(UptimeGenerator),
            FileMode::from_bits_truncate(0o444), // r--r--r--
        );
        root.add_child("uptime", uptime)?;

        // 创建 /proc/cpuinfo
        let cpuinfo = ProcInode::new_dynamic_file(
            "cpuinfo",
            alloc::sync::Arc::new(CpuinfoGenerator),
            FileMode::from_bits_truncate(0o444), // r--r--r--
        );
        root.add_child("cpuinfo", cpuinfo)?;

        // 创建 /proc/mounts
        let mounts = ProcInode::new_dynamic_file(
            "mounts",
            alloc::sync::Arc::new(MountsGenerator),
            FileMode::from_bits_truncate(0o444), // r--r--r--
        );
        root.add_child("mounts", mounts)?;

        // 创建 /proc/psmem - 进程内存快照（便于定位 FrameAllocFailed 的真实来源）
        let psmem = ProcInode::new_dynamic_file(
            "psmem",
            alloc::sync::Arc::new(crate::fs::proc::generators::PsmemGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("psmem", psmem)?;

        // 创建 /proc/self - 动态符号链接，指向当前进程
        let self_link = ProcInode::new_dynamic_symlink("self", || {
            use alloc::string::ToString;
            // 获取当前任务的 PID 并转换为字符串
            let task = current_task();
            task.lock().pid.to_string()
        });
        root.add_child("self", self_link)?;

        Ok(())
    }
}

impl FileSystem for ProcFS {
    fn fs_type(&self) -> &'static str {
        "proc"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root_inode.clone()
    }

    fn sync(&self) -> Result<(), FsError> {
        // proc 是纯内存文件系统，无需同步
        Ok(())
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        Ok(StatFs {
            block_size: 4096,
            total_blocks: 0,
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
}
