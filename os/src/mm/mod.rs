//! Memory management module
//!
//! This module provides architecture-independent memory management abstractions
//! and implementations for the kernel.
//!
//! # Components
//!
//! - [`address`]: Address and page number abstractions
//! - [`frame_allocator`]: Physical frame allocation
//! - [`global_allocator`]: Global heap allocator

mod address;
mod frame_allocator;
mod global_allocator;

pub use frame_allocator::init_frame_allocator;
pub use global_allocator::init_heap;
