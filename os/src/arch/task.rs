//! Architecture-neutral task setup data.

/// Result of laying out a new userspace stack for `execve`.
#[derive(Debug, Clone, Copy)]
pub struct ExecStackLayout {
    /// Initial userspace stack pointer.
    pub sp: usize,
    /// Argument count.
    pub argc: usize,
    /// Userspace address of the argv vector.
    pub argv: usize,
    /// Userspace address of the envp vector.
    pub envp: usize,
    /// Architecture-specific thread pointer/TLS value.
    pub tls: usize,
}
