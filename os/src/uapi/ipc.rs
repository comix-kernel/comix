//! System V IPC UAPI subset.

use core::ffi::{c_int, c_long, c_uint, c_ulong, c_ushort};

pub type KeyT = c_int;

pub const IPC_PRIVATE: KeyT = 0;

pub const IPC_CREAT: c_int = 0o1000;
pub const IPC_EXCL: c_int = 0o2000;
pub const IPC_NOWAIT: c_int = 0o4000;

pub const IPC_RMID: c_int = 0;
pub const IPC_SET: c_int = 1;
pub const IPC_STAT: c_int = 2;
pub const IPC_INFO: c_int = 3;

pub const SHM_HUGETLB: c_int = 0o4000;
pub const SHM_NORESERVE: c_int = 0o10000;

pub const SHM_DEST: c_int = 0o1000;
pub const SHM_RDONLY: c_int = 0o10000;
pub const SHM_RND: c_int = 0o20000;
pub const SHM_REMAP: c_int = 0o40000;
pub const SHM_EXEC: c_int = 0o100000;

pub const SHMLBA: usize = crate::config::PAGE_SIZE;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IpcPerm {
    pub key: KeyT,
    pub uid: c_uint,
    pub gid: c_uint,
    pub cuid: c_uint,
    pub cgid: c_uint,
    pub mode: c_uint,
    pub seq: c_ushort,
    pub __pad2: c_ushort,
    pub __unused1: c_ulong,
    pub __unused2: c_ulong,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShmIdDs {
    pub shm_perm: IpcPerm,
    pub shm_segsz: usize,
    pub shm_atime: c_long,
    pub shm_dtime: c_long,
    pub shm_ctime: c_long,
    pub shm_cpid: c_int,
    pub shm_lpid: c_int,
    pub shm_nattch: c_ulong,
    pub __unused4: c_ulong,
    pub __unused5: c_ulong,
}
