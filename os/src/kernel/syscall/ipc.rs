//! IPC 相关的系统调用实现

use alloc::sync::Arc;

use crate::{
    config::PAGE_SIZE,
    ipc::{shm_check_access, shm_detach_segment, shm_mark_removed, shm_segment, shmget_segment},
    kernel::{ShmAttachment, current_memory_space, current_task},
    mm::{
        address::{PageNum, VA, Vpn, VpnRange},
        page_table::UniversalPTEFlag,
    },
    uapi::{
        errno::{EFAULT, EINVAL},
        ipc::{
            IPC_RMID, IPC_STAT, KeyT, SHM_EXEC, SHM_RDONLY, SHM_REMAP, SHM_RND, SHMLBA, ShmIdDs,
        },
    },
    util::user_buffer::write_to_user,
    vfs::{FdFlags, File, FsError, OpenFlags, PipeFile},
};

pub fn dup(oldfd: usize) -> isize {
    let task = current_task();
    match task.lock().fd_table.dup(oldfd) {
        Ok(newfd) => newfd as isize,
        Err(e) => e.to_errno(),
    }
}

pub fn dup3(oldfd: usize, newfd: usize, flags: u32) -> isize {
    let task = current_task();

    let open_flags = match OpenFlags::from_bits(flags) {
        Some(f) => f,
        None => return FsError::InvalidArgument.to_errno(),
    };

    if open_flags.bits() & !OpenFlags::O_CLOEXEC.bits() != 0 {
        return FsError::InvalidArgument.to_errno();
    }

    match task.lock().fd_table.dup3(oldfd, newfd, open_flags) {
        Ok(newfd) => newfd as isize,
        Err(e) => e.to_errno(),
    }
}

pub fn pipe2(pipefd: *mut i32, flags: u32) -> isize {
    if pipefd.is_null() {
        return FsError::InvalidArgument.to_errno();
    }

    let valid_flags = OpenFlags::O_CLOEXEC | OpenFlags::O_NONBLOCK;
    if flags & !valid_flags.bits() != 0 {
        return FsError::InvalidArgument.to_errno();
    }

    let fd_flags =
        FdFlags::from_open_flags(OpenFlags::from_bits(flags).unwrap_or(OpenFlags::empty()));

    let (pipe_read, pipe_write) = PipeFile::create_pair();

    // 获取当前任务的 FD 表
    let fd_table = current_task().lock().fd_table.clone();

    // 分配文件描述符
    let read_fd = match fd_table.alloc_with_flags(Arc::new(pipe_read) as Arc<dyn File>, fd_flags) {
        Ok(fd) => fd,
        Err(e) => return e.to_errno(),
    };

    let write_fd = match fd_table.alloc_with_flags(Arc::new(pipe_write) as Arc<dyn File>, fd_flags)
    {
        Ok(fd) => fd,
        Err(e) => {
            // 分配失败，需要回滚读端 FD
            let _ = fd_table.close(read_fd);
            return e.to_errno();
        }
    };

    // 将 FD 写回用户空间
    unsafe {
        write_to_user(pipefd, read_fd as i32);
        write_to_user(pipefd.add(1), write_fd as i32);
    }

    0
}

pub fn shmget(key: KeyT, size: usize, shmflg: i32) -> isize {
    match shmget_segment(key, size, shmflg) {
        Ok(id) => id as isize,
        Err(errno) => -errno as isize,
    }
}

pub fn shmctl(shmid: i32, cmd: i32, buf: *mut ShmIdDs) -> isize {
    match cmd {
        IPC_STAT => {
            if buf.is_null() {
                return -EFAULT as isize;
            }
            let segment = match shm_segment(shmid) {
                Ok(segment) => segment,
                Err(errno) => return -errno as isize,
            };
            write_to_user(buf, segment.stat());
            0
        }
        IPC_RMID => match shm_mark_removed(shmid) {
            Ok(()) => 0,
            Err(errno) => -errno as isize,
        },
        _ => -EINVAL as isize,
    }
}

pub fn shmat(shmid: i32, shmaddr: *const u8, shmflg: i32) -> isize {
    let unsupported = shmflg & !(SHM_RDONLY | SHM_RND | SHM_REMAP | SHM_EXEC);
    if unsupported != 0 {
        return -EINVAL as isize;
    }
    if shmflg & SHM_REMAP != 0 && shmaddr.is_null() {
        return -EINVAL as isize;
    }

    let readonly = shmflg & SHM_RDONLY != 0;
    let segment = match shm_segment(shmid) {
        Ok(segment) => segment,
        Err(errno) => return -errno as isize,
    };
    if let Err(errno) = shm_check_access(&segment, readonly) {
        return -errno as isize;
    }

    let len = segment.pages() * PAGE_SIZE;
    let hint = shmaddr as usize;
    let start = if hint == 0 {
        let space = current_memory_space();
        match space.lock().find_free_region(len, PAGE_SIZE) {
            Some(addr) => addr.as_usize(),
            None => return -crate::uapi::errno::ENOMEM as isize,
        }
    } else if shmflg & SHM_RND != 0 {
        hint & !(SHMLBA - 1)
    } else if !hint.is_multiple_of(PAGE_SIZE) {
        return -EINVAL as isize;
    } else {
        hint
    };
    let end = match start.checked_add(len) {
        Some(end) => end,
        None => return -EINVAL as isize,
    };

    let start_vpn = Vpn::from_addr_floor(VA::from_usize(start));
    let end_vpn = Vpn::from_addr_ceil(VA::from_usize(end));
    let range = VpnRange::new(start_vpn, end_vpn);

    let mut flags =
        UniversalPTEFlag::VALID | UniversalPTEFlag::READABLE | UniversalPTEFlag::USER_ACCESSIBLE;
    if !readonly {
        flags |= UniversalPTEFlag::WRITEABLE;
    }
    if shmflg & SHM_EXEC != 0 {
        flags |= UniversalPTEFlag::EXECUTABLE;
    }

    let memory_space = current_memory_space();
    let mut space = memory_space.lock();
    if shmflg & SHM_REMAP != 0 {
        if let Err(_) = space.munmap(VA::from_usize(start), len) {
            return -EINVAL as isize;
        }
    }
    if let Err(_) = space.insert_shared_area(range, flags, segment.clone()) {
        return -EINVAL as isize;
    }
    drop(space);

    let task = current_task();
    let (pid, table) = {
        let t = task.lock();
        (t.pid as i32, t.shm_attachments.clone())
    };
    if let Some(old) = table.lock().remove(&start) {
        shm_detach_segment(&old.segment, pid);
    }
    segment.mark_attached(pid);
    table.lock().insert(start, ShmAttachment {
        addr: start,
        len,
        segment,
    });

    start as isize
}

pub fn shmdt(shmaddr: *const u8) -> isize {
    let addr = shmaddr as usize;
    if !addr.is_multiple_of(PAGE_SIZE) {
        return -EINVAL as isize;
    }

    let task = current_task();
    let (pid, table, attachment) = {
        let t = task.lock();
        let pid = t.pid as i32;
        let table = t.shm_attachments.clone();
        let attachment = match table.lock().remove(&addr) {
            Some(attachment) => attachment,
            None => return -EINVAL as isize,
        };
        (pid, table, attachment)
    };

    let memory_space = current_memory_space();
    if let Err(_) = memory_space
        .lock()
        .munmap(VA::from_usize(attachment.addr), attachment.len)
    {
        table.lock().insert(addr, attachment);
        return -EINVAL as isize;
    }

    shm_detach_segment(&attachment.segment, pid);
    0
}
