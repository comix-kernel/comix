//! Platform Level Interrupt Controller (PLIC) 驱动实现
//!
//! PLIC 提供对外设中断的集中管理，支持优先级和中断分发功能。

use super::{super::DRIVERS, IrqManager};
use crate::arch::constant::SUPERVISOR_EXTERNAL;
use crate::device::device_tree::{DEVICE_TREE_INTC, DEVICE_TREE_REGISTRY};
use crate::device::irq::IntcDriver;
use crate::device::{DeviceType, Driver, IRQ_MANAGER};
use crate::kernel::current_memory_space;
use crate::mm::address::{Paddr, UsizeConvert};
use crate::{pr_info, pr_warn};
use crate::{sync::SpinLock as Mutex, util::read, util::write};
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use fdt::node::FdtNode;

/// Platform Level Interrupt Controller (PLIC) 结构体
pub struct Plic {
    base: usize,
    manager: Mutex<IrqManager>,
}

impl Driver for Plic {
    /// 处理中断
    /// # 参数：
    /// * `irq` - 可选的中断号
    /// # 返回值
    /// 如果中断被处理则返回 true，否则返回 false
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        let pending: u32 = read(self.base + 0x1000);
        if pending != 0 {
            let claim: u32 = read(self.base + 0x201004);
            let manager = self.manager.lock();
            let res = manager.try_handle_interrupt(Some(claim as usize));
            write(self.base + 0x201004, claim);
            res
        } else {
            false
        }
    }

    /// 返回设备类型
    /// # 返回值
    /// 设备类型枚举值
    fn device_type(&self) -> DeviceType {
        DeviceType::Intc
    }

    /// 获取设备唯一标识符
    /// # 返回值
    /// 设备标识符字符串
    fn get_id(&self) -> String {
        format!("plic_{}", self.base)
    }
}

impl IntcDriver for Plic {
    /// 注册本地中断处理程序
    /// # 参数：
    /// * `irq` - 中断号
    /// * `driver` - 要注册的驱动程序
    fn register_local_irq(&self, irq: usize, driver: Arc<dyn Driver>) {
        write(
            self.base + 0x2080,
            read::<u32>(self.base + 0x2080) | (1 << irq),
        );
        write(self.base + irq * 4, 7);
        let mut manager = self.manager.lock();
        manager.register_irq(irq, driver);
    }
}

/// 初始化设备树中的 PLIC 中断控制器
/// # 参数：
/// * `dt` - 设备树节点
pub fn init_dt(dt: &FdtNode) {
    if let Some(reg) = dt.reg().and_then(|mut reg| reg.next()) {
        let paddr = reg.starting_address as usize;
        let size = reg.size.unwrap_or(0);
        if size == 0 {
            pr_warn!("[Device] PLIC device tree node {} has no size", dt.name);
            return;
        }
        let vaddr = current_memory_space()
            .lock()
            .map_mmio(Paddr::from_usize(paddr), size)
            .ok()
            .expect("Failed to map MMIO region");
        let phandle = dt
            .property("phandle")
            .unwrap()
            .as_usize()
            .expect("Failed to convert 'phandle' property to usize");
        let base = vaddr.as_usize();
        let plic = Arc::new(Plic {
            base,
            manager: Mutex::new(IrqManager::new(false)),
        });

        // set prio threshold to 0 for context 1
        write(base + 0x201000, 0);
        DRIVERS.write().push(plic.clone());
        // register under root irq manager
        IRQ_MANAGER
            .write()
            .register_irq(SUPERVISOR_EXTERNAL, plic.clone());
        // register interrupt controller
        DEVICE_TREE_INTC
            .write()
            .insert(phandle.try_into().unwrap(), plic);
    } else {
        pr_warn!(
            "[Device] PLIC device tree node {} has no 'reg' property",
            dt.name
        );
    }
    pr_info!(
        "[Device] PLIC initialized from device tree node {}",
        dt.name
    );
}

/// 注册 PLIC 驱动初始化函数
pub fn driver_init() {
    DEVICE_TREE_REGISTRY.write().insert("riscv,plic0", init_dt);
}
