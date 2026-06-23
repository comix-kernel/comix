//! VFS inode wrapper for FAT/VFAT paths.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use crate::fs::vfat::fs::{FatDir, FatFile, FatFs, VfatState, map_fat_error};
use crate::uapi::time::TimeSpec;
use crate::vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

/// FAT/VFAT inode represented by a path relative to the mounted volume root.
pub struct VfatInode {
    state: Arc<VfatState>,
    path: String,
    inode_type: InodeType,
}

impl VfatInode {
    /// Creates the root inode for a mounted FAT/VFAT volume.
    pub fn new_root(state: Arc<VfatState>) -> Arc<Self> {
        Arc::new(Self {
            state,
            path: String::new(),
            inode_type: InodeType::Directory,
        })
    }

    fn new_child(&self, name: &str, inode_type: InodeType) -> Arc<Self> {
        Arc::new(Self {
            state: self.state.clone(),
            path: child_path(&self.path, name),
            inode_type,
        })
    }

    fn new_with_path(&self, path: String, inode_type: InodeType) -> Arc<Self> {
        Arc::new(Self {
            state: self.state.clone(),
            path,
            inode_type,
        })
    }

    fn root_dir<'a>(&self, fs: &'a FatFs) -> FatDir<'a> {
        fs.root_dir()
    }

    fn open_dir<'a>(&self, fs: &'a FatFs) -> Result<FatDir<'a>, FsError> {
        let root = self.root_dir(fs);
        if self.path.is_empty() {
            Ok(root)
        } else {
            root.open_dir(&self.path).map_err(map_fat_error)
        }
    }

    fn metadata_from_fs(&self, fs: &FatFs) -> Result<InodeMetadata, FsError> {
        if self.path.is_empty() {
            return Ok(make_metadata(&self.path, InodeType::Directory, 0));
        }

        let root = self.root_dir(fs);
        if let Ok(file) = root.open_file(&self.path) {
            let size = file.size().unwrap_or(0) as usize;
            return Ok(make_metadata(&self.path, InodeType::File, size));
        }

        root.open_dir(&self.path).map_err(map_fat_error)?;
        Ok(make_metadata(&self.path, InodeType::Directory, 0))
    }

    fn ensure_directory(&self) -> Result<(), FsError> {
        if self.inode_type == InodeType::Directory {
            Ok(())
        } else {
            Err(FsError::NotDirectory)
        }
    }
}

impl Inode for VfatInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.state.with_fs(|fs| self.metadata_from_fs(fs))
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        self.state.with_fs(|fs| {
            let root = self.root_dir(fs);
            let mut file = root.open_file(&self.path).map_err(map_fat_error)?;
            fatfs::Seek::seek(&mut file, fatfs::SeekFrom::Start(offset as u64))
                .map_err(map_fat_error)?;

            let mut total = 0;
            while total < buf.len() {
                let read =
                    fatfs::Read::read(&mut file, &mut buf[total..]).map_err(map_fat_error)?;
                if read == 0 {
                    break;
                }
                total += read;
            }
            Ok(total)
        })
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }
        if offset > u32::MAX as usize {
            return Err(FsError::InvalidArgument);
        }

        self.state.with_fs(|fs| {
            let root = self.root_dir(fs);
            let mut file = root.open_file(&self.path).map_err(map_fat_error)?;
            let current_size = file.size().unwrap_or(0) as usize;

            if offset > current_size {
                fatfs::Seek::seek(&mut file, fatfs::SeekFrom::Start(current_size as u64))
                    .map_err(map_fat_error)?;
                write_zeros(&mut file, offset - current_size)?;
            } else {
                fatfs::Seek::seek(&mut file, fatfs::SeekFrom::Start(offset as u64))
                    .map_err(map_fat_error)?;
            }

            fatfs::Write::write_all(&mut file, buf).map_err(map_fat_error)?;
            Ok(buf.len())
        })
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        if name == "." {
            return Ok(self.new_with_path(self.path.clone(), self.inode_type) as Arc<dyn Inode>);
        }
        if name == ".." {
            let parent = parent_path(&self.path);
            return Ok(self.new_with_path(parent, InodeType::Directory) as Arc<dyn Inode>);
        }
        validate_child_name(name)?;
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let parent = self.open_dir(fs)?;
            if parent.open_dir(name).is_ok() {
                return Ok(self.new_child(name, InodeType::Directory) as Arc<dyn Inode>);
            }
            if parent.open_file(name).is_ok() {
                return Ok(self.new_child(name, InodeType::File) as Arc<dyn Inode>);
            }
            Err(FsError::NotFound)
        })
    }

    fn create(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        validate_child_name(name)?;
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let parent = self.open_dir(fs)?;
            if parent.open_file(name).is_ok() || parent.open_dir(name).is_ok() {
                return Err(FsError::AlreadyExists);
            }
            parent.create_file(name).map_err(map_fat_error)?;
            Ok(self.new_child(name, InodeType::File) as Arc<dyn Inode>)
        })
    }

    fn mkdir(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        validate_child_name(name)?;
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let parent = self.open_dir(fs)?;
            if parent.open_file(name).is_ok() || parent.open_dir(name).is_ok() {
                return Err(FsError::AlreadyExists);
            }
            parent.create_dir(name).map_err(map_fat_error)?;
            Ok(self.new_child(name, InodeType::Directory) as Arc<dyn Inode>)
        })
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        validate_child_name(name)?;
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let parent = self.open_dir(fs)?;
            parent.remove(name).map_err(map_fat_error)
        })
    }

    fn rmdir(&self, name: &str) -> Result<(), FsError> {
        validate_child_name(name)?;
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let parent = self.open_dir(fs)?;
            parent.open_dir(name).map_err(map_fat_error)?;
            parent.remove(name).map_err(map_fat_error)
        })
    }

    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError> {
        validate_child_name(old_name)?;
        validate_child_name(new_name)?;
        self.ensure_directory()?;

        let new_parent = new_parent
            .as_any()
            .downcast_ref::<VfatInode>()
            .ok_or(FsError::CrossDeviceLink)?;
        new_parent.ensure_directory()?;

        if !Arc::ptr_eq(&self.state, &new_parent.state) {
            return Err(FsError::CrossDeviceLink);
        }

        if self.path == new_parent.path && old_name.eq_ignore_ascii_case(new_name) {
            return Ok(());
        }

        self.state.with_fs(|fs| {
            let old_dir = self.open_dir(fs)?;
            let new_dir = new_parent.open_dir(fs)?;
            let old_is_dir = old_dir.open_dir(old_name).is_ok();

            if new_dir.open_file(new_name).is_ok() || new_dir.open_dir(new_name).is_ok() {
                let new_is_dir = new_dir.open_dir(new_name).is_ok();
                if old_is_dir && !new_is_dir {
                    return Err(FsError::NotDirectory);
                }
                if !old_is_dir && new_is_dir {
                    return Err(FsError::IsDirectory);
                }
                new_dir.remove(new_name).map_err(map_fat_error)?;
            }

            old_dir
                .rename(old_name, &new_dir, new_name)
                .map_err(map_fat_error)
        })
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        self.ensure_directory()?;

        self.state.with_fs(|fs| {
            let dir = self.open_dir(fs)?;
            let mut entries = Vec::new();
            entries.push(DirEntry {
                name: ".".to_string(),
                inode_no: inode_no_for_path(&self.path),
                inode_type: InodeType::Directory,
            });
            let parent = parent_path(&self.path);
            entries.push(DirEntry {
                name: "..".to_string(),
                inode_no: inode_no_for_path(&parent),
                inode_type: InodeType::Directory,
            });

            for entry in dir.iter() {
                let entry = entry.map_err(map_fat_error)?;
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let inode_type = if entry.is_dir() {
                    InodeType::Directory
                } else {
                    InodeType::File
                };
                entries.push(DirEntry {
                    inode_no: inode_no_for_path(&child_path(&self.path, &name)),
                    name,
                    inode_type,
                });
            }

            Ok(entries)
        })
    }

    fn truncate(&self, size: usize) -> Result<(), FsError> {
        if self.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }
        if size > u32::MAX as usize {
            return Err(FsError::InvalidArgument);
        }

        self.state.with_fs(|fs| {
            let root = self.root_dir(fs);
            let mut file = root.open_file(&self.path).map_err(map_fat_error)?;
            let current_size = file.size().unwrap_or(0) as usize;
            if size > current_size {
                fatfs::Seek::seek(&mut file, fatfs::SeekFrom::Start(current_size as u64))
                    .map_err(map_fat_error)?;
                write_zeros(&mut file, size - current_size)?;
            } else {
                fatfs::Seek::seek(&mut file, fatfs::SeekFrom::Start(size as u64))
                    .map_err(map_fat_error)?;
                file.truncate().map_err(map_fat_error)?;
            }
            Ok(())
        })
    }

    fn sync(&self) -> Result<(), FsError> {
        if self.state.device.flush() {
            Ok(())
        } else {
            Err(FsError::IoError)
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn set_times(&self, _atime: Option<TimeSpec>, _mtime: Option<TimeSpec>) -> Result<(), FsError> {
        Ok(())
    }

    fn readlink(&self) -> Result<String, FsError> {
        Err(FsError::NotSupported)
    }

    fn mknod(&self, _name: &str, _mode: FileMode, _dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Ok(())
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Ok(())
    }
}

fn validate_child_name(name: &str) -> Result<(), FsError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(FsError::InvalidArgument);
    }
    Ok(())
}

fn child_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent, name)
    }
}

fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[..index].to_string(),
        None => String::new(),
    }
}

fn make_metadata(path: &str, inode_type: InodeType, size: usize) -> InodeMetadata {
    let type_mode = match inode_type {
        InodeType::Directory => FileMode::S_IFDIR,
        InodeType::File => FileMode::S_IFREG,
        InodeType::Symlink => FileMode::S_IFLNK,
        InodeType::CharDevice => FileMode::S_IFCHR,
        InodeType::BlockDevice => FileMode::S_IFBLK,
        InodeType::Fifo => FileMode::S_IFIFO,
        InodeType::Socket => FileMode::S_IFSOCK,
    };
    let mode = type_mode | FileMode::from_bits_truncate(0o777);
    let time = TimeSpec::zero();

    InodeMetadata {
        inode_no: inode_no_for_path(path),
        inode_type,
        mode,
        uid: 0,
        gid: 0,
        size,
        atime: time,
        mtime: time,
        ctime: time,
        nlinks: if inode_type == InodeType::Directory {
            2
        } else {
            1
        },
        blocks: size.div_ceil(512),
        rdev: 0,
    }
}

fn inode_no_for_path(path: &str) -> usize {
    if path.is_empty() {
        return 1;
    }

    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash as usize
}

fn write_zeros(file: &mut FatFile<'_>, mut len: usize) -> Result<(), FsError> {
    let zeros = vec![0u8; 512];
    while len > 0 {
        let chunk = len.min(zeros.len());
        fatfs::Write::write_all(file, &zeros[..chunk]).map_err(map_fat_error)?;
        len -= chunk;
    }
    Ok(())
}
