// os/src/log/tests/basic.rs

use super::*;

test_case!(test_write_and_read, {
    // Create an independent LogCore instance with Debug level enabled
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Write log
    test_log!(log, LogLevel::Info, "test message");

    // Verify
    kassert!(log._log_len() == 1);

    let entry = log._read_log().unwrap();
    kassert!(entry.message() == "test message");
    kassert!(entry.level() == LogLevel::Info);

    // Buffer should be empty
    kassert!(log._log_len() == 0);
});

test_case!(test_format_arguments, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Test formatting
    test_log!(log, LogLevel::Info, "value: {}", 42);
    test_log!(log, LogLevel::Debug, "hex: {:#x}", 0xDEAD);

    let e1 = log._read_log().unwrap();
    kassert!(e1.message() == "value: 42");

    let e2 = log._read_log().unwrap();
    kassert!(e2.message() == "hex: 0xdead");
});

test_case!(test_fifo_order, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Write multiple logs
    for i in 0..5 {
        test_log!(log, LogLevel::Debug, "message {}", i);
    }

    kassert!(log._log_len() == 5);

    // Read in FIFO order
    for i in 0..5 {
        let entry = log._read_log().unwrap();
        // Check that the message contains the corresponding number
        let expected_msg = alloc::format!("message {}", i);
        kassert!(entry.message() == expected_msg.as_str());
    }

    kassert!(log._log_len() == 0);
});

test_case!(test_empty_buffer_read, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Empty buffer
    kassert!(log._log_len() == 0);
    kassert!(log._read_log().is_none());

    // Read empty buffer multiple times
    kassert!(log._read_log().is_none());
    kassert!(log._read_log().is_none());
});
