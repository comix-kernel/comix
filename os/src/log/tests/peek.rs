// os/src/log/tests/peek.rs
//
// 测试非破坏性读取（peek）功能

use super::*;

test_case!(test_peek_basic, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入一条日志
    test_log!(logger, LogLevel::Info, "Test message");

    let start_index = logger._log_reader_index();
    let end_index = logger._log_writer_index();

    // 验证有一条日志
    kassert!(end_index == start_index + 1);

    // Peek 读取
    let entry = logger._peek_log(start_index);
    kassert!(entry.is_some());

    // 验证读指针未移动
    kassert!(logger._log_reader_index() == start_index);

    // 可以再次 peek 相同的条目
    let entry2 = logger._peek_log(start_index);
    kassert!(entry2.is_some());
    kassert!(logger._log_reader_index() == start_index);
});

test_case!(test_peek_vs_read, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入两条日志
    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");

    let start_index = logger._log_reader_index();

    // Peek 第一条
    let entry1_peek = logger._peek_log(start_index).unwrap();
    kassert!(logger._log_reader_index() == start_index); // 指针未动

    // Read 第一条
    let entry1_read = logger._read_log().unwrap();
    kassert!(logger._log_reader_index() == start_index + 1); // 指针移动

    // 验证内容相同
    kassert!(entry1_peek.message() == entry1_read.message());
});

test_case!(test_peek_multiple, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入多条日志
    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let start_index = logger._log_reader_index();
    let end_index = logger._log_writer_index();

    // Peek 所有条目
    let entry1 = logger._peek_log(start_index);
    let entry2 = logger._peek_log(start_index + 1);
    let entry3 = logger._peek_log(start_index + 2);

    kassert!(entry1.is_some());
    kassert!(entry2.is_some());
    kassert!(entry3.is_some());

    // 读指针始终未动
    kassert!(logger._log_reader_index() == start_index);

    // 验证消息内容
    kassert!(entry1.unwrap().message().contains("Message 1"));
    kassert!(entry2.unwrap().message().contains("Message 2"));
    kassert!(entry3.unwrap().message().contains("Message 3"));
});

test_case!(test_peek_out_of_range, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入一条日志
    test_log!(logger, LogLevel::Info, "Test");

    let start_index = logger._log_reader_index();
    let end_index = logger._log_writer_index();

    // Peek 有效索引
    kassert!(logger._peek_log(start_index).is_some());

    // Peek 超出范围的索引
    kassert!(logger._peek_log(end_index).is_none()); // 等于 writer，无效
    kassert!(logger._peek_log(end_index + 1).is_none()); // 超出范围
    kassert!(logger._peek_log(start_index - 1).is_none()); // 小于 reader
});

test_case!(test_peek_after_read, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入三条日志
    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let start_index = logger._log_reader_index();

    // Read 第一条
    logger._read_log();

    // Peek 第一条应该失败（已被消费）
    kassert!(logger._peek_log(start_index).is_none());

    // Peek 第二条应该成功
    kassert!(logger._peek_log(start_index + 1).is_some());

    // Peek 第三条应该成功
    kassert!(logger._peek_log(start_index + 2).is_some());
});

test_case!(test_peek_index_tracking, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    let initial_reader = logger._log_reader_index();
    let initial_writer = logger._log_writer_index();

    // 初始状态：reader == writer（空）
    kassert!(initial_reader == initial_writer);

    // 写入一条
    test_log!(logger, LogLevel::Info, "Test");

    // Writer 增加，reader 不变
    kassert!(logger._log_writer_index() == initial_writer + 1);
    kassert!(logger._log_reader_index() == initial_reader);

    // Read 一条
    logger._read_log();

    // Reader 增加
    kassert!(logger._log_reader_index() == initial_reader + 1);

    // 再次相等
    kassert!(logger._log_reader_index() == logger._log_writer_index());
});

test_case!(test_peek_with_byte_counting, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入日志
    test_log!(logger, LogLevel::Info, "Test");

    let bytes_before = logger._log_unread_bytes();
    let start_index = logger._log_reader_index();

    // Peek 不应该改变字节计数
    logger._peek_log(start_index);
    kassert!(logger._log_unread_bytes() == bytes_before);

    // 再次 peek
    logger._peek_log(start_index);
    kassert!(logger._log_unread_bytes() == bytes_before);

    // Read 才会改变字节计数
    logger._read_log();
    kassert!(logger._log_unread_bytes() == 0);
});

test_case!(test_peek_empty_buffer, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    let start_index = logger._log_reader_index();

    // 空缓冲区，peek 应该返回 None
    kassert!(logger._peek_log(start_index).is_none());
    kassert!(logger._peek_log(start_index + 1).is_none());
});

test_case!(test_peek_sequential_access, {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    // 写入多条
    for i in 0..5 {
        test_log!(logger, LogLevel::Info, "Message {}", i);
    }

    let start_index = logger._log_reader_index();

    // 顺序 peek 所有条目
    for i in 0..5 {
        let entry = logger._peek_log(start_index + i);
        kassert!(entry.is_some());
    }

    // 越界
    kassert!(logger._peek_log(start_index + 5).is_none());

    // 读指针未移动
    kassert!(logger._log_reader_index() == start_index);
});
