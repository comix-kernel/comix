//! 系统相关系统调用实现

use core::ffi::{c_char, c_int, c_void};

use crate::{
    arch::lib::sbi::shutdown,
    kernel::current_task,
    pr_alert,
    tool::user_buffer::UserBuffer,
    uapi::{
        errno::{EINVAL, ENAMETOOLONG},
        reboot::{
            REBOOT_CMD_POWER_OFF, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_MAGIC2A, REBOOT_MAGIC2B,
            REBOOT_MAGIC2C,
        },
        uts_namespace::HOST_NAME_MAX,
    },
};

/// 重启系统调用
/// # 参数
/// - `magic`: 第一个魔数，必须为 REBOOT_MAGIC1
/// - `magic2`: 第二个魔数，必须为 REBOOT_MAGIC2 或 REBOOT_MAGIC2A/B/C
/// - `op`: 重启操作码，指定重启类型
/// - `arg`: 可选参数，取决于操作码
/// # 返回值
/// 成功返回 0，失败返回负错误码
/// 对于重启或关机操作，函数不会返回
pub fn reboot(magic: c_int, magic2: c_int, op: c_int, _arg: *mut c_void) -> c_int {
    // TODO: 支持更多重启操作码
    if magic as u32 != REBOOT_MAGIC1 {
        return -EINVAL;
    }
    if magic2 as u32 != REBOOT_MAGIC2
        && magic2 as u32 != REBOOT_MAGIC2A
        && magic2 as u32 != REBOOT_MAGIC2B
        && magic2 as u32 != REBOOT_MAGIC2C
    {
        return -EINVAL;
    }
    match op as u32 {
        REBOOT_CMD_POWER_OFF => {
            shutdown(true);
        }
        _ => {
            pr_alert!("reboot: unsupported reboot operation code {}\n", op);
        }
    }
    0
}

/// 获取主机名系统调用
/// # 参数
/// - `buf`: 指向用户缓冲区的指针，用于存放主机名
/// - `len`: 缓冲区长度
/// # 返回值
/// 成功返回 0，失败返回负错误码
/// 注意: 没有GETHOSTNAME系统调用, 该功能实际属于SYS_UNAME或sysinfo
pub fn get_hostname(buf: *mut c_char, len: usize) -> c_int {
    let mut result = 0;
    let task = current_task();
    let mut name = {
        let t = task.lock();
        t.uts_namespace.lock().nodename.clone()
    };
    name.push(0);
    if name.len() > len {
        result = -ENAMETOOLONG;
    }
    let buffer = UserBuffer::new(buf, len);
    unsafe {
        buffer.copy_to_user(&name);
    }
    result
    // TODO: EPERM 和 EFAULT
}

/// 设置主机名系统调用
/// # 参数
/// - `name`: 指向包含新主机名的用户缓冲区的指针
/// - `len`: 主机名长度
/// # 返回值
/// 成功返回 0，失败返回负错误码
pub fn set_hostname(name: *const c_char, len: usize) -> c_int {
    if len > HOST_NAME_MAX {
        return -EINVAL;
    }
    let uts = {
        let task = current_task();
        let t = task.lock();
        t.uts_namespace.clone()
    };
    let name_buf = UserBuffer::new(name as *mut _, len);
    let name = unsafe { name_buf.copy_from_user() };
    {
        let mut uts_lock = uts.lock();
        uts_lock.nodename = name;
    }
    0
    // TODO: EPERM 和 EFAULT
}
