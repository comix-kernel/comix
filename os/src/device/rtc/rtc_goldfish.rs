use alloc::{string::String, sync::Arc};
use fdt::node::FdtNode;

use crate::{
    device::{
        DRIVERS, DeviceType, Driver, RTC_DRIVERS, device_tree::DEVICE_TREE_REGISTRY, rtc::RtcDriver,
    },
    kernel::current_memory_space,
    mm::address::{Paddr, UsizeConvert},
    pr_info, pr_warn,
    util::read,
};

const TIMER_TIME_LOW: usize = 0x00;
const TIMER_TIME_HIGH: usize = 0x04;

pub struct RtcGoldfish {
    base: usize,
}

impl Driver for RtcGoldfish {
    fn try_handle_interrupt(&self, irq: Option<usize>) -> bool {
        false
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Rtc
    }

    fn get_id(&self) -> String {
        String::from("rtc_goldfish")
    }

    fn as_rtc(&self) -> Option<&dyn RtcDriver> {
        Some(self)
    }
}

impl RtcDriver for RtcGoldfish {
    // read seconds since 1970-01-01
    fn read_epoch(&self) -> u64 {
        let low: u32 = read(self.base + TIMER_TIME_LOW);
        let high: u32 = read(self.base + TIMER_TIME_HIGH);
        let ns = ((high as u64) << 32) | (low as u64);
        ns / 1_000_000_000u64
    }
}

fn init_dt(dt: &FdtNode) {
    let reg = dt
        .reg()
        .and_then(|mut reg| reg.next())
        .expect("No reg property found for goldfish-rtc");
    let paddr = reg.starting_address as usize;
    let size = reg.size.unwrap_or(0);
    if size == 0 {
        pr_warn!(
            "[Device] goldfish-rtc device tree node {} has no size",
            dt.name
        );
        return;
    }
    let vaddr = current_memory_space()
        .lock()
        .map_mmio(Paddr::from_usize(paddr), size)
        .ok()
        .expect("Failed to map MMIO region for goldfish-rtc");
    let rtc = Arc::new(RtcGoldfish {
        base: vaddr.as_usize(),
    });
    DRIVERS.write().push(rtc.clone());
    RTC_DRIVERS.write().push(rtc);
    pr_info!("[Device] RTC Goldfish initialized");
}

pub fn driver_init() {
    DEVICE_TREE_REGISTRY
        .write()
        .insert("google,goldfish-rtc", init_dt);
}
