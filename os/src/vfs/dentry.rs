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
