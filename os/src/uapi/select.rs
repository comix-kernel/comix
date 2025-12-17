//! select() system call definitions

use core::mem;

/// Maximum number of file descriptors in fd_set
pub const FD_SETSIZE: usize = 1024;

/// Number of longs needed to hold FD_SETSIZE bits
const NFDBITS: usize = 8 * mem::size_of::<usize>();
const FD_SET_LONGS: usize = (FD_SETSIZE + NFDBITS - 1) / NFDBITS;

/// File descriptor set for select()
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FdSet {
    fds_bits: [usize; FD_SET_LONGS],
}

impl FdSet {
    /// Create a new empty fd_set
    pub fn new() -> Self {
        Self {
            fds_bits: [0; FD_SET_LONGS],
        }
    }

    /// Clear all bits (FD_ZERO)
    pub fn zero(&mut self) {
        self.fds_bits = [0; FD_SET_LONGS];
    }

    /// Set a bit (FD_SET)
    pub fn set(&mut self, fd: usize) {
        if fd < FD_SETSIZE {
            self.fds_bits[fd / NFDBITS] |= 1 << (fd % NFDBITS);
        }
    }

    /// Clear a bit (FD_CLR)
    pub fn clear(&mut self, fd: usize) {
        if fd < FD_SETSIZE {
            self.fds_bits[fd / NFDBITS] &= !(1 << (fd % NFDBITS));
        }
    }

    /// Test a bit (FD_ISSET)
    pub fn is_set(&self, fd: usize) -> bool {
        if fd < FD_SETSIZE {
            (self.fds_bits[fd / NFDBITS] & (1 << (fd % NFDBITS))) != 0
        } else {
            false
        }
    }
}
