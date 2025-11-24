//! IO 相关的系统调用实现

use riscv::register::sstatus;

use crate::kernel::current_cpu;

/// 向文件描述符写入数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 要写入的数据缓冲区
/// - `count`: 要写入的字节数
pub fn write(fd: usize, buf: *const u8, count: usize) -> isize {
    // 1. 获取文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 2. 访问用户态缓冲区
    unsafe { sstatus::set_sum() };
    let buffer = unsafe { core::slice::from_raw_parts(buf, count) };

    // 3. 调用File::write（会自动处理O_APPEND和offset）
    let result = match file.write(buffer) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
    };

    unsafe { sstatus::clear_sum() };
    result
}

/// 从文件描述符读取数据
/// # 参数
/// - `fd`: 文件描述符
/// - `buf`: 存储读取数据的缓冲区
/// - `count`: 要读取的字节数
pub fn read(fd: usize, buf: *mut u8, count: usize) -> isize {
    // 1. 获取文件对象
    let task = current_cpu().lock().current_task.as_ref().unwrap().clone();
    let file = match task.lock().fd_table.get(fd) {
        Ok(f) => f,
        Err(e) => return e.to_errno(),
    };

    // 2. 访问用户态缓冲区
    unsafe { sstatus::set_sum() };
    let buffer = unsafe { core::slice::from_raw_parts_mut(buf, count) };

    // 3. 调用File::read（会自动更新offset）
    let result = match file.read(buffer) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno(),
    };

    unsafe { sstatus::clear_sum() };
    result
}
