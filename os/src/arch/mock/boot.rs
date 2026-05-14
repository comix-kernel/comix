pub fn main(_hartid: usize) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
