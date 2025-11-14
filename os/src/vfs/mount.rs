use crate::sync::SpinLock;
use crate::vfs::{Dentry, FileMode, FileSystem, FsError};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// 挂载标志
bitflags::bitflags! {
    pub struct MountFlags: u32 {
        /// 只读挂载
        const READ_ONLY  = 1 << 0;

        /// 禁止执行
        const NO_EXEC    = 1 << 1;

        /// 忽略 SUID/SGID 位
        const NO_SUID    = 1 << 2;

        /// 同步写入
        const SYNC       = 1 << 3;

        /// 禁止设备文件
        const NO_DEV     = 1 << 4;
    }
}

/// 挂载点信息
pub struct MountPoint {
    /// 挂载的文件系统
    pub fs: Arc<dyn FileSystem>,

    /// 挂载点的根 dentry
    pub root: Arc<Dentry>,

    /// 挂载标志
    pub flags: MountFlags,

    /// 设备路径（如果有）
    pub device: Option<String>,

    /// 挂载路径
    pub mount_path: String,
}

impl MountPoint {
    /// 创建新的挂载点
    pub fn new(
        fs: Arc<dyn FileSystem>,
        mount_path: String,
        flags: MountFlags,
        device: Option<String>,
    ) -> Arc<Self> {
        let root_inode = fs.root_inode();
        let root = Dentry::new(String::from("/"), root_inode);

        Arc::new(Self {
            fs,
            root,
            flags,
            device,
            mount_path,
        })
    }
}

/// 全局挂载表
pub struct MountTable {
    /// 挂载路径 -> 挂载点
    mounts: SpinLock<BTreeMap<String, Arc<MountPoint>>>,
}

impl MountTable {
    /// 创建新的挂载表
    pub const fn new() -> Self {
        Self {
            mounts: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 挂载文件系统
    pub fn mount(
        &self,
        fs: Arc<dyn FileSystem>,
        path: &str,
        flags: MountFlags,
        device: Option<String>,
    ) -> Result<(), FsError> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);

        // 检查是否已经挂载
        if self.mounts.lock().contains_key(&normalized_path) {
            return Err(FsError::AlreadyExists);
        }

        // 创建挂载点
        let mount_point = MountPoint::new(fs, normalized_path.clone(), flags, device);

        // 添加到挂载表
        self.mounts.lock().insert(normalized_path, mount_point);

        Ok(())
    }

    /// 卸载文件系统
    pub fn umount(&self, path: &str) -> Result<(), FsError> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);

        let mount_point = self
            .mounts
            .lock()
            .remove(&normalized_path)
            .ok_or(FsError::NotFound)?;

        // 同步文件系统
        mount_point.fs.sync()?;

        // 执行卸载清理
        mount_point.fs.umount()?;

        Ok(())
    }

    /// 查找给定路径的挂载点
    ///
    /// 返回最长匹配的挂载点
    pub fn find_mount(&self, path: &str) -> Option<Arc<MountPoint>> {
        use crate::vfs::normalize_path;

        let normalized_path = normalize_path(path);
        let mounts = self.mounts.lock();

        // 查找最长匹配的挂载点
        let mut best_match = None;
        let mut best_len = 0;

        for (mount_path, mount_point) in mounts.iter() {
            if normalized_path.starts_with(mount_path) && mount_path.len() > best_len {
                best_match = Some(mount_point.clone());
                best_len = mount_path.len();
            }
        }

        best_match
    }

    /// 获取根挂载点
    pub fn root_mount(&self) -> Option<Arc<MountPoint>> {
        self.mounts.lock().get("/").cloned()
    }

    /// 列出所有挂载点（用于调试）
    pub fn list_mounts(&self) -> Vec<(String, String)> {
        let mounts = self.mounts.lock();
        mounts
            .iter()
            .map(|(path, mp)| (path.clone(), String::from(mp.fs.fs_type())))
            .collect()
    }
}

// 全局挂载表
lazy_static::lazy_static! {
    pub static ref MOUNT_TABLE: MountTable = MountTable::new();
}

/// 获取根 dentry
pub fn get_root_dentry() -> Result<Arc<Dentry>, FsError> {
    MOUNT_TABLE
        .root_mount()
        .map(|mp| mp.root.clone())
        .ok_or(FsError::NotSupported)
}
