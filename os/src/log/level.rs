#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Emergency = 0, // KERN_EMERG
    Alert = 1,     // KERN_ALERT
    Critical = 2,  // KERN_CRIT
    Error = 3,     // KERN_ERR
    Warning = 4,   // KERN_WARNING
    Notice = 5,    // KERN_NOTICE
    Info = 6,      // KERN_INFO
    Debug = 7,     // KERN_DEBUG
}

impl LogLevel {
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Emergency => "[EMERG]",
            LogLevel::Alert => "[ALERT]",
            LogLevel::Critical => "[CRIT]",
            LogLevel::Error => "[ERR]",
            LogLevel::Warning => "[WARNING]",
            LogLevel::Notice => "[NOTICE]",
            LogLevel::Info => "[INFO]",
            LogLevel::Debug => "[DEBUG]",
        }
    }

    pub(super) const fn color_code(&self) -> &'static str {
        match self {
            Self::Emergency | Self::Alert | Self::Critical => "\x1b[1;31m",
            Self::Error => "\x1b[31m",
            Self::Warning => "\x1b[33m",
            Self::Notice => "\x1b[1;37m",
            Self::Info => "\x1b[37m",
            Self::Debug => "\x1b[90m",
        }
    }

    pub(super) const fn reset_color_code(&self) -> &'static str {
        "\x1b[0m"
    }

    pub(super) fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::Emergency,
            1 => Self::Alert,
            2 => Self::Critical,
            3 => Self::Error,
            4 => Self::Warning,
            5 => Self::Notice,
            6 => Self::Info,
            7 => Self::Debug,

            _ => super::config::DEFAULT_LOG_LEVEL,
        }
    }
}
