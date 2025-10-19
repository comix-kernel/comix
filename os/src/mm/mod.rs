//! Memory management module
//!
//! This module provides architecture-independent memory management abstractions
//! and implementations for the kernel.
//!
//! # Components
//!
//! - [`address`]: Address and page number abstractions
//! - [`frame_allocator`]: Physical frame allocation

mod address;
mod frame_allocator;