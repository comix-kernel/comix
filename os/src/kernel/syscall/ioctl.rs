//! ioctl 系统调用实现
//!
//! ioctl (input/output control) 是一个多功能的系统调用，用于设备特定的控制操作。

use crate::arch::trap::SumGuard;
use crate::kernel::current_task;
use crate::uapi::errno::{EBADF, EINVAL, ENOTTY, EOPNOTSUPP};
use crate::uapi::ioctl::*;
use crate::vfs::FsError;
use crate::{pr_debug, pr_err, pr_warn};

/// ioctl - 设备特定的输入/输出控制
///
/// # 参数
/// - `fd`: 文件描述符
/// - `request`: ioctl 请求码（由 _IO, _IOR, _IOW, _IOWR 宏构造）
/// - `arg`: 参数指针（根据 request 类型解释）
///
/// # 返回值
/// - 成功: 返回 0 或设备特定的值
/// - 失败: 返回 -errno
///
/// # 支持的操作
///
/// ## 通用文件操作
/// - `FIONBIO` - 设置非阻塞模式
/// - `FIONREAD` - 获取可读字节数
/// - `FIOASYNC` - 设置异步 I/O
///
/// ## 终端操作
/// - `TIOCGWINSZ` - 获取终端窗口大小
/// - `TIOCSWINSZ` - 设置终端窗口大小
/// - `TCGETS` - 获取终端属性
/// - `TCSETS` - 设置终端属性
///
/// ## 网络操作
/// - `SIOCGIFCONF` - 获取网络接口列表
/// - `SIOCGIFADDR` - 获取接口地址
/// - `SIOCGIFFLAGS` - 获取接口标志
/// - 等等（详见 uapi/ioctl.rs）
///
/// # 注意
/// - 大部分 ioctl 操作需要相应的设备驱动程序支持
/// - 无效的 request 码会返回 ENOTTY
pub fn ioctl(fd: i32, request: u32, arg: usize) -> isize {
    pr_debug!(
        "ioctl: fd={}, request={:#x} ({}), arg={:#x}",
        fd,
        request,
        request,
        arg
    );

    // 参数验证
    if fd < 0 {
        pr_err!("ioctl: invalid fd {}", fd);
        return -EBADF as isize;
    }

    // 获取文件对象
    let task = current_task();
    let file = {
        let task_lock = task.lock();
        match task_lock.fd_table.get(fd as usize) {
            Ok(f) => f,
            Err(_) => {
                pr_err!("ioctl: fd {} not found", fd);
                return -EBADF as isize;
            }
        }
    };

    // 根据 request 类型分发处理
    let result = match request {
        //  通用文件 I/O 控制
        FIONBIO => handle_fionbio(&file, arg),
        FIONREAD => handle_fionread(&file, arg),
        FIOASYNC => handle_fioasync(&file, arg),

        //  终端控制 - 委托给文件对象的 ioctl 方法
        TIOCGWINSZ | TIOCSWINSZ | TCGETS | TCSETS | TCSETSW | TCSETSF => {
            match file.ioctl(request, arg) {
                Ok(ret) => ret,
                Err(FsError::NotSupported) => {
                    pr_warn!(
                        "ioctl: fd={}, terminal request {:#x} ({}) not supported by file type",
                        fd,
                        request,
                        request
                    );
                    -ENOTTY as isize
                }
                Err(e) => fs_error_to_errno(e),
            }
        }

        //  终端进程组控制 - 读取/设置任务的 pgid
        TIOCGPGRP => handle_tiocgpgrp(&task, arg),
        TIOCSPGRP => handle_tiocspgrp(&task, arg),

        //  控制终端设置
        TIOCSCTTY => handle_tiocsctty(&file, arg),

        //  虚拟终端查询
        VT_OPENQRY => handle_vt_openqry(arg),

        //  网络 Socket 控制
        SIOCGIFCONF => handle_siocgifconf(arg),
        SIOCGIFADDR | SIOCSIFADDR | SIOCGIFFLAGS | SIOCSIFFLAGS | SIOCGIFNETMASK
        | SIOCSIFNETMASK | SIOCGIFMTU | SIOCSIFMTU | SIOCGIFHWADDR | SIOCSIFHWADDR
        | SIOCGIFINDEX => handle_ifreq(&file, request, arg),

        //  设备特定
        // 尝试委托给文件对象的 ioctl 方法
        _ => {
            pr_debug!("ioctl: delegating request {:#x} to file object", request);
            match file.ioctl(request, arg) {
                Ok(ret) => ret,
                Err(FsError::NotSupported) => {
                    pr_warn!(
                        "ioctl: unsupported request {:#x} (type={:#x}, nr={}, size={})",
                        request,
                        _IOC_TYPE(request),
                        _IOC_NR(request),
                        _IOC_SIZE(request)
                    );
                    -ENOTTY as isize
                }
                Err(e) => {
                    pr_err!("ioctl: file ioctl failed: {:?}", e);
                    fs_error_to_errno(e)
                }
            }
        }
    };

    pr_debug!(
        "ioctl: fd={}, request={:#x} => result={}",
        fd,
        request,
        result
    );
    result
}

//  通用文件 I/O 控制处理函数

/// FIONBIO - 设置/清除非阻塞 I/O 标志
fn handle_fionbio(file: &alloc::sync::Arc<dyn crate::vfs::File>, arg: usize) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let value_ptr = arg as *const i32;
        if value_ptr.is_null() {
            return -EINVAL as isize;
        }

        let value = core::ptr::read_volatile(value_ptr);

        // 设置文件的 O_NONBLOCK 标志
        let mut flags = file.flags();
        if value != 0 {
            flags |= crate::uapi::fcntl::OpenFlags::O_NONBLOCK;
        } else {
            flags &= !crate::uapi::fcntl::OpenFlags::O_NONBLOCK;
        }

        match file.set_status_flags(flags) {
            Ok(_) => 0,
            Err(e) => {
                pr_warn!("ioctl: FIONBIO failed: {:?}", e);
                -EOPNOTSUPP as isize
            }
        }
    }
}

/// FIONREAD - 获取可读字节数
fn handle_fionread(file: &alloc::sync::Arc<dyn crate::vfs::File>, arg: usize) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let value_ptr = arg as *mut i32;
        if value_ptr.is_null() {
            return -EINVAL as isize;
        }

        // 对于普通文件，可读字节数 = 文件大小 - 当前偏移量
        let available = match file.metadata() {
            Ok(meta) => {
                let size = meta.size;
                let offset = file.offset();
                if size > offset {
                    (size - offset) as i32
                } else {
                    0
                }
            }
            Err(_) => {
                // 对于不支持 metadata 的设备，返回 0
                0
            }
        };

        core::ptr::write_volatile(value_ptr, available);
        0
    }
}

/// FIOASYNC - 设置/清除异步 I/O 通知
fn handle_fioasync(file: &alloc::sync::Arc<dyn crate::vfs::File>, arg: usize) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let value_ptr = arg as *const i32;
        if value_ptr.is_null() {
            return -EINVAL as isize;
        }

        let _value = core::ptr::read_volatile(value_ptr);

        // TODO: 实现异步 I/O 支持
        pr_warn!("ioctl: FIOASYNC not yet implemented");
        -EOPNOTSUPP as isize
    }
}

//  终端控制处理函数

/// TIOCGPGRP - 获取终端前台进程组 ID
fn handle_tiocgpgrp(
    task: &alloc::sync::Arc<crate::sync::SpinLock<crate::kernel::task::TaskStruct>>,
    arg: usize,
) -> isize {
    if arg == 0 {
        return -EINVAL as isize;
    }

    let pgid = task.lock().pgid as i32;
    pr_debug!("ioctl: TIOCGPGRP writing pgid={} to {:#x}", pgid, arg);

    unsafe {
        crate::util::user_buffer::write_to_user(arg as *mut i32, pgid);
    }

    pr_debug!("ioctl: TIOCGPGRP completed");
    0
}

/// TIOCSPGRP - 设置终端前台进程组 ID
fn handle_tiocspgrp(
    task: &alloc::sync::Arc<crate::sync::SpinLock<crate::kernel::task::TaskStruct>>,
    arg: usize,
) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let pid_ptr = arg as *const i32;
        if pid_ptr.is_null() {
            return -EINVAL as isize;
        }

        let pgid = core::ptr::read_volatile(pid_ptr);

        // 设置当前任务的进程组 ID
        task.lock().pgid = pgid as u32;

        pr_debug!("ioctl: TIOCSPGRP set pgid={}", pgid);
        0
    }
}

/// TIOCSCTTY - 设置控制终端
///
/// 这个 ioctl 用于使当前终端成为调用进程的控制终端。
/// busybox init 在启动时会调用这个函数。
/// 目前我们只是简单返回成功，因为我们还没有完整的会话管理。
fn handle_tiocsctty(_file: &alloc::sync::Arc<dyn crate::vfs::File>, _arg: usize) -> isize {
    // TODO: 实现完整的控制终端管理
    // 目前只是返回成功，让 init 可以继续运行
    pr_debug!("ioctl: TIOCSCTTY accepted (not fully implemented)");
    0
}

/// VT_OPENQRY - 查询可用的虚拟终端
///
/// 这个 ioctl 用于查找第一个未打开的虚拟终端号。
/// 对于不支持虚拟终端的系统，返回 ENOTTY 是合理的。
fn handle_vt_openqry(arg: usize) -> isize {
    if arg == 0 {
        return -EINVAL as isize;
    }

    // 对于不支持虚拟终端的系统，返回 ENOTTY
    pr_debug!("ioctl: VT_OPENQRY not supported (no VT subsystem)");
    -ENOTTY as isize
}

//  网络控制处理函数

/// SIOCGIFCONF - 获取网络接口列表
fn handle_siocgifconf(arg: usize) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let ifconf_ptr = arg as *mut Ifconf;
        if ifconf_ptr.is_null() {
            return -EINVAL as isize;
        }

        // 读取 ifconf 结构
        let ifconf = core::ptr::read_volatile(ifconf_ptr);

        // 先清零原结构体，避免泄露内核栈数据
        core::ptr::write_bytes(ifconf_ptr, 0, 1);

        // TODO: 填充实际的网络接口列表
        // 目前返回空列表
        let mut new_ifconf = ifconf;
        new_ifconf.ifc_len = 0;
        core::ptr::write_volatile(ifconf_ptr, new_ifconf);

        pr_debug!("ioctl: SIOCGIFCONF returned 0 interfaces");
        0
    }
}

/// 处理网络接口请求（ifreq 结构）
fn handle_ifreq(_file: &alloc::sync::Arc<dyn crate::vfs::File>, request: u32, arg: usize) -> isize {
    unsafe {
        let _guard = SumGuard::new();
        let ifreq_ptr = arg as *mut Ifreq;
        if ifreq_ptr.is_null() {
            return -EINVAL as isize;
        }

        let _ifreq = core::ptr::read_volatile(ifreq_ptr);

        // TODO: 实现实际的网络接口操作
        match request {
            SIOCGIFADDR | SIOCGIFFLAGS | SIOCGIFNETMASK | SIOCGIFMTU | SIOCGIFHWADDR
            | SIOCGIFINDEX => {
                pr_debug!("ioctl: network get request {:#x} not implemented", request);
                -EOPNOTSUPP as isize
            }
            SIOCSIFADDR | SIOCSIFFLAGS | SIOCSIFNETMASK | SIOCSIFMTU | SIOCSIFHWADDR => {
                pr_debug!("ioctl: network set request {:#x} not implemented", request);
                -EOPNOTSUPP as isize
            }
            _ => -EINVAL as isize,
        }
    }
}

//  辅助函数

/// 将 VFS 错误转换为 errno
fn fs_error_to_errno(err: FsError) -> isize {
    match err {
        FsError::NotSupported => -EOPNOTSUPP as isize,
        FsError::InvalidArgument => -EINVAL as isize,
        FsError::NotFound => -crate::uapi::errno::ENOENT as isize,
        FsError::PermissionDenied => -crate::uapi::errno::EACCES as isize,
        FsError::AlreadyExists => -crate::uapi::errno::EEXIST as isize,
        FsError::IoError => -crate::uapi::errno::EIO as isize,
        _ => -crate::uapi::errno::EIO as isize,
    }
}
