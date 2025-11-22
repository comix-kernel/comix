use super::{super::DRIVERS, IrqManager};
use crate::device::device_tree::DEVICE_TREE_REGISTRY;
use crate::device::irq::IntcDriver;
use crate::device::{DeviceType, Driver};
use crate::println;
// use super::{super::IRQ_MANAGER, IntcDriver, IrqManager};
use crate::{sync::SpinLock as Mutex, tool::read, tool::write};
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
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool {
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
    fn device_type(&self) -> DeviceType {
        DeviceType::Intc
    }

    /// 获取设备唯一标识符
    fn get_id(&self) -> String {
        format!("plic_{}", self.base)
    }
}

impl IntcDriver for Plic {
    /// 注册本地中断处理程序
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

/// 超级用户外部中断号
pub const SUPERVISOR_EXTERNAL: usize = usize::MAX / 2 + 1 + 8;

/// 初始化设备树中的 PLIC 中断控制器
/// # 参数：
/// * `dt` - 设备树节点
pub fn init_dt(dt: &FdtNode) {
    todo!()
    // let addr = dt.prop_u64("reg").unwrap() as usize;
    // let phandle = dt.prop_u32("phandle").unwrap();
    // println!("Found riscv plic at {:#x}, {:?}", addr, dt);
    // let base = phys_to_virt(addr);
    // let plic = Arc::new(Plic {
    //     base,
    //     manager: Mutex::new(IrqManager::new(false)),
    // });
    // // set prio threshold to 0 for context 1
    // write(base + 0x201000, 0);

    // DRIVERS.write().push(plic.clone());
    // // register under root irq manager
    // IRQ_MANAGER
    //     .write()
    //     .register_irq(SUPERVISOR_EXTERNAL, plic.clone());
    // // register interrupt controller
    // DEVICE_TREE_INTC.write().insert(phandle, plic);
}

/// 注册 PLIC 驱动初始化函数
pub fn driver_init() {
    DEVICE_TREE_REGISTRY.write().insert("riscv,plic0", init_dt);
}
