use crate::{read, write};

/// 打印字符串到标准输出 (文件描述符 1)
///
/// # 参数
/// - `s`: 要打印的字符串切片
pub fn print(s: &[u8]) {
    let fd: usize = 1; // 标准输出

    let result = unsafe { write(fd, s, s.len()) };

    if result < 0 {
        // 在实际系统中，这里应该处理写入错误，例如 log 错误信息
        // eprintln!("Error writing to console: {}", result);
    }
}

/// 从标准输入 (文件描述符 0) 读取一行文本
///
/// 通过循环反复调用底层的单字节读取，直到遇到换行符或缓冲区满。
/// 支持回显输入的字符到标准输出。
///
/// # 参数
/// - `buffer`: 可变的字节切片，用于接收读取的数据。
///
/// # 返回值
/// 成功读取的字节数（不包含换行符 `\n`，如果不溢出的话）。失败时返回 0。
pub fn read_line(buffer: &mut [u8]) -> usize {
    let fd: usize = 0; // 标准输入 (stdin)
    let max_len = buffer.len();
    let mut current_pos = 0;

    while current_pos < max_len {
        let mut byte = [0u8; 1];

        let result = unsafe { read(fd, &mut byte, 1) };

        if result < 0 {
            return 0;
        }

        if result == 0 {
            // 遇到 EOF (文件末尾)
            break;
        }

        let ch = byte[0];

        // 处理退格键
        if ch == 8 || ch == 127 {
            // ASCII 8 = Backspace, 127 = DEL
            if current_pos > 0 {
                current_pos -= 1;
                // 回显: 退格 + 空格 + 退格
                print(b"\x08 \x08");
            }
            continue;
        }

        // readline不读取换行符
        if ch == b'\n' || ch == b'\r' {
            print(b"\n"); // 回显换行
            break;
        }

        // 回显输入的字符
        print(&[ch]);

        buffer[current_pos] = ch;
        current_pos += 1;
    }

    // 返回实际读取的字节总数
    current_pos
}
