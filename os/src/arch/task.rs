//! Architecture-neutral task setup data.

use crate::arch::address::VA;

/// Result of laying out a new userspace stack for `execve`.
#[derive(Debug, Clone, Copy)]
pub struct ExecStackLayout {
    /// Initial userspace stack pointer.
    pub sp: VA,
    /// Argument count.
    pub argc: usize,
    /// Userspace address of the argv vector.
    pub argv: VA,
    /// Userspace address of the envp vector.
    pub envp: VA,
    /// Architecture-specific thread pointer/TLS value.
    pub tls: VA,
}
