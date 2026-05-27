//! 标准输入工具模块

use alloc::string::String;
use alloc::vec::Vec;

pub struct Stdin;

/// 使用 sbi 调用从控制台获取字符(qemu uart handler)
/// 返回值：字符的 ASCII 码
pub fn console_getchar() -> usize {
    crate::console::getchar().map_or(usize::MAX, |c| c as usize)
}

impl Stdin {
    pub fn read_char(&mut self) -> Option<char> {
        crate::console::getchar().map(|c| c as char)
    }

    pub fn read_line(&mut self, buf: &mut String) {
        let mut bytes = Vec::new();
        while let Some(c) = crate::console::getchar() {
            if c == b'\n' || c == b'\r' {
                break;
            }
            bytes.push(c);
        }
        buf.push_str(&String::from_utf8_lossy(&bytes));
    }
}

pub fn stdin() -> Stdin {
    Stdin
}
