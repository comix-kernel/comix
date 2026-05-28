//! RISC-V 架构相关的启动代码

use core::arch::global_asm;
use core::sync::atomic::{AtomicUsize, Ordering};

global_asm!(include_str!("entry.S"));

use crate::mm::address::UsizeConvert;
use crate::{
    arch::{timer, trap},
    kernel::{self, current_cpu, num_cpu, set_num_cpu},
    pr_debug, pr_err, pr_info, pr_warn,
    sync::PreemptGuard,
};

/// 已上线 CPU 位掩码
static CPU_ONLINE_MASK: AtomicUsize = AtomicUsize::new(0);

// 从核启动标志（在 entry.S 中定义）
unsafe extern "C" {
    static mut secondary_boot_flag: u64; // extern symbol from entry.S
}

/// 从核调试入口
#[unsafe(no_mangle)]
pub extern "C" fn secondary_debug_entry(hartid: usize) {
    crate::println!("[DEBUG] Hart {} reached secondary_wait_high", hartid);
}

/// RISC-V 主核启动入口
pub fn main(hartid: usize) {
    let mut ops = kernel::boot::PrimaryBootOps::new("RISC-V", "Hart");
    ops.before_clear_bss = setup_boot_cpu_dummy;
    ops.after_mm_init = setup_boot_cpu;
    ops.after_time_init = boot_secondaries;
    kernel::boot::run_primary_boot(hartid, ops);
}

fn setup_boot_cpu_dummy(_hartid: usize) {
    {
        static BOOT_CPU_DUMMY: usize = 0;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) &raw const BOOT_CPU_DUMMY);
        }
    }
}

fn setup_boot_cpu(_hartid: usize) {
    {
        use crate::kernel::CPUS;
        let cpu_ptr = CPUS.get_of(0) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
        crate::println!("[Boot] Initialized CPUS, tp = 0x{:x}", cpu_ptr);
    }
}

fn boot_secondaries(_hartid: usize) {
    let num_cpus = num_cpu();
    if num_cpus > 1 {
        boot_secondary_cpus(num_cpus);
    }
}

// SBI HSM 从核入口（在 entry.S 中定义）
unsafe extern "C" {
    fn secondary_sbi_entry();
}

/// 从核入口
///
/// 与主核对称：创建 idle → switch_task → trap::init → 启用中断 → idle_loop。
#[unsafe(no_mangle)]
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    trap::init_boot_trap();

    // 设置 tp 指向对应的 Cpu 结构体
    {
        use crate::kernel::CPUS;
        let cpu_ptr = CPUS.get_of(hartid) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    CPU_ONLINE_MASK.fetch_or(1 << hartid, Ordering::Release);
    pr_info!("[SMP] CPU {} is online", hartid);

    // 创建 idle 并设为当前任务
    let idle_task = kernel::boot::create_idle_task(hartid, kernel::boot::idle_loop);
    {
        let _guard = PreemptGuard::new();
        let cpu = current_cpu();
        cpu.idle_task = Some(idle_task.clone());
        cpu.switch_task(idle_task);
    }

    // 切换到全局内核页表
    if let Some(kernel_space) = crate::mm::get_global_kernel_space() {
        let root_ppn = kernel_space.lock().root_ppn();
        {
            let _guard = PreemptGuard::new();
            current_cpu().switch_space(kernel_space);
        }
        pr_info!(
            "[SMP] CPU {} switched to global kernel space, root PPN: 0x{:x}",
            hartid,
            root_ppn.as_usize()
        );
    } else {
        pr_warn!(
            "[SMP] CPU {} could not get global kernel space; still on boot_pagetable",
            hartid
        );
    }

    // 完整陷阱 + 定时器 + 中断
    trap::init();
    timer::init();
    crate::arch::enable_interrupts();

    pr_debug!("[SMP] CPU {} entering idle loop", hartid);
    kernel::boot::idle_loop();
}

/// 启动从核（由主核调用）
pub fn boot_secondary_cpus(num_cpus: usize) {
    use crate::arch::timer::{clock_freq, get_time};

    if num_cpus <= 1 {
        pr_info!("[SMP] Single CPU mode, skipping secondary boot");
        CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);
        set_num_cpu(1);
        return;
    }

    pr_info!("[SMP] Booting up to {} secondary CPUs...", num_cpus - 1);

    CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);

    let mut expected_mask: usize = 1;
    for hartid in 1..num_cpus {
        let start_vaddr = secondary_sbi_entry as usize;
        let start_paddr =
            unsafe { crate::arch::mm::va_to_pa(crate::arch::address::VA::from_usize(start_vaddr)) }
                .as_usize();
        pr_info!(
            "[SMP] Starting hart {} at vaddr=0x{:x}, paddr=0x{:x}",
            hartid,
            start_vaddr,
            start_paddr
        );

        let ret = crate::arch::lib::hart_start(hartid, start_paddr, hartid);
        if ret.error != 0 {
            pr_err!(
                "[SMP] Failed to start hart {}: SBI error {}",
                hartid,
                ret.error
            );
            continue;
        }
        expected_mask |= 1 << hartid;
        pr_info!("[SMP] Hart {} SBI call accepted", hartid);
    }

    if expected_mask == 1 {
        pr_warn!("[SMP] No secondary hart could be started; falling back to single-core");
        set_num_cpu(1);
        return;
    }

    let deadline = get_time().saturating_add(clock_freq() * 2);
    while CPU_ONLINE_MASK.load(Ordering::Acquire) != expected_mask {
        if get_time() >= deadline {
            let current_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);
            pr_warn!(
                "[SMP] Timeout waiting secondary CPUs. Expected: {:#b}, got: {:#b}",
                expected_mask,
                current_mask
            );
            break;
        }
        core::hint::spin_loop();
    }

    let online_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);
    let online_cnt = online_mask.count_ones() as usize;
    set_num_cpu(core::cmp::max(online_cnt, 1));

    if online_mask == expected_mask {
        pr_info!("[SMP] All {} CPUs are online!", num_cpu());
    } else {
        pr_warn!(
            "[SMP] Proceeding with {} online CPU(s), mask={:#b}",
            num_cpu(),
            online_mask
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    test_case!(test_num_cpu, {
        let num_cpu = crate::kernel::num_cpu();
        kassert!(num_cpu >= 1);
        kassert!(num_cpu <= crate::config::MAX_CPU_COUNT);
    });

    test_case!(test_cpu_online_mask, {
        let num_cpu = crate::kernel::num_cpu();
        let actual_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);

        if actual_mask == 0 {
            return;
        }

        let expected_mask = (1 << num_cpu) - 1;
        kassert!(actual_mask == expected_mask);
        kassert!((actual_mask & 1) != 0);

        if num_cpu > 1 {
            for hartid in 1..num_cpu {
                kassert!((actual_mask & (1 << hartid)) != 0);
            }
        }
    });
}
