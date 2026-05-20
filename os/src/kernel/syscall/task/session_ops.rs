use super::*;

/// 创建一个新的会话并设置进程组 ID
/// # 返回值
/// - 成功返回新会话的进程组 ID, 失败返回负错误码
pub fn setsid() -> c_int {
    let task = current_task();
    let mut t = task.lock();
    if t.pid == t.pgid {
        return -EPERM;
    }
    let new_pgid = t.pid;
    t.pgid = new_pgid;
    new_pgid as c_int
}
