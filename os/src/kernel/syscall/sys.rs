//! 系统相关系统调用实现

use core::ffi::{c_char, c_int};

use crate::{
    kernel::{HOST_NAME_MAX, current_task},
    tool::user_buffer::UserBuffer,
    uapi::errno::{EINVAL, ENAMETOOLONG},
};

/// 关闭系统调用
pub fn shutdown() -> ! {
    crate::shutdown(false);
}

/// 获取主机名系统调用
/// # 参数
/// - `buf`: 指向用户缓冲区的指针，用于存放主机名
/// - `len`: 缓冲区长度
/// # 返回值
/// 成功返回 0，失败返回负错误码
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
/// HACK: 由于某些关于类型原因, 这里name不得不声明为mut指针, 实际并不会修改它指向的数据
pub fn set_hostname(name: *mut c_char, len: usize) -> c_int {
    if len > HOST_NAME_MAX {
        return -EINVAL;
    }
    let uts = {
        let task = current_task();
        let t = task.lock();
        t.uts_namespace.clone()
    };
    let name_buf = UserBuffer::new(name, len);
    let name = unsafe { name_buf.copy_from_user() };
    {
        let mut uts_lock = uts.lock();
        uts_lock.nodename = name;
    }
    0
    // TODO: EPERM 和 EFAULT
}
