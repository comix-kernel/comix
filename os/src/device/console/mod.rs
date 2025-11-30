//! 控制台驱动模块

pub mod frame_console;
pub mod uart_console;

use alloc::{string::String, sync::Arc, vec::Vec};
use spin::RwLock;

lazy_static::lazy_static! {
    /// 全局控制台列表
    pub static ref CONSOLES: RwLock<Vec<Arc<dyn Console>>> = RwLock::new(Vec::new());
    /// 全局主控制台
    pub static ref MAIN_CONSOLE: RwLock<Option<Arc<dyn Console>>> = RwLock::new(None);
}

pub trait Console: Send + Sync {
    /// 向控制台写入字符串
    fn write_str(&self, s: &str);

    /// 从控制台读取一个字符
    fn read_char(&self) -> char;

    /// 从控制台读取一行字符串，直到遇到换行符
    fn read_line(&self, buf: &mut String);

    /// 刷新控制台输出缓冲区
    fn flush(&self);
}

/// 初始化控制台设备
pub fn init() {
    MAIN_CONSOLE.write().replace(CONSOLES.read()[0].clone());
    // frame_console::init();
}
