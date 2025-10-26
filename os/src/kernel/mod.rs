pub mod task;

mod cpu;
mod scheduler;

use core::array;
use cpu::Cpu;
use lazy_static::lazy_static;

use crate::{arch::kernel::cpu::cpu_id, config::NUM_CPU};

pub use task::TaskState;
pub use task::TaskStruct;

lazy_static! {
    // XXX: 很明显有数据竞争
    pub static ref CPUS: [Cpu; NUM_CPU] = array::from_fn(|_| Cpu::new());
}

pub fn current_cpu() -> &'static Cpu {
    let hartid: usize = cpu_id();
    &CPUS[hartid]
}
