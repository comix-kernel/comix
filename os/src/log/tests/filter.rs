// os/src/log/tests/filter.rs

use super::*;

test_case!(test_global_level_filtering, {
    let log = LogCore::new(LogLevel::Warning, LogLevel::Warning);

    // Write logs at different levels
    test_log!(log, LogLevel::Emergency, "emergency"); // 0 <= 4, buffered
    test_log!(log, LogLevel::Error, "error"); // 3 <= 4, buffered
    test_log!(log, LogLevel::Warning, "warning"); // 4 <= 4, buffered
    test_log!(log, LogLevel::Info, "info"); // 6 > 4, filtered
    test_log!(log, LogLevel::Debug, "debug"); // 7 > 4, filtered

    // Verify that only 3 logs are buffered
    kassert!(log._log_len() == 3);

    kassert!(log._read_log().unwrap().message() == "emergency");
    kassert!(log._read_log().unwrap().message() == "error");
    kassert!(log._read_log().unwrap().message() == "warning");
    kassert!(log._log_len() == 0);
});

test_case!(test_level_boundary, {
    let log = LogCore::new(LogLevel::Info, LogLevel::Warning);

    // Boundary test: Info (6) == 6
    test_log!(log, LogLevel::Info, "boundary");
    kassert!(log._log_len() == 1);

    // Debug (7) > 6, filtered
    test_log!(log, LogLevel::Debug, "filtered");
    kassert!(log._log_len() == 1); // Still 1

    kassert!(log._read_log().unwrap().message() == "boundary");
});

test_case!(test_dynamic_level_change, {
    let log = LogCore::new(LogLevel::Info, LogLevel::Warning);

    test_log!(log, LogLevel::Debug, "debug1"); // Filtered
    test_log!(log, LogLevel::Info, "info1"); // Buffered

    kassert!(log._log_len() == 1);

    // Switch to Debug
    log._set_global_level(LogLevel::Debug);

    test_log!(log, LogLevel::Debug, "debug2"); // Now buffered
    test_log!(log, LogLevel::Info, "info2"); // Buffered

    kassert!(log._log_len() == 3);

    kassert!(log._read_log().unwrap().message() == "info1");
    kassert!(log._read_log().unwrap().message() == "debug2");
    kassert!(log._read_log().unwrap().message() == "info2");
});

test_case!(test_all_levels, {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Write all levels
    test_log!(log, LogLevel::Emergency, "emerg");
    test_log!(log, LogLevel::Alert, "alert");
    test_log!(log, LogLevel::Critical, "crit");
    test_log!(log, LogLevel::Error, "err");
    test_log!(log, LogLevel::Warning, "warn");
    test_log!(log, LogLevel::Notice, "notice");
    test_log!(log, LogLevel::Info, "info");
    test_log!(log, LogLevel::Debug, "debug");

    kassert!(log._log_len() == 8);

    let levels = [
        "emerg", "alert", "crit", "err", "warn", "notice", "info", "debug",
    ];
    for expected in &levels {
        let entry = log._read_log().unwrap();
        kassert!(entry.message() == *expected);
    }
});
