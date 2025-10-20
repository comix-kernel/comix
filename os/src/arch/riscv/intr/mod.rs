use riscv::register::sie;


pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

pub fn enable_interrupts() {
    unsafe {
        use riscv::register::sstatus; 
        sstatus::set_sie();
    }
}