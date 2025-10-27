pub mod task;

mod cpu;
mod scheduler;

use core::array;
use cpu::Cpu;
use lazy_static::lazy_static;

use crate::config::NUM_CPU;
use crate::sync::spin_lock::SpinLock;

pub use task::TaskState;

lazy_static! {
    pub static ref CPUS: [SpinLock<Cpu>; NUM_CPU] = array::from_fn(|_| SpinLock::new(Cpu::new()));
}
