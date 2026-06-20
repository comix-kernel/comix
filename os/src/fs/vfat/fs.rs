//! VFS filesystem wrapper for FAT/VFAT volumes.

use alloc::sync::Arc;

use crate::device::block::BlockDriver;
use crate::fs::vfat::adapter::{FatBlockDevice, VfatIoError};
use crate::fs::vfat::inode::VfatInode;
use crate::sync::Mutex;
use crate::vfs::{FileSystem, FsError, Inode, StatFs};

pub(super) type FatFs = fatfs::FileSystem<FatBlockDevice>;
pub(super) type FatDir<'a> =
    fatfs::Dir<'a, FatBlockDevice, fatfs::DefaultTimeProvider, fatfs::LossyOemCpConverter>;
pub(super) type FatFile<'a> =
    fatfs::File<'a, FatBlockDevice, fatfs::DefaultTimeProvider, fatfs::LossyOemCpConverter>;

/// Shared state for a mounted FAT/VFAT volume.
pub(super) struct VfatState {
    pub(super) device: Arc<dyn BlockDriver>,
    pub(super) device_id: usize,
    op_lock: Mutex<()>,
}

impl VfatState {
    fn new(device: Arc<dyn BlockDriver>, device_id: usize) -> Self {
        Self {
            device,
            device_id,
            op_lock: Mutex::new(()),
        }
    }

    fn open_storage(&self) -> Result<FatBlockDevice, FsError> {
        FatBlockDevice::new(self.device.clone()).map_err(map_io_error)
    }

    pub(super) fn with_fs<T>(
        &self,
        op: impl FnOnce(&FatFs) -> Result<T, FsError>,
    ) -> Result<T, FsError> {
        let _guard = self.op_lock.lock();
        let storage = self.open_storage()?;
        let fs = fatfs::FileSystem::new(storage, fatfs::FsOptions::new()).map_err(map_fat_error)?;

        let op_result = op(&fs);
        let unmount_result = fs.unmount().map_err(map_fat_error);

        match (op_result, unmount_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }
}

/// FAT/VFAT filesystem implementation backed by a kernel block device.
pub struct VfatFileSystem {
    state: Arc<VfatState>,
    root: Arc<dyn Inode>,
}

impl VfatFileSystem {
    /// Opens a FAT/VFAT filesystem on `device`.
    pub fn open(device: Arc<dyn BlockDriver>, device_id: usize) -> Result<Arc<Self>, FsError> {
        crate::pr_info!(
            "[VFAT] Opening FAT filesystem: block_size={}, total_blocks={}",
            device.block_size(),
            device.total_blocks()
        );

        let state = Arc::new(VfatState::new(device, device_id));
        state.with_fs(|_| Ok(()))?;

        let root = VfatInode::new_root(state.clone()) as Arc<dyn Inode>;
        let fs = Arc::new(Self { state, root });

        crate::pr_info!("[VFAT] Filesystem opened successfully");
        Ok(fs)
    }
}

impl FileSystem for VfatFileSystem {
    fn fs_type(&self) -> &'static str {
        "vfat"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root.clone()
    }

    fn sync(&self) -> Result<(), FsError> {
        self.state.with_fs(|_| Ok(()))?;
        if self.state.device.flush() {
            Ok(())
        } else {
            Err(FsError::IoError)
        }
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        self.state.with_fs(|fs| {
            let stats = fs.stats().map_err(map_fat_error)?;
            Ok(StatFs {
                block_size: stats.cluster_size() as usize,
                total_blocks: stats.total_clusters() as usize,
                free_blocks: stats.free_clusters() as usize,
                available_blocks: stats.free_clusters() as usize,
                total_inodes: 0,
                free_inodes: 0,
                fsid: self.state.device_id as u64,
                max_filename_len: 255,
            })
        })
    }

    fn umount(&self) -> Result<(), FsError> {
        self.sync()
    }
}

pub(super) fn map_io_error(error: VfatIoError) -> FsError {
    match error {
        VfatIoError::OutOfBounds => FsError::InvalidArgument,
        VfatIoError::DeviceError => FsError::IoError,
    }
}

pub(super) fn map_fat_error(error: fatfs::Error<VfatIoError>) -> FsError {
    match error {
        fatfs::Error::Io(error) => map_io_error(error),
        fatfs::Error::UnexpectedEof | fatfs::Error::WriteZero => FsError::IoError,
        fatfs::Error::InvalidInput | fatfs::Error::UnsupportedFileNameCharacter => {
            FsError::InvalidArgument
        }
        fatfs::Error::InvalidFileNameLength => FsError::NameTooLong,
        fatfs::Error::NotFound => FsError::NotFound,
        fatfs::Error::AlreadyExists => FsError::AlreadyExists,
        fatfs::Error::DirectoryIsNotEmpty => FsError::DirectoryNotEmpty,
        fatfs::Error::CorruptedFileSystem => FsError::InvalidArgument,
        fatfs::Error::NotEnoughSpace => FsError::NoSpace,
        _ => FsError::IoError,
    }
}
