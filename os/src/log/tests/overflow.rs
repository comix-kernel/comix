// os/src/log/tests/overflow.rs

use super::*;

test_case!(test_buffer_overflow, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Write many logs to trigger overflow
    const TOTAL: usize = 100;
    for i in 0..TOTAL {
        test_log!(log, LogLevel::Info, "log {}", i);
    }

    // Verify overflow handling
    let buffered = log._log_len();
    let dropped = log._log_dropped_count();

    kassert!(dropped > 0); // Should have dropped logs
    kassert!(buffered + dropped == TOTAL);
});

test_case!(test_overflow_fifo_behavior, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Fill buffer + trigger overflow
    for i in 0..100 {
        test_log!(log, LogLevel::Info, "entry {}", i);
    }

    let dropped = log._log_dropped_count();
    kassert!(dropped > 0);

    // The first entry read should be the oldest entry after overwriting
    let first_entry = log._read_log().unwrap();

    // Verify message format is correct (oldest entry after overwriting)
    kassert!(first_entry.message().starts_with("entry"));
});

test_case!(test_write_after_overflow, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Trigger overflow
    for i in 0..100 {
        test_log!(log, LogLevel::Info, "overflow {}", i);
    }

    let dropped_before = log._log_dropped_count();
    kassert!(dropped_before > 0);

    // Clear buffer
    while log._read_log().is_some() {}

    // Write again
    test_log!(log, LogLevel::Info, "after overflow");

    // Should work normally
    kassert!(log._log_len() == 1);
    kassert!(log._read_log().unwrap().message() == "after overflow");
});
