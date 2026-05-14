pub fn are_interrupts_enabled() -> bool {
    false
}

pub unsafe fn read_and_disable_interrupts() -> usize {
    0
}

pub unsafe fn restore_interrupts(_flags: usize) {}

pub unsafe fn enable_interrupts() {}

pub unsafe fn disable_interrupts() {}

pub unsafe fn read_and_enable_interrupts() -> usize {
    0
}

pub fn enable_irq(_irq: usize) {}

pub fn enable_software_interrupt() {}

pub fn enable_timer_interrupt() {}
