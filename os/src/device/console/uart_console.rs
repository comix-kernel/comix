//! UART 控制台驱动模块

use alloc::{string::String, sync::Arc};

use crate::device::{
    console::{CONSOLES, Console},
    serial::SerialDriver,
};

struct UARTConsole {
    uart: Arc<dyn SerialDriver>,
}

impl Console for UARTConsole {
    fn write_str(&self, s: &str) {
        self.uart.write(s.as_bytes());
    }

    fn read_char(&self) -> char {
        let byte = self.uart.read();
        self.uart.write(&[byte]); // 回显
        byte as char
    }

    fn read_line(&self, buf: &mut String) {
        loop {
            let c = self.read_char();
            if c == '\n' || c == '\r' {
                break;
            }
            buf.push(c);
        }
    }

    fn flush(&self) {
        // UART 通常不需要显式刷新
    }
}

pub fn init(uart: Arc<dyn SerialDriver>) {
    let console = Arc::new(UARTConsole { uart });
    CONSOLES.write().push(console);
}
