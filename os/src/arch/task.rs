//! Architecture-neutral task setup data.

use crate::arch::address::VA;

/// ELF PT_TLS template for the initial thread.
#[derive(Debug, Clone, Copy)]
pub struct ExecTlsTemplate {
    /// Runtime address of the initialized TLS image (.tdata).
    pub image: VA,
    /// Number of initialized bytes to copy.
    pub filesz: usize,
    /// Total TLS image size including zero-filled .tbss.
    pub memsz: usize,
    /// Required TLS alignment from the program header.
    pub align: usize,
}

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
