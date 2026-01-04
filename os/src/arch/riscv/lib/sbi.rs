/// use sbi call to putchar to console (qemu uart handler)
pub fn console_putchar(c: usize) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}

/// 使用 sbi 调用从控制台获取字符(qemu uart handler)
/// 返回值：字符的 ASCII 码
pub fn console_getchar() -> usize {
    #[allow(deprecated)]
    sbi_rt::legacy::console_getchar()
}

/// use sbi call to set timer
pub fn set_timer(timer: usize) {
    sbi_rt::set_timer(timer as _);
}

/// use sbi call to shutdown the system
pub fn shutdown(failure: bool) -> ! {
    use sbi_rt::{NoReason, Shutdown, SystemFailure, system_reset};
    if !failure {
        system_reset(Shutdown, NoReason);
    } else {
        system_reset(Shutdown, SystemFailure);
    }
    unreachable!()
}

/// SBI 调用返回值
#[derive(Debug)]
pub struct SbiRet {
    pub error: isize,
    pub value: usize,
}

/// SBI HSM 扩展 ID
const EID_HSM: usize = 0x48534D;

/// HSM 功能：启动 hart
const FID_HART_START: usize = 0;

/// SBI IPI 扩展 ID
const EID_IPI: usize = 0x735049;

/// IPI 功能：发送 IPI
const FID_SEND_IPI: usize = 0;

/// Legacy SBI 发送 IPI
const LEGACY_SEND_IPI: usize = 4;

/// 执行 SBI 调用
#[inline(always)]
fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> SbiRet {
    let error: isize;
    let value: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            lateout("a0") error,
            lateout("a1") value,
        );
    }
    SbiRet { error, value }
}

/// 启动指定的 hart
///
/// # 参数
/// - hartid: 要启动的 hart ID
/// - start_addr: 启动地址
/// - opaque: 传递给 hart 的参数（通过 a1 寄存器）
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> SbiRet {
    sbi_call(EID_HSM, FID_HART_START, hartid, start_addr, opaque)
}

/// 发送 IPI 到指定的 hart
///
/// 使用 SBI IPI 扩展或 Legacy SBI
///
/// # 参数
/// - hart_mask: hart 位掩码，每位代表一个 hart
pub fn send_ipi(hart_mask: usize) {
    // 尝试使用 SBI IPI 扩展
    let ret = sbi_call(EID_IPI, FID_SEND_IPI, hart_mask, 0, 0);

    if ret.error == 0 {
        return;
    }

    // 回退到 Legacy SBI
    // Legacy SBI 使用指针传递 hart_mask
    let _ret2 = sbi_call(LEGACY_SEND_IPI, 0, &hart_mask as *const _ as usize, 0, 0);
}
