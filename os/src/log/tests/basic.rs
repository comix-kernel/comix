// os/src/log/tests/basic.rs

use super::*;

test_case!(test_write_and_read, {
    // 创建独立的 LogCore 实例，启用 Debug 级别
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 写入日志
    test_log!(log, LogLevel::Info, "test message");

    // 验证
    kassert!(log._log_len() == 1);

    let entry = log._read_log().unwrap();
    kassert!(entry.message() == "test message");
    kassert!(entry.level() == LogLevel::Info);

    // 缓冲区应为空
    kassert!(log._log_len() == 0);
});

test_case!(test_format_arguments, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 测试格式化
    test_log!(log, LogLevel::Info, "value: {}", 42);
    test_log!(log, LogLevel::Debug, "hex: {:#x}", 0xDEAD);

    let e1 = log._read_log().unwrap();
    kassert!(e1.message() == "value: 42");

    let e2 = log._read_log().unwrap();
    kassert!(e2.message() == "hex: 0xdead");
});

test_case!(test_fifo_order, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 写入多条日志
    for i in 0..5 {
        test_log!(log, LogLevel::Debug, "message {}", i);
    }

    kassert!(log._log_len() == 5);

    // 按 FIFO 顺序读取
    for i in 0..5 {
        let entry = log._read_log().unwrap();
        // 检查消息中包含对应的数字
        let expected_msg = alloc::format!("message {}", i);
        kassert!(entry.message() == expected_msg.as_str());
    }

    kassert!(log._log_len() == 0);
});

test_case!(test_empty_buffer_read, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 空缓冲区
    kassert!(log._log_len() == 0);
    kassert!(log._read_log().is_none());

    // 多次读取空缓冲区
    kassert!(log._read_log().is_none());
    kassert!(log._read_log().is_none());
});
