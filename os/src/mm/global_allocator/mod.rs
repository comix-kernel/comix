//! Global allocator module
//!
//! This module provides dynamic heap memory allocation functionality using the talc allocator.
//!
//! # Components
//!
//! - [`init_heap`]: Initialize the global heap allocator

mod global_allocator;

pub use global_allocator::init_heap;
