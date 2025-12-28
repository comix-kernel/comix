//! SBI (Supervisor Binary Interface) 调用接口

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
