// os/src/log/tests/format.rs

use super::*;

test_case!(test_message_truncation, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Create a long message (>256 bytes)
    let long_msg = alloc::format!("{}", "a".repeat(300));
    test_log!(log, LogLevel::Info, "{}", long_msg);

    let entry = log._read_log().unwrap();

    // Should be truncated to 256 bytes
    kassert!(entry.message().len() <= 256);
});

test_case!(test_empty_message, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "");

    let entry = log._read_log().unwrap();
    kassert!(entry.message() == "");
});

test_case!(test_special_characters, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "special: !@#$%^&*()");

    let entry = log._read_log().unwrap();
    kassert!(entry.message() == "special: !@#$%^&*()");
});

test_case!(test_utf8_message, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "你好，世界！");
    test_log!(log, LogLevel::Info, "Hello, мир!");

    let e1 = log._read_log().unwrap();
    kassert!(e1.message() == "你好，世界！");

    let e2 = log._read_log().unwrap();
    kassert!(e2.message() == "Hello, мир!");
});
