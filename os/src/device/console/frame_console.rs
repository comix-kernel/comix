// Console -> TextBuffer -> FrameBuffer

/// 初始化控制台设备
pub fn init() {
    // if cfg!(feature = "consolegraphic") {
    //     if let Some(fb) = FRAME_BUFFER.write().take() {
    //         // TODO: now take FrameBuffer out of global variable, then move into Console
    //         let console = Console::on_frame_buffer(fb.fb_info.xres, fb.fb_info.yres, fb);
    //         *CONSOLE.lock() = Some(console);
    //         pr_info!("console: init end");
    //     } else {
    //         pr_warn!("console: init failed");
    //     }
    // }
}
