pub fn shutdown(_failure: bool) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

pub fn restart() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

pub fn console_putchar(_c: u8) {}

pub fn console_getchar() -> usize {
    usize::MAX
}

pub fn send_ipi(_hart_mask: usize) {}

pub fn set_timer(_time: usize) {}
