//! 信号相关的系统调用实现

use core::ffi::{c_int, c_ulong};

use crate::{ipc::do_sigpending, tool::user_buffer::write_to_user};

/// 获取当前任务的待处理信号集合, 包括私有和共享的信号集合
/// # 参数：
/// * `uset` - 指向用户空间缓冲区的指针，用于存放待处理信号集合
pub fn sigpending(uset: *mut c_ulong) -> c_int {
    let pending = do_sigpending();
    unsafe {
        write_to_user(uset, pending.bits() as c_ulong);
    }
    0
}
