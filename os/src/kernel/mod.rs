pub mod task;

mod cpu;
mod scheduler;
mod tid_allocator;

use crate::arch::kernel::context::Context;
use cpu::Cpu;

lazy_static! {
    /// 全局 CPU 实例
    pub static CPUS: Vec<Cpu> = {
        let mut v = Vec::new();
        for _ in 0..crate::config::NUM_CPU {
            v.push(Cpu::new());
        }
        v
    };
}

pub fn current_cpu() -> &'static Cpu {
    let hartid: usize = cpu_id();
    &CPUS[hartid]
}
