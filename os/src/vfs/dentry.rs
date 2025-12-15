//! 目录项（Dentry）与全局缓存
//!
//! 该模块实现了 VFS 路径层的核心组件，提供目录树结构管理和路径到 Inode 的映射缓存。
//!
//! # 组件
//!
//! - [`Dentry`] - 目录项结构，表示路径中的一个节点
//! - [`DentryCache`] - 全局路径缓存，加速重复路径查找
//! - [`DENTRY_CACHE`] - 全局缓存单例
//!
//! # 核心概念
//!
//! ## Dentry 的作用
//!
//! Dentry (Directory Entry) 是文件系统目录树中的一个节点，它：
//! - 缓存文件名到 Inode 的映射
//! - 维护父子关系，构成目录树
//! - 标记挂载点信息
//!
//! ## 与 Inode 的关系
//!
//! - **Dentry**: 路径层组件，可能有多个 Dentry 指向同一个 Inode (硬链接)
//! - **Inode**: 存储层组件，代表实际的文件或目录
//!
//! ```text
//! /home/user/file.txt ──┐
//!                       ├──> Dentry ──> Inode (文件数据)
//! /tmp/link_to_file  ───┘
//! ```
//!
//! # 引用计数设计
//!
//! Dentry 使用 `Arc` 和 `Weak` 管理生命周期，避免循环引用：
//!
//! - **parent**: `Weak<Dentry>` - 弱引用父节点，避免循环
//! - **children**: `Arc<Dentry>` - 强引用子节点
//! - **全局缓存**: `Weak<Dentry>` - 不延长生命周期
//!
//! ```text
//! Arc<Dentry("/")>
//!   └─> Arc<Dentry("/etc")> { parent: Weak<Dentry("/">") }
//!         └─> Arc<Dentry("/etc/passwd")> { parent: Weak<Dentry("/etc")> }
//! ```
//!
//! # 缓存机制
//!
//! ## 全局缓存 (DENTRY_CACHE)
//!
//! 维护路径字符串到 Dentry 的映射：
//!
//! - **插入**: 路径解析成功后自动插入
//! - **查找**: O(log n) BTreeMap 查找
//! - **失效**: Weak 引用自动失效，无需手动清理
//!
//! ## 树状缓存
//!
//! Dentry 内部维护子项缓存，加速相对路径查找：
//!
//! ```rust
//! let parent = vfs_lookup("/etc")?;
//! // 快速查找，不需要再次访问 Inode
//! if let Some(child) = parent.lookup_child("passwd") {
//!     // 缓存命中
//! }
//! ```
//!
//! # 使用示例
//!
//! ## 创建 Dentry
//!
//! ```rust
//! use vfs::{Dentry, Inode};
//!
//! let inode = create_inode()?;
//! let dentry = Dentry::new(String::from("file.txt"), inode);
//! ```
//!
//! ## 管理父子关系
//!
//! ```rust
//! // 添加子项
//! parent.add_child(child_dentry.clone());
//!
//! // 查找子项
//! if let Some(child) = parent.lookup_child("file.txt") {
//!     println!("找到: {}", child.name);
//! }
//!
//! // 删除子项
//! parent.remove_child("file.txt");
//! ```
//!
//! ## 使用全局缓存
//!
//! ```rust
//! use vfs::DENTRY_CACHE;
//!
//! // 插入缓存
//! DENTRY_CACHE.insert(&dentry);
//!
//! // 查找缓存
//! if let Some(cached) = DENTRY_CACHE.lookup("/etc/passwd") {
//!     // 缓存命中，避免重复路径解析
//! }
//!
//! // 删除缓存
//! DENTRY_CACHE.remove("/etc/passwd");
//! ```

use crate::sync::SpinLock;
use crate::vfs::inode::Inode;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use core::fmt;

/// 目录项（Dentry）
///
/// 表示路径中的一个组件，缓存文件名到 inode 的映射
pub struct Dentry {
    /// 文件名（不含路径）
    pub name: String,

    /// 关联的 inode
    pub inode: Arc<dyn Inode>,

    /// 父目录 dentry（弱引用避免循环）
    parent: SpinLock<Weak<Dentry>>,

    /// 子 dentry 映射（文件名 -> dentry）
    children: SpinLock<BTreeMap<String, Arc<Dentry>>>,

    /// 如果此 dentry 是挂载点，指向挂载的根 dentry
    mount_point: SpinLock<Option<Weak<Dentry>>>,
}

impl fmt::Debug for Dentry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parent_name = self.parent().map(|p| p.name.clone());
        let child_names = {
            let children = self.children.lock();
            children.keys().cloned().collect::<alloc::vec::Vec<_>>()
        };

        f.debug_struct("Dentry")
            .field("name", &self.name)
            .field("parent", &parent_name)
            .field("children", &child_names)
            .finish()
    }
}

impl Dentry {
    /// 创建新的 dentry
    pub fn new(name: String, inode: Arc<dyn Inode>) -> Arc<Self> {
        let dentry = Arc::new(Self {
            name,
            inode,
            parent: SpinLock::new(Weak::new()),
            children: SpinLock::new(BTreeMap::new()),
            mount_point: SpinLock::new(None),
        });

        dentry.inode.set_dentry(Arc::downgrade(&dentry));

        dentry
    }

    /// 设置父 dentry
    pub fn set_parent(&self, parent: &Arc<Dentry>) {
        *self.parent.lock() = Arc::downgrade(parent);
    }

    /// 获取父 dentry
    pub fn parent(&self) -> Option<Arc<Dentry>> {
        self.parent.lock().upgrade()
    }

    /// 查找子 dentry
    pub fn lookup_child(&self, name: &str) -> Option<Arc<Dentry>> {
        self.children.lock().get(name).cloned()
    }

    /// 添加子 dentry
    pub fn add_child(self: &Arc<Self>, child: Arc<Dentry>) {
        child.set_parent(self);
        self.children.lock().insert(child.name.clone(), child);
    }

    /// 删除子 dentry
    pub fn remove_child(&self, name: &str) -> Option<Arc<Dentry>> {
        self.children.lock().remove(name)
    }

    /// 获取完整路径（通过向上遍历父节点直到根目录）
    pub fn full_path(&self) -> String {
        let mut components = alloc::vec::Vec::new();
        let mut current: *const Dentry = self;

        // 向上遍历到根目录
        loop {
            let dentry = unsafe { &*current };

            // 根目录的名字是 "/"
            if dentry.name == "/" {
                break;
            }

            components.push(dentry.name.clone());

            // 获取父节点
            match dentry.parent() {
                Some(parent) => current = Arc::as_ptr(&parent),
                None => break, // 到达根或孤立节点
            }
        }

        // 反转（从根到当前）
        components.reverse();

        if components.is_empty() {
            String::from("/")
        } else {
            String::from("/") + &components.join("/")
        }
    }

    /// 设置挂载点
    pub fn set_mount(&self, mounted_root: &Arc<Dentry>) {
        *self.mount_point.lock() = Some(Arc::downgrade(mounted_root));
    }

    /// 清除挂载点
    pub fn clear_mount(&self) {
        *self.mount_point.lock() = None;
    }

    /// 获取挂载的根 dentry（如果有）
    pub fn get_mount(&self) -> Option<Arc<Dentry>> {
        self.mount_point.lock().as_ref()?.upgrade()
    }
}

// 全局 dentry 缓存实例
lazy_static::lazy_static! {
    pub static ref DENTRY_CACHE: DentryCache = DentryCache::new();
}

/// 全局 Dentry 缓存
pub struct DentryCache {
    /// 路径 -> dentry 的弱引用映射
    cache: SpinLock<BTreeMap<String, Weak<Dentry>>>,
}

impl DentryCache {
    /// 创建新的缓存
    pub const fn new() -> Self {
        Self {
            cache: SpinLock::new(BTreeMap::new()),
        }
    }

    /// 从缓存中查找 dentry
    pub fn lookup(&self, path: &str) -> Option<Arc<Dentry>> {
        let cache = self.cache.lock();
        let weak = cache.get(path)?;
        weak.upgrade()
    }

    /// 插入 dentry 到缓存
    pub fn insert(&self, dentry: &Arc<Dentry>) {
        let path = dentry.full_path();
        self.cache.lock().insert(path, Arc::downgrade(dentry));
    }

    /// 从缓存中移除
    pub fn remove(&self, path: &str) {
        self.cache.lock().remove(path);
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.cache.lock().clear();
    }
}
