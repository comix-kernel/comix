//! Adapter from [`BlockDriver`] to the byte-oriented IO traits required by `fatfs`.

use alloc::sync::Arc;
use alloc::vec;

use crate::device::block::BlockDriver;

/// Error returned by the VFAT block-device adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfatIoError {
    /// A requested offset or length is outside the block device.
    OutOfBounds,
    /// The underlying block driver rejected an operation.
    DeviceError,
}

impl fatfs::IoError for VfatIoError {
    fn is_interrupted(&self) -> bool {
        false
    }

    fn new_unexpected_eof_error() -> Self {
        Self::OutOfBounds
    }

    fn new_write_zero_error() -> Self {
        Self::DeviceError
    }
}

/// Byte-stream facade over a kernel block driver.
///
/// `fatfs` reads and writes arbitrary byte ranges, while [`BlockDriver`] only
/// accepts whole blocks. This adapter translates unaligned IO into block reads
/// and writes, preserving untouched bytes in partially written blocks.
pub struct FatBlockDevice {
    inner: Arc<dyn BlockDriver>,
    position: u64,
    block_size: usize,
    total_bytes: u64,
}

impl FatBlockDevice {
    /// Creates a new adapter for `device`.
    pub fn new(device: Arc<dyn BlockDriver>) -> Result<Self, VfatIoError> {
        let block_size = device.block_size();
        let total_blocks = device.total_blocks();
        if block_size == 0 {
            return Err(VfatIoError::DeviceError);
        }

        let total_bytes = (block_size as u64)
            .checked_mul(total_blocks as u64)
            .ok_or(VfatIoError::OutOfBounds)?;

        Ok(Self {
            inner: device,
            position: 0,
            block_size,
            total_bytes,
        })
    }

    /// Returns the underlying device block size.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Returns the total byte length exposed by the block device.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Returns the current byte position.
    pub fn position(&self) -> u64 {
        self.position
    }

    fn read_block(&self, block_id: usize, block: &mut [u8]) -> Result<(), VfatIoError> {
        if self.inner.read_block(block_id, block) {
            Ok(())
        } else {
            Err(VfatIoError::DeviceError)
        }
    }

    fn write_block(&self, block_id: usize, block: &[u8]) -> Result<(), VfatIoError> {
        if self.inner.write_block(block_id, block) {
            Ok(())
        } else {
            Err(VfatIoError::DeviceError)
        }
    }
}

impl fatfs::IoBase for FatBlockDevice {
    type Error = VfatIoError;
}

impl fatfs::Read for FatBlockDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() || self.position >= self.total_bytes {
            return Ok(0);
        }

        let remaining = (self.total_bytes - self.position) as usize;
        let len = buf.len().min(remaining);
        let mut copied = 0;
        let mut block = vec![0u8; self.block_size];

        while copied < len {
            let absolute = self.position + copied as u64;
            let block_id = (absolute / self.block_size as u64) as usize;
            let block_offset = (absolute % self.block_size as u64) as usize;
            let chunk_len = (len - copied).min(self.block_size - block_offset);

            self.read_block(block_id, &mut block)?;
            buf[copied..copied + chunk_len]
                .copy_from_slice(&block[block_offset..block_offset + chunk_len]);
            copied += chunk_len;
        }

        self.position += copied as u64;
        Ok(copied)
    }
}

impl fatfs::Write for FatBlockDevice {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        let end = self
            .position
            .checked_add(buf.len() as u64)
            .ok_or(VfatIoError::OutOfBounds)?;
        if end > self.total_bytes {
            return Err(VfatIoError::OutOfBounds);
        }

        let mut written = 0;
        let mut block = vec![0u8; self.block_size];

        while written < buf.len() {
            let absolute = self.position + written as u64;
            let block_id = (absolute / self.block_size as u64) as usize;
            let block_offset = (absolute % self.block_size as u64) as usize;
            let chunk_len = (buf.len() - written).min(self.block_size - block_offset);

            if block_offset == 0 && chunk_len == self.block_size {
                self.write_block(block_id, &buf[written..written + chunk_len])?;
            } else {
                self.read_block(block_id, &mut block)?;
                block[block_offset..block_offset + chunk_len]
                    .copy_from_slice(&buf[written..written + chunk_len]);
                self.write_block(block_id, &block)?;
            }

            written += chunk_len;
        }

        self.position += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        if self.inner.flush() {
            Ok(())
        } else {
            Err(VfatIoError::DeviceError)
        }
    }
}

impl fatfs::Seek for FatBlockDevice {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        let next = match pos {
            fatfs::SeekFrom::Start(offset) => i128::from(offset),
            fatfs::SeekFrom::End(offset) => i128::from(self.total_bytes) + i128::from(offset),
            fatfs::SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
        };

        if next < 0 || next > i128::from(self.total_bytes) {
            return Err(VfatIoError::OutOfBounds);
        }

        self.position = next as u64;
        Ok(self.position)
    }
}
