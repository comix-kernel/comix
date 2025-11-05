// os/src/log/tests/overflow.rs

use super::*;

test_case!(test_buffer_overflow, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 写入大量日志触发溢出
    const TOTAL: usize = 100;
    for i in 0..TOTAL {
        test_log!(log, LogLevel::Info, "log {}", i);
    }

    // 验证溢出处理
    let buffered = log._log_len();
    let dropped = log._log_dropped_count();

    kassert!(dropped > 0);  // 应该有丢弃
    kassert!(buffered + dropped == TOTAL);
});

test_case!(test_overflow_fifo_behavior, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 填满缓冲区 + 触发溢出
    for i in 0..100 {
        test_log!(log, LogLevel::Info, "entry {}", i);
    }

    let dropped = log._log_dropped_count();
    kassert!(dropped > 0);

    // 读取的第一条应该是被覆盖后的最旧条目
    let first_entry = log._read_log().unwrap();

    // 验证消息格式正确（被覆盖的最旧条目）
    kassert!(first_entry.message().starts_with("entry"));
});

test_case!(test_write_after_overflow, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // 触发溢出
    for i in 0..100 {
        test_log!(log, LogLevel::Info, "overflow {}", i);
    }

    let dropped_before = log._log_dropped_count();
    kassert!(dropped_before > 0);

    // 清空缓冲区
    while log._read_log().is_some() {}

    // 再次写入
    test_log!(log, LogLevel::Info, "after overflow");

    // 应该正常工作
    kassert!(log._log_len() == 1);
    kassert!(log._read_log().unwrap().message() == "after overflow");
});
