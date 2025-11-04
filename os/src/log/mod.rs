mod buffer;
mod config;
mod context;
mod entry;
mod filter;
mod level;
pub mod macros;

pub use entry::LogEntry;
pub use level::LogLevel;

// Re-export public APIs for reading logs
pub use buffer::{log_dropped_count, log_len, read_log};

// Re-export public APIs for configuring log levels
pub use filter::{get_console_level, get_global_level, set_console_level, set_global_level};

// Re-export for internal use by macros (hidden from docs)
#[doc(hidden)]
pub use filter::is_level_enabled;

/// Implementation of the log function (for internal use by macros)
#[doc(hidden)]
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    let log_context = context::collect_context();
    let (cpu_id, task_id, timestamp) = (
        log_context.cpu_id,
        log_context.task_id,
        log_context.timestamp,
    );
    let entry = LogEntry::from_args(level, cpu_id, task_id, timestamp, args);

    if filter::is_console_level(level) {
        direct_print_entry(&entry);
    }

    buffer::write_log(&entry);
}

fn direct_print_entry(entry: &LogEntry) {
    // Important!: must lock console here to prevent:
    // garbled (interleaved) output from concurrent calls.
    //
    // This function is the single "choke point" for all physical
    // console I/O and can be called concurrently from 2 different sources:
    //
    // 1. **Urgent Logs:** Multiple CPUs hitting high-priority logs (e.g., `pr_err!`).
    // 2. **Async Consumer:** The `console_flush_thread` printing buffered logs.
    //
    // A global `CONSOLE_LOCK` (SpinLock) must be acquired before these
    // `write!` operations to serialize all access to the (e.g.) UART hardware.
    //
    // let _guard = CONSOLE_LOCK.lock(); // <-- lock here

    use crate::console::Stdout;
    use core::fmt::Write;

    let mut stdout = Stdout;
    let _ = write!(
        stdout,
        "{}{} ",
        entry.level().color_code(),
        entry.level().as_str()
    );
    let _ = stdout.write_str(entry.message());
    let _ = write!(stdout, "{}", entry.level().reset_color_code());
    let _ = writeln!(stdout);
}
