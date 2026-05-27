//! 控制台驱动模块

pub mod frame_console;
pub mod uart_console;

use crate::sync::RwLock;
use alloc::{string::String, sync::Arc, vec::Vec};

lazy_static::lazy_static! {
    /// 全局控制台列表
    pub static ref CONSOLES: RwLock<Vec<Arc<dyn Console>>> = RwLock::new(Vec::new());
    /// 全局主控制台
    pub static ref MAIN_CONSOLE: RwLock<Option<Arc<dyn Console>>> = RwLock::new(None);
}

pub trait Console: Send + Sync {
    /// 向控制台写入字符串
    fn write_str(&self, s: &str);

    /// 向控制台写入原始字节
    fn write_bytes(&self, bytes: &[u8]) {
        if let Ok(s) = core::str::from_utf8(bytes) {
            self.write_str(s);
        }
    }

    /// 从控制台读取一个字符
    fn read_char(&self) -> char;

    /// 从控制台读取一行字符串，直到遇到换行符
    fn read_line(&self, buf: &mut String);

    /// 刷新控制台输出缓冲区
    fn flush(&self);
}

/// 初始化控制台设备
pub fn init() {
    let Some(console) = CONSOLES.read().first().cloned() else {
        crate::println!("[Console] No runtime console registered, keeping early console");
        return;
    };

    MAIN_CONSOLE.write().replace(console);
    // frame_console::init();

    // 切换到运行时控制台
    crate::console::init();
    crate::pr_info!("[Console] Switched to runtime console");
}
