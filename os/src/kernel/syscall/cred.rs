//! 用户凭证和权限相关的系统调用
//!
//! 在单 root 用户系统中，这些系统调用存储值但不实际限制权限

use crate::kernel::task::current_task;
use crate::uapi::cred::{GID_UNCHANGED, ROOT_GID, ROOT_UID, UID_UNCHANGED};
use crate::uapi::errno::{EFAULT, EPERM};
use crate::util::user_buffer::{validate_user_ptr_mut, write_to_user};

/// 获取真实用户 ID
///
/// 返回当前进程的真实用户 ID
pub fn getuid() -> isize {
    let task = current_task();
    let task_inner = task.lock();
    task_inner.credential.uid as isize
}

/// 获取有效用户 ID
///
/// 返回当前进程的有效用户 ID（用于权限检查）
pub fn geteuid() -> isize {
    let task = current_task();
    let task_inner = task.lock();
    task_inner.credential.euid as isize
}

/// 获取真实组 ID
///
/// 返回当前进程的真实组 ID
pub fn getgid() -> isize {
    let task = current_task();
    let task_inner = task.lock();
    task_inner.credential.gid as isize
}

/// 获取有效组 ID
///
/// 返回当前进程的有效组 ID（用于权限检查）
pub fn getegid() -> isize {
    let task = current_task();
    let task_inner = task.lock();
    task_inner.credential.egid as isize
}

/// 设置用户 ID
///
/// 在单 root 用户系统中，只允许设置为 0（root）
pub fn setuid(uid: u32) -> isize {
    if uid == ROOT_UID {
        // 在单 root 用户系统中，uid 始终是 0，不需要实际修改
        0
    } else {
        // 假装没有权限设置非 root 用户
        -EPERM as isize
    }
}

/// 设置组 ID
///
/// 在单 root 用户系统中，只允许设置为 0（root 组）
pub fn setgid(gid: u32) -> isize {
    if gid == ROOT_GID { 0 } else { -EPERM as isize }
}

/// 设置有效用户 ID
pub fn seteuid(euid: u32) -> isize {
    if euid == ROOT_UID { 0 } else { -EPERM as isize }
}

/// 设置有效组 ID
pub fn setegid(egid: u32) -> isize {
    if egid == ROOT_GID { 0 } else { -EPERM as isize }
}

/// 同时设置真实、有效和保存的用户 ID
///
/// # 参数
/// * `ruid` - 真实用户 ID（u32::MAX 表示不改变）
/// * `euid` - 有效用户 ID（u32::MAX 表示不改变）
/// * `suid` - 保存的用户 ID（u32::MAX 表示不改变）
///
/// # 返回值
/// * 0 - 成功
/// * -EPERM - 权限不足
/// * -EINVAL - 无效参数
pub fn setresuid(ruid: u32, euid: u32, suid: u32) -> isize {
    // 检查参数有效性：要么是 ROOT_UID，要么是 UID_UNCHANGED
    if (ruid != ROOT_UID && ruid != UID_UNCHANGED)
        || (euid != ROOT_UID && euid != UID_UNCHANGED)
        || (suid != ROOT_UID && suid != UID_UNCHANGED)
    {
        return -EPERM as isize;
    }

    // 在单 root 用户系统中，所有 ID 始终是 0，不需要实际修改
    0
}

/// 同时设置真实、有效和保存的组 ID
///
/// # 参数
/// * `rgid` - 真实组 ID（u32::MAX 表示不改变）
/// * `egid` - 有效组 ID（u32::MAX 表示不改变）
/// * `sgid` - 保存的组 ID（u32::MAX 表示不改变）
///
/// # 返回值
/// * 0 - 成功
/// * -EPERM - 权限不足
/// * -EINVAL - 无效参数
pub fn setresgid(rgid: u32, egid: u32, sgid: u32) -> isize {
    // 检查参数有效性：要么是 ROOT_GID，要么是 GID_UNCHANGED
    if (rgid != ROOT_GID && rgid != GID_UNCHANGED)
        || (egid != ROOT_GID && egid != GID_UNCHANGED)
        || (sgid != ROOT_GID && sgid != GID_UNCHANGED)
    {
        return -EPERM as isize;
    }

    // 在单 root 用户系统中，所有 ID 始终是 0，不需要实际修改
    0
}

/// 获取真实、有效和保存的用户 ID
///
/// # 参数
/// * `ruid` - 指向存储真实用户 ID 的指针
/// * `euid` - 指向存储有效用户 ID 的指针
/// * `suid` - 指向存储保存的用户 ID 的指针
///
/// # 返回值
/// * 0 - 成功
/// * -EFAULT - 指针无效（指向内核空间或不可写）
///
/// # 安全性
/// 此函数会验证所有指针是否指向有效的用户空间地址，
/// 防止恶意用户程序传入内核空间地址导致内核内存被破坏。
pub fn getresuid(ruid: *mut u32, euid: *mut u32, suid: *mut u32) -> isize {
    // 验证所有指针的有效性
    if !ruid.is_null() && !validate_user_ptr_mut(ruid) {
        return -(EFAULT as isize);
    }
    if !euid.is_null() && !validate_user_ptr_mut(euid) {
        return -(EFAULT as isize);
    }
    if !suid.is_null() && !validate_user_ptr_mut(suid) {
        return -(EFAULT as isize);
    }

    let task = current_task();
    let task_inner = task.lock();
    let cred = &task_inner.credential;

    // 安全地写入用户空间
    unsafe {
        if !ruid.is_null() {
            write_to_user(ruid, cred.uid);
        }
        if !euid.is_null() {
            write_to_user(euid, cred.euid);
        }
        if !suid.is_null() {
            write_to_user(suid, cred.suid);
        }
    }
    0
}

/// 获取真实、有效和保存的组 ID
///
/// # 参数
/// * `rgid` - 指向存储真实组 ID 的指针
/// * `egid` - 指向存储有效组 ID 的指针
/// * `sgid` - 指向存储保存的组 ID 的指针
///
/// # 返回值
/// * 0 - 成功
/// * -EFAULT - 指针无效（指向内核空间或不可写）
///
/// # 安全性
/// 此函数会验证所有指针是否指向有效的用户空间地址，
/// 防止恶意用户程序传入内核空间地址导致内核内存被破坏。
pub fn getresgid(rgid: *mut u32, egid: *mut u32, sgid: *mut u32) -> isize {
    // 验证所有指针的有效性
    if !rgid.is_null() && !validate_user_ptr_mut(rgid) {
        return -(EFAULT as isize);
    }
    if !egid.is_null() && !validate_user_ptr_mut(egid) {
        return -(EFAULT as isize);
    }
    if !sgid.is_null() && !validate_user_ptr_mut(sgid) {
        return -(EFAULT as isize);
    }

    let task = current_task();
    let task_inner = task.lock();
    let cred = &task_inner.credential;

    // 安全地写入用户空间
    unsafe {
        if !rgid.is_null() {
            write_to_user(rgid, cred.gid);
        }
        if !egid.is_null() {
            write_to_user(egid, cred.egid);
        }
        if !sgid.is_null() {
            write_to_user(sgid, cred.sgid);
        }
    }
    0
}

/// 设置文件创建掩码
///
/// # 参数
/// * `mask` - 新的 umask 值
///
/// # 返回值
/// 返回之前的 umask 值
///
/// # 注意
/// 在当前方案中，umask 值会被存储但不会实际应用到文件创建过程z
pub fn umask(mask: u32) -> isize {
    let task = current_task();
    let mut task_inner = task.lock();
    let old_umask = task_inner.umask;
    task_inner.umask = mask & 0o777; // 只保留权限位
    old_umask as isize
}
