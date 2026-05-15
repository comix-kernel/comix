//! 全局分配器模块
//!
//! 本模块使用 **talc** 分配器提供动态堆内存分配功能。
//!
//! # 模块组成
//!
//! - [`init_heap`]：初始化全局堆分配器。

mod heap;
#[cfg(feature = "alloc")]
mod talc_alloc;

#[cfg(feature = "alloc")]
pub use talc_alloc::init_heap;
