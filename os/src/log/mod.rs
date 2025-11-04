pub mod buffer;
pub mod config;
pub mod context;
pub mod entry;
pub mod level;
pub mod macros;

pub use entry::LogEntry;
pub use level::LogLevel;

/// Implementation of the log function (for temporary use)
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    use crate::console::Stdout;
    use core::fmt::Write;

    let mut stdout = Stdout;
    let _ = write!(stdout, "{}{} ", level.color_code(), level.as_str());
    let _ = stdout.write_fmt(args);
    let _ = write!(stdout, "{}", level.reset_color_code());
    let _ = writeln!(stdout);
}
