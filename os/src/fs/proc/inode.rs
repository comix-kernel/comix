use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    sync::{Mutex, SpinLock},
    uapi::time::TimeSpec,
    vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType},
};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

/// 动态内容生成器 trait
pub trait ContentGenerator: Send + Sync {
    /// 生成文件内容（每次调用时重新生成）
    fn generate(&self) -> Result<Vec<u8>, FsError>;
}

pub struct ProcInode {
    /// 元数据
    metadata: SpinLock<InodeMetadata>,

    /// 内容
    content: ProcInodeContent,
}

pub enum ProcInodeContent {
    /// 静态文件（内容固定）
    Static(Vec<u8>),

    /// 动态文件（每次读取时生成）
    Dynamic(Arc<dyn ContentGenerator>),

    /// 目录（包含子节点）
    Directory(Mutex<BTreeMap<String, Arc<ProcInode>>>),

    /// 符号链接
    Symlink(String),

    /// 动态符号链接（每次读取时生成目标）
    DynamicSymlink(Arc<dyn Fn() -> String + Send + Sync>),
}

/// 全局 Inode 编号分配器
static NEXT_INODE_NO: AtomicUsize = AtomicUsize::new(1);

impl ProcInode {
    /// 创建静态文件 inode
    pub fn new_static_file(_name: &str, content: Vec<u8>, mode: FileMode) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();

        Arc::new(Self {
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::File,
                mode,
                uid: 0,
                gid: 0,
                size: 0, // proc 文件总是返回 size = 0
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Static(content),
        })
    }

    /// 创建动态文件 inode
    pub fn new_dynamic_file(
        _name: &str,
        generator: Arc<dyn ContentGenerator>,
        mode: FileMode,
    ) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();

        Arc::new(Self {
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::File,
                mode,
                uid: 0,
                gid: 0,
                size: 0, // proc 文件总是返回 size = 0
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Dynamic(generator),
        })
    }

    /// 创建目录 inode
    pub fn new_directory(mode: FileMode) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();

        Arc::new(Self {
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Directory,
                mode,
                uid: 0,
                gid: 0,
                size: 0,
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 2, // . 和 ..
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Directory(Mutex::new(BTreeMap::new())),
        })
    }

    /// 创建符号链接 inode
    pub fn new_symlink(_name: &str, target: String) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();

        Arc::new(Self {
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Symlink,
                mode: FileMode::from_bits_truncate(0o777),
                uid: 0,
                gid: 0,
                size: target.len(),
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Symlink(target),
        })
    }

    /// 创建动态符号链接 inode
    pub fn new_dynamic_symlink<F>(_name: &str, generator: F) -> Arc<Self>
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = TimeSpec::now();

        Arc::new(Self {
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Symlink,
                mode: FileMode::from_bits_truncate(0o777),
                uid: 0,
                gid: 0,
                size: 0, // 动态链接的大小未知
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::DynamicSymlink(Arc::new(generator)),
        })
    }

    /// 向目录添加子节点
    pub fn add_child(&self, name: &str, child: Arc<ProcInode>) -> Result<(), FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                children.lock().insert(name.to_string(), child);
                Ok(())
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    /// 为指定 PID 创建进程目录
    fn create_process_dir(&self, pid: u32) -> Option<Arc<ProcInode>> {
        use crate::fs::proc::generators::{CmdlineGenerator, StatGenerator, StatusGenerator};
        use crate::kernel::{TASK_MANAGER, TaskManagerTrait};

        // 获取任务
        let task = TASK_MANAGER.lock().get_task(pid)?;

        // 创建进程目录
        let proc_dir = Self::new_directory(FileMode::from_bits_truncate(
            0o555 | FileMode::S_IFDIR.bits(),
        ));

        // 创建 status 文件
        let status = Self::new_dynamic_file(
            "status",
            Arc::new(StatusGenerator::new(Arc::downgrade(&task))),
            FileMode::from_bits_truncate(0o444),
        );
        let _ = proc_dir.add_child("status", status);

        // 创建 stat 文件
        let stat = Self::new_dynamic_file(
            "stat",
            Arc::new(StatGenerator::new(Arc::downgrade(&task))),
            FileMode::from_bits_truncate(0o444),
        );
        let _ = proc_dir.add_child("stat", stat);

        // 创建 cmdline 文件
        let cmdline = Self::new_dynamic_file(
            "cmdline",
            Arc::new(CmdlineGenerator::new(Arc::downgrade(&task))),
            FileMode::from_bits_truncate(0o444),
        );
        let _ = proc_dir.add_child("cmdline", cmdline);

        // 创建 exe 符号链接：尽量满足 readlinkat("/proc/self/exe") 的基本需求
        // 目前 Comix 未持久化记录“可执行文件真实路径”，先返回一个稳定的绝对路径占位。
        let exe = Self::new_dynamic_symlink("exe", || "/".to_string());
        let _ = proc_dir.add_child("exe", exe);

        Some(proc_dir)
    }
}

impl Inode for ProcInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(self.metadata.lock().clone())
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        match &self.content {
            ProcInodeContent::Static(data) => {
                if offset >= data.len() {
                    return Ok(0);
                }
                let to_read = (data.len() - offset).min(buf.len());
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            ProcInodeContent::Dynamic(generator) => {
                let data = generator.generate()?;
                if offset >= data.len() {
                    return Ok(0);
                }
                let to_read = (data.len() - offset).min(buf.len());
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            _ => Err(FsError::IsDirectory),
        }
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                // 先从已有的子节点中查找
                if let Some(child) = children.lock().get(name).cloned() {
                    return Ok(child as Arc<dyn Inode>);
                }

                // 检查是否为进程目录（数字命名）
                if let Ok(pid) = name.parse::<u32>() {
                    // 动态创建进程目录
                    if let Some(proc_dir) = self.create_process_dir(pid) {
                        // 缓存到children中
                        children.lock().insert(name.to_string(), proc_dir.clone());
                        return Ok(proc_dir as Arc<dyn Inode>);
                    }
                }

                Err(FsError::NotFound)
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rmdir(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                let metadata = self.metadata.lock();
                let mut entries = Vec::new();

                entries.push(DirEntry {
                    name: ".".to_string(),
                    inode_no: metadata.inode_no,
                    inode_type: InodeType::Directory,
                });
                entries.push(DirEntry {
                    name: "..".to_string(),
                    inode_no: metadata.inode_no,
                    inode_type: InodeType::Directory,
                });

                for (name, child) in children.lock().iter() {
                    let child_meta = child.metadata.lock();
                    entries.push(DirEntry {
                        name: name.clone(),
                        inode_no: child_meta.inode_no,
                        inode_type: child_meta.inode_type,
                    });
                }

                Ok(entries)
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn set_times(&self, _atime: Option<TimeSpec>, _mtime: Option<TimeSpec>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn readlink(&self) -> Result<String, FsError> {
        match &self.content {
            ProcInodeContent::Symlink(target) => Ok(target.clone()),
            ProcInodeContent::DynamicSymlink(generator) => Ok(generator()),
            _ => Err(FsError::InvalidArgument),
        }
    }

    fn mknod(&self, _name: &str, _mode: FileMode, _dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
