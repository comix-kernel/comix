pub mod task;

mod cpu;
mod scheduler;
mod tid_allocator;

use core::array;
use cpu::Cpu;
use lazy_static::lazy_static;

use crate::{arch::kernel::cpu::cpu_id, config::NUM_CPU};

lazy_static! {
    // XXX: 很明显有数据竞争
    pub static ref CPUS: [Cpu; NUM_CPU] = array::from_fn(|_| Cpu::new());
}

pub fn current_cpu() -> &'static Cpu {
    let hartid: usize = cpu_id();
    &CPUS[hartid]
}
