pub fn main(_hartid: usize) -> ! {
    crate::kernel::boot::idle_loop()
}
