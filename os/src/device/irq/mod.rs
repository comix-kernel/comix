//! 中断管理模块
//!
//! 包含中断管理器和中断控制器驱动接口的定义

use alloc::{
    collections::btree_map::{BTreeMap, Entry},
    sync::Arc,
    vec::Vec,
};

pub mod plic;

use crate::{arch::intr::enable_irq, device::Driver};

/// 中断管理器结构体
pub struct IrqManager {
    /// 是否为根中断管理器
    root: bool,
    /// 中断号到驱动程序列表的映射
    /// 每个中断号可以对应多个驱动程序
    mapping: BTreeMap<usize, Vec<Arc<dyn Driver>>>,
    /// 全局驱动程序列表
    /// 这些驱动程序会处理所有中断
    all: Vec<Arc<dyn Driver>>,
}

impl IrqManager {
    /// 创建一个新的中断管理器
    /// # 参数：
    /// * `root` - 是否为根中断管理器
    /// # 返回值：
    /// 新创建的中断管理器实例
    pub fn new(root: bool) -> IrqManager {
        IrqManager {
            root,
            mapping: BTreeMap::new(),
            all: Vec::new(),
        }
    }

    /// 注册中断号与驱动程序的映射
    /// # 参数：
    /// * `irq` - 中断号
    /// * `driver` - 要注册的驱动程序
    pub fn register_irq(&mut self, irq: usize, driver: Arc<dyn Driver>) {
        // 对于根中断管理器，在架构层启用中断
        // 对于其他中断控制器，在调用此函数之前启用中断
        if self.root {
            enable_irq(irq);
        }
        match self.mapping.entry(irq) {
            Entry::Occupied(mut e) => {
                e.get_mut().push(driver);
            }
            Entry::Vacant(e) => {
                let mut v = Vec::new();
                v.push(driver);
                e.insert(v);
            }
        }
    }

    /// 注册全局驱动程序
    /// # 参数：
    /// * `driver` - 要注册的驱动程序
    pub fn register_all(&mut self, driver: Arc<dyn Driver>) {
        self.all.push(driver);
    }

    /// 注册可选的中断号与驱动程序的映射
    /// # 参数：
    /// * `irq_opt` - 可选的中断号
    /// * `driver` - 要注册的驱动程序
    pub fn register_opt(&mut self, irq_opt: Option<usize>, driver: Arc<dyn Driver>) {
        if let Some(irq) = irq_opt {
            self.register_irq(irq, driver);
        } else {
            self.register_all(driver);
        }
    }

    /// 注销中断号与驱动程序的映射
    /// # 参数：
    /// * `irq` - 要注销的中断号
    /// * `driver` - 要注销的驱动程序
    pub fn deregister_irq(&mut self, irq: usize, driver: Arc<dyn Driver>) {
        if let Some(e) = self.mapping.get_mut(&irq) {
            e.retain(|d| !Arc::ptr_eq(d, &driver));
        }
    }

    /// 注销全局驱动程序
    /// # 参数：
    /// * `driver` - 要注销的驱动程序
    pub fn deregister_all(&mut self, driver: Arc<dyn Driver>) {
        self.all.retain(|d| !Arc::ptr_eq(d, &driver));
    }

    /// 处理中断
    /// # 参数：
    /// * `irq_opt` - 可选的中断号
    /// # 返回值：
    /// 如果中断被处理则返回 true，否则返回 false
    pub fn try_handle_interrupt(&self, irq_opt: Option<usize>) -> bool {
        if let Some(irq) = irq_opt {
            if let Some(e) = self.mapping.get(&irq) {
                for dri in e.iter() {
                    if dri.try_handle_interrupt(Some(irq)) {
                        return true;
                    }
                }
            }
        }

        for dri in self.all.iter() {
            if dri.try_handle_interrupt(irq_opt) {
                return true;
            }
        }
        false
    }
}

/// 中断控制器驱动接口
pub trait IntcDriver: Driver {
    /// 注册本地中断处理程序
    /// # 参数：
    /// * `irq` - 中断号
    /// * `driver` - 要注册的驱动程序
    fn register_local_irq(&self, irq: usize, driver: Arc<dyn Driver>);
}
