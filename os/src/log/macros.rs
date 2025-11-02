// pub enum LogLevel {
//     Emergency = 0, // KERN_EMERG
//     Alert = 1,     // KERN_ALERT
//     Critical = 2,  // KERN_CRIT
//     Error = 3,     // KERN_ERR
//     Warning = 4,   // KERN_WARNING
//     Notice = 5,    // KERN_NOTICE
//     Info = 6,      // KERN_INFO
//     Debug = 7,     // KERN_DEBUG
// }

/// Logs a message at the EMERGENCY level.
#[macro_export]
macro_rules! pr_emerg {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Emergency,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the ALERT level.
#[macro_export]
macro_rules! pr_alert {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Alert,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the CRITICAL level.
#[macro_export]
macro_rules! pr_crit {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Critical,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the ERROR level.
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Error,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the WARNING level.
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Warning,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the NOTICE level.
#[macro_export]
macro_rules! pr_notice {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Notice,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the INFO level.
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Info,
            format_args!($($arg)*)
        )
    }
}

/// Logs a message at the DEBUG level.
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        $crate::log::log_impl(
            $crate::log::LogLevel::Debug,
            format_args!($($arg)*)
        )
    }
}