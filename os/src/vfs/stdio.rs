use crate::arch::lib::console::{Stdin, Stdout};
use crate::vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType, TimeSpec};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// 标准输入 Inode
pub struct StdinInode;

impl Inode for StdinInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let time = TimeSpec::now();
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IRUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: time,
            mtime: time,
            ctime: time,
            nlinks: 1,
            blocks: 0,
        })
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        // 字符设备忽略 offset
        // 从 SBI 或串口读取字符
        // TODO: 重构为架构无关实现
        use crate::arch::lib::sbi::console_getchar;

        let mut count = 0;
        for byte in buf.iter_mut() {
            let ch = console_getchar();
            if ch == 0 {
                // 无输入可用
                break;
            }

            *byte = ch as u8;
            count += 1;

            // 如果读到换行符，停止读取（行缓冲）
            if ch == b'\n' as usize {
                break;
            }
        }

        Ok(count)
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, FsError> {
        // stdin 不可写
        Err(FsError::PermissionDenied)
    }

    fn lookup(&self, _name: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::NotDirectory)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(()) // 字符设备无需同步
    }
}

/// 标准输出
pub struct StdoutInode;

impl Inode for StdoutInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let time = TimeSpec::now();
        Ok(InodeMetadata {
            inode_no: 1,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: time,
            mtime: time,
            ctime: time,
            nlinks: 1,
            blocks: 0,
        })
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, FsError> {
        // stdout 不可读
        Err(FsError::PermissionDenied)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        use core::fmt::Write;
        
        let s = unsafe {core::str::from_utf8_unchecked(buf)};
        crate::arch::lib::console::Stdout.write_str(s).unwrap();
        Ok(buf.len())
    }

    fn lookup(&self, _name: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::NotDirectory)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }
}

pub struct StderrInode;

impl Inode for StderrInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let time = TimeSpec::now();
        Ok(InodeMetadata {
            inode_no: 2,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: time,
            mtime: time,
            ctime: time,
            nlinks: 1,
            blocks: 0,
        })
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, FsError> {
        // stderr 不可读
        Err(FsError::PermissionDenied)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        use core::fmt::Write;
        
        let s = unsafe {core::str::from_utf8_unchecked(buf)};
        crate::arch::lib::console::Stdout.write_str(s).unwrap();
        Ok(buf.len())
    }

    fn lookup(&self, _name: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::NotDirectory)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        Err(FsError::NotDirectory)
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }
}

/// 创建标准 I/O 文件对象（stdin、stdout、stderr）
///
/// 返回：三元组 (stdin, stdout, stderr)，分别对应文件描述符 0、1、2
pub fn create_stdio_files() -> (Arc<crate::vfs::File>, Arc<crate::vfs::File>, Arc<crate::vfs::File>) {
    use crate::vfs::{File, OpenFlags, Dentry};

    // 创建 inode
    let stdin_inode = Arc::new(StdinInode) as Arc<dyn Inode>;
    let stdout_inode = Arc::new(StdoutInode) as Arc<dyn Inode>;
    let stderr_inode = Arc::new(StderrInode) as Arc<dyn Inode>;

    // 创建 dentry
    let stdin_dentry = Dentry::new("stdin".into(), stdin_inode);
    let stdout_dentry = Dentry::new("stdout".into(), stdout_inode);
    let stderr_dentry = Dentry::new("stderr".into(), stderr_inode);

    // 创建 file 对象
    let stdin_file = Arc::new(File::new(stdin_dentry, OpenFlags::O_RDONLY));
    let stdout_file = Arc::new(File::new(stdout_dentry, OpenFlags::O_WRONLY));
    let stderr_file = Arc::new(File::new(stderr_dentry, OpenFlags::O_WRONLY));

    (stdin_file, stdout_file, stderr_file)
}