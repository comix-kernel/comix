use alloc::{string::String, sync::Arc};
use fdt::node::FdtNode;

use crate::{
    device::{
        DRIVERS, DeviceType, Driver, RTC_DRIVERS, device_tree::DEVICE_TREE_REGISTRY, rtc::RtcDriver,
    },
    kernel::current_memory_space,
    mm::address::{PA, VA},
    pr_info, pr_warn,
    util::{read, write},
};

const GOLDFISH_TIME_LOW: usize = 0x00;
const GOLDFISH_TIME_HIGH: usize = 0x04;

const LS7A_TOYREAD0: usize = 0x2c;
const LS7A_TOYREAD1: usize = 0x30;
const LS7A_RTCCTRL: usize = 0x40;
const LS7A_RTCCTRL_RTCEN: u32 = 1 << 13;
const LS7A_RTCCTRL_TOYEN: u32 = 1 << 11;
const LS7A_RTCCTRL_EO: u32 = 1 << 8;

#[derive(Debug, Clone, Copy)]
enum RtcBackend {
    Goldfish,
    Ls7a,
}

pub struct RtcGoldfish {
    base: VA,
    backend: RtcBackend,
}

impl Driver for RtcGoldfish {
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        false
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Rtc
    }

    fn get_id(&self) -> String {
        match self.backend {
            RtcBackend::Goldfish => String::from("rtc_goldfish"),
            RtcBackend::Ls7a => String::from("rtc_ls7a"),
        }
    }

    fn as_rtc(&self) -> Option<&dyn RtcDriver> {
        Some(self)
    }
}

impl RtcDriver for RtcGoldfish {
    // read seconds since 1970-01-01
    fn read_epoch(&self) -> u64 {
        match self.backend {
            RtcBackend::Goldfish => self.read_goldfish_epoch(),
            RtcBackend::Ls7a => self.read_ls7a_epoch(),
        }
    }
}

impl RtcGoldfish {
    fn read_goldfish_epoch(&self) -> u64 {
        let base = self.base.as_usize();
        let low: u32 = read(base + GOLDFISH_TIME_LOW);
        let high: u32 = read(base + GOLDFISH_TIME_HIGH);
        let ns = ((high as u64) << 32) | (low as u64);
        ns / 1_000_000_000u64
    }

    fn read_ls7a_epoch(&self) -> u64 {
        let base = self.base.as_usize();

        let mut year: u32 = read(base + LS7A_TOYREAD1);
        let toy0: u32 = read(base + LS7A_TOYREAD0);
        let year_after: u32 = read(base + LS7A_TOYREAD1);
        let toy0 = if year == year_after {
            toy0
        } else {
            year = year_after;
            read(base + LS7A_TOYREAD0)
        };

        let month = (toy0 >> 26) & 0x3f;
        let day = (toy0 >> 21) & 0x1f;
        let hour = (toy0 >> 16) & 0x1f;
        let minute = (toy0 >> 10) & 0x3f;
        let second = (toy0 >> 4) & 0x3f;

        utc_to_epoch(1900 + year as i32, month, day, hour, minute, second).unwrap_or(0)
    }
}

fn init_dt(dt: &FdtNode, backend: RtcBackend) {
    let reg = dt
        .reg()
        .and_then(|mut reg| reg.next())
        .expect("No reg property found for RTC");
    let paddr = reg.starting_address as usize;
    let size = reg.size.unwrap_or(0);
    if size == 0 {
        pr_warn!("[Device] RTC device tree node {} has no size", dt.name);
        return;
    }
    let vaddr = current_memory_space()
        .lock()
        .map_mmio(PA::from_usize(paddr), size)
        .ok()
        .expect("Failed to map MMIO region for RTC");
    if matches!(backend, RtcBackend::Ls7a) {
        let ctrl: u32 = read(vaddr.as_usize() + LS7A_RTCCTRL);
        write(
            vaddr.as_usize() + LS7A_RTCCTRL,
            ctrl | LS7A_RTCCTRL_EO | LS7A_RTCCTRL_TOYEN | LS7A_RTCCTRL_RTCEN,
        );
    }

    let rtc = Arc::new(RtcGoldfish {
        base: vaddr,
        backend,
    });
    DRIVERS.write().push(rtc.clone());
    RTC_DRIVERS.write().push(rtc);
    pr_info!("[Device] RTC {:?} initialized", backend);
}

fn init_goldfish_dt(dt: &FdtNode) {
    init_dt(dt, RtcBackend::Goldfish);
}

fn init_ls7a_dt(dt: &FdtNode) {
    init_dt(dt, RtcBackend::Ls7a);
}

fn utc_to_epoch(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<u64> {
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
        || year < 1970
    {
        return None;
    }

    let mut days = 0u64;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    for m in 1..month {
        days += days_in_month(year, m)? as u64;
    }

    let dim = days_in_month(year, month)?;
    if day > dim {
        return None;
    }

    days += (day - 1) as u64;
    Some(days * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second.min(59) as u64)
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub fn driver_init() {
    DEVICE_TREE_REGISTRY
        .lock()
        .insert("google,goldfish-rtc", init_goldfish_dt);
    DEVICE_TREE_REGISTRY
        .lock()
        .insert("loongson,ls7a-rtc", init_ls7a_dt);
}
