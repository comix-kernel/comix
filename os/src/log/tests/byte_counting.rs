// os/src/log/tests/byte_counting.rs
//
// 测试精确字节计数功能

use super::*;

test_case!(test_unread_bytes_basic, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 初始状态：未读字节数应为 0
    kassert!(logger._log_unread_bytes() == 0);

    // 写入一条日志
    test_log!(logger, LogLevel::Info, "Test message");

    // 验证字节数增加
    let bytes_after_write = logger._log_unread_bytes();
    kassert!(bytes_after_write > 0);

    // 读取日志
    let _entry = logger._read_log();

    // 验证字节数减少
    let bytes_after_read = logger._log_unread_bytes();
    kassert!(bytes_after_read < bytes_after_write);

    // 应该回到 0（只有一条日志）
    kassert!(bytes_after_read == 0);
});

test_case!(test_unread_bytes_multiple, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入多条日志
    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let total_bytes = logger._log_unread_bytes();
    kassert!(total_bytes > 0);

    // 读取一条
    let _entry1 = logger._read_log();
    let bytes_after_one = logger._log_unread_bytes();
    kassert!(bytes_after_one < total_bytes);
    kassert!(bytes_after_one > 0); // 还有 2 条

    // 读取第二条
    let _entry2 = logger._read_log();
    let bytes_after_two = logger._log_unread_bytes();
    kassert!(bytes_after_two < bytes_after_one);
    kassert!(bytes_after_two > 0); // 还有 1 条

    // 读取第三条
    let _entry3 = logger._read_log();
    let bytes_after_three = logger._log_unread_bytes();
    kassert!(bytes_after_three == 0); // 全部读完
});

test_case!(test_unread_bytes_accuracy, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入一条已知长度的日志
    test_log!(logger, LogLevel::Info, "Hello");

    let reported_bytes = logger._log_unread_bytes();

    // 读取并格式化
    let entry = logger._read_log().unwrap();
    let formatted = super::super::log_core::format_log_entry(&entry);
    let actual_bytes = formatted.len();

    // 验证字节数准确
    // 注意：由于上下文信息（CPU ID, 任务 ID, 时间戳）可能不同，
    // 我们只验证报告的字节数在合理范围内
    kassert!(reported_bytes > 0);
    kassert!(reported_bytes >= actual_bytes - 10); // 允许少量误差（数字位数变化）
    kassert!(reported_bytes <= actual_bytes + 10);
});

test_case!(test_unread_bytes_different_lengths, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入不同长度的消息
    test_log!(logger, LogLevel::Info, "A"); // 短消息
    let bytes_short = logger._log_unread_bytes();

    test_log!(
        logger,
        LogLevel::Info,
        "This is a much longer message with more content"
    );
    let bytes_both = logger._log_unread_bytes();

    // 第二条消息应该增加更多字节
    kassert!(bytes_both > bytes_short);
    let diff = bytes_both - bytes_short;
    kassert!(diff > 30); // 长消息至少增加 30+ 字节
});

test_case!(test_unread_bytes_with_different_levels, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 不同级别的日志，长度略有不同（级别标签长度不同）
    test_log!(logger, LogLevel::Emergency, "Test"); // "[EMERG]" = 7 字符
    let bytes_emerg = logger._log_unread_bytes();

    logger._read_log(); // 清空

    test_log!(logger, LogLevel::Info, "Test"); // "[INFO]" = 6 字符
    let bytes_info = logger._log_unread_bytes();

    // 相同消息，不同级别，字节数应该略有不同
    kassert!(bytes_emerg != bytes_info);
});

test_case!(test_unread_bytes_empty_message, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 空消息
    test_log!(logger, LogLevel::Info, "");

    let bytes = logger._log_unread_bytes();
    // 即使消息为空，也有格式化开销（级别、时间戳、上下文等）
    kassert!(bytes > 40); // 至少有固定开销
});

test_case!(test_unread_bytes_persistence, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入日志
    test_log!(logger, LogLevel::Info, "Persistent");
    let bytes_initial = logger._log_unread_bytes();

    // 多次查询，字节数不变
    kassert!(logger._log_unread_bytes() == bytes_initial);
    kassert!(logger._log_unread_bytes() == bytes_initial);
    kassert!(logger._log_unread_bytes() == bytes_initial);

    // 读取后才变化
    logger._read_log();
    kassert!(logger._log_unread_bytes() == 0);
});
