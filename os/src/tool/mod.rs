//! 工具函数模块
#![allow(dead_code)]
pub mod ring_buffer;
mod str;
pub mod user_buffer;

pub use str::*;

pub fn if_null<T>(ptr: *const T) -> bool {
    ptr.is_null()
}