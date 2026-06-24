//! System V shared memory registry.

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use core::ffi::{c_int, c_ulong};

use lazy_static::lazy_static;

use crate::{
    config::PAGE_SIZE,
    kernel::{Capabilities, current_task},
    mm::{
        address::{PageNum, Ppn},
        frame_allocator::{FrameTracker, alloc_frames},
    },
    sync::SpinLock,
    uapi::{
        errno::{EACCES, EEXIST, EINVAL, ENOENT, ENOMEM, EPERM},
        ipc::{IPC_CREAT, IPC_EXCL, IPC_PRIVATE, IpcPerm, KeyT, SHM_DEST, SHM_HUGETLB, ShmIdDs},
        time::TimeSpec,
    },
};

#[derive(Debug)]
pub struct ShmSegment {
    pub id: c_int,
    pub key: KeyT,
    pub size: usize,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub cuid: u32,
    pub cgid: u32,
    pub cpid: c_int,
    frames: Vec<FrameTracker>,
    inner: SpinLock<ShmSegmentState>,
}

#[derive(Debug)]
struct ShmSegmentState {
    marked_removed: bool,
    attach_count: usize,
    atime: i64,
    dtime: i64,
    ctime: i64,
    lpid: c_int,
}

impl ShmSegment {
    fn new(id: c_int, key: KeyT, size: usize, shmflg: c_int) -> Result<Self, c_int> {
        let pages = size.div_ceil(PAGE_SIZE);
        let frames = alloc_frames(pages).ok_or(ENOMEM)?;
        let cred = current_task().lock().credential;
        let now = unix_time();
        Ok(Self {
            id,
            key,
            size,
            mode: (shmflg as u32) & 0o777,
            uid: cred.euid,
            gid: cred.egid,
            cuid: cred.euid,
            cgid: cred.egid,
            cpid: current_task().lock().pid as c_int,
            frames,
            inner: SpinLock::new(ShmSegmentState {
                marked_removed: false,
                attach_count: 0,
                atime: 0,
                dtime: 0,
                ctime: now,
                lpid: 0,
            }),
        })
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn pages(&self) -> usize {
        self.frames.len()
    }

    pub fn ppn_at(&self, page_idx: usize) -> Option<Ppn> {
        self.frames.get(page_idx).map(FrameTracker::ppn)
    }

    pub fn mark_attached(&self, pid: c_int) {
        let mut inner = self.inner.lock();
        inner.attach_count += 1;
        inner.atime = unix_time();
        inner.lpid = pid;
    }

    pub fn mark_detached(&self, pid: c_int) -> bool {
        let mut inner = self.inner.lock();
        inner.attach_count = inner.attach_count.saturating_sub(1);
        inner.dtime = unix_time();
        inner.lpid = pid;
        inner.marked_removed && inner.attach_count == 0
    }

    pub fn mark_removed(&self) -> bool {
        let mut inner = self.inner.lock();
        inner.marked_removed = true;
        inner.ctime = unix_time();
        inner.attach_count == 0
    }

    pub fn is_removed(&self) -> bool {
        self.inner.lock().marked_removed
    }

    pub fn stat(&self) -> ShmIdDs {
        let inner = self.inner.lock();
        let mode = if inner.marked_removed {
            self.mode | SHM_DEST as u32
        } else {
            self.mode
        };
        ShmIdDs {
            shm_perm: IpcPerm {
                key: self.key,
                uid: self.uid,
                gid: self.gid,
                cuid: self.cuid,
                cgid: self.cgid,
                mode,
                seq: 0,
                ..IpcPerm::default()
            },
            shm_segsz: self.size,
            shm_atime: inner.atime,
            shm_dtime: inner.dtime,
            shm_ctime: inner.ctime,
            shm_cpid: self.cpid,
            shm_lpid: inner.lpid,
            shm_nattch: inner.attach_count as c_ulong,
            ..ShmIdDs::default()
        }
    }
}

#[derive(Debug)]
struct ShmRegistry {
    next_id: c_int,
    by_id: BTreeMap<c_int, Arc<ShmSegment>>,
    by_key: BTreeMap<KeyT, c_int>,
}

impl ShmRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            by_id: BTreeMap::new(),
            by_key: BTreeMap::new(),
        }
    }

    fn allocate_id(&mut self) -> c_int {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        id
    }

    fn get_or_create(&mut self, key: KeyT, size: usize, shmflg: c_int) -> Result<c_int, c_int> {
        if shmflg & SHM_HUGETLB != 0 {
            return Err(EINVAL);
        }

        if key != IPC_PRIVATE {
            if let Some(id) = self.by_key.get(&key).copied() {
                let segment = self.by_id.get(&id).cloned();
                if let Some(segment) = segment
                    && !segment.is_removed()
                {
                    if shmflg & IPC_CREAT != 0 && shmflg & IPC_EXCL != 0 {
                        return Err(EEXIST);
                    }
                    if size > segment.size {
                        return Err(EINVAL);
                    }
                    let requested = (shmflg as u32) & 0o666;
                    shm_check_mode_access(&segment, requested)?;
                    return Ok(id);
                }
                self.by_key.remove(&key);
            }
        }

        if key != IPC_PRIVATE && shmflg & IPC_CREAT == 0 {
            return Err(ENOENT);
        }
        if size == 0 {
            return Err(EINVAL);
        }

        let id = self.allocate_id();
        let segment = Arc::new(ShmSegment::new(id, key, size, shmflg)?);
        if key != IPC_PRIVATE {
            self.by_key.insert(key, id);
        }
        self.by_id.insert(id, segment);
        Ok(id)
    }

    fn get(&self, shmid: c_int) -> Result<Arc<ShmSegment>, c_int> {
        self.by_id.get(&shmid).cloned().ok_or(EINVAL)
    }

    fn mark_removed(&mut self, shmid: c_int) -> Result<(), c_int> {
        let segment = self.by_id.get(&shmid).cloned().ok_or(EINVAL)?;
        shm_check_control(&segment)?;
        if segment.key != IPC_PRIVATE && self.by_key.get(&segment.key).copied() == Some(shmid) {
            self.by_key.remove(&segment.key);
        }
        if segment.mark_removed() {
            self.remove_segment(shmid);
        }
        Ok(())
    }

    fn remove_after_detach(&mut self, shmid: c_int) {
        if let Some(segment) = self.by_id.get(&shmid)
            && segment.is_removed()
            && segment.inner.lock().attach_count == 0
        {
            self.remove_segment(shmid);
        }
    }

    fn remove_segment(&mut self, shmid: c_int) {
        if let Some(segment) = self.by_id.remove(&shmid)
            && segment.key != IPC_PRIVATE
            && self.by_key.get(&segment.key).copied() == Some(shmid)
        {
            self.by_key.remove(&segment.key);
        }
    }
}

lazy_static! {
    static ref SHM_REGISTRY: SpinLock<ShmRegistry> = SpinLock::new(ShmRegistry::new());
}

pub fn shmget_segment(key: KeyT, size: usize, shmflg: c_int) -> Result<c_int, c_int> {
    SHM_REGISTRY.lock().get_or_create(key, size, shmflg)
}

pub fn shm_segment(shmid: c_int) -> Result<Arc<ShmSegment>, c_int> {
    SHM_REGISTRY.lock().get(shmid)
}

pub fn shm_mark_removed(shmid: c_int) -> Result<(), c_int> {
    SHM_REGISTRY.lock().mark_removed(shmid)
}

pub fn shm_detach_segment(segment: &Arc<ShmSegment>, pid: c_int) {
    let should_remove = segment.mark_detached(pid);
    if should_remove {
        SHM_REGISTRY.lock().remove_after_detach(segment.id);
    }
}

pub fn shm_check_access(segment: &ShmSegment, readonly: bool) -> Result<(), c_int> {
    let requested = if readonly { 0o4 } else { 0o6 };
    shm_check_mode_access(segment, requested)
}

fn shm_check_mode_access(segment: &ShmSegment, requested: u32) -> Result<(), c_int> {
    if requested == 0 {
        return Ok(());
    }

    let cred = current_task().lock().credential;
    if cred.capabilities.has(Capabilities::IPC_OWNER) {
        return Ok(());
    }

    let available = if cred.euid == segment.uid || cred.euid == segment.cuid {
        (segment.mode >> 6) & 0o7
    } else if cred.egid == segment.gid || cred.egid == segment.cgid {
        (segment.mode >> 3) & 0o7
    } else {
        segment.mode & 0o7
    };

    if available & requested == requested {
        return Ok(());
    }
    Err(EACCES)
}

fn shm_check_control(segment: &ShmSegment) -> Result<(), c_int> {
    let cred = current_task().lock().credential;
    if cred.euid == segment.uid
        || cred.euid == segment.cuid
        || cred.capabilities.has(Capabilities::IPC_OWNER)
    {
        return Ok(());
    }
    Err(EPERM)
}

fn unix_time() -> i64 {
    TimeSpec::now().tv_sec
}
