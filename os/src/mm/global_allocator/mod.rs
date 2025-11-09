// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
//! 全局分配器模块
//!
//! 本模块使用 **talc** 分配器提供动态堆内存分配功能。
//!
//! # 模块组成
//!
//! - [`init_heap`]：初始化全局堆分配器。

mod global_allocator;
mod heap;

pub use global_allocator::init_heap;
