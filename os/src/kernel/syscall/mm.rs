use core::ffi::c_void;

use crate::kernel::current_memory_space;
use crate::pr_err;
use crate::uapi::errno::{EINVAL, EOPNOTSUPP};
use crate::uapi::mm::{MapFlags, ProtFlags, MAP_FAILED};

/// brk - 改变数据段的结束地址（堆顶）
///
/// # 参数
/// - `new_brk`: 新的堆顶地址
///   - 如果为 0，返回当前 brk 值（查询模式）
///   - 如果非 0，尝试将堆顶设置为该地址
///
/// # 返回值
/// - 成功: 返回新的 brk 值
/// - 失败: 返回当前 brk 值（Linux 语义：brk 失败时返回旧值）
///
/// # 注意
/// - 如果 new_brk 小于堆起始地址，失败并返回当前 brk
/// - 如果 new_brk 超过最大堆大小限制，失败并返回当前 brk
/// - 如果 new_brk 与栈或其他区域重叠，失败并返回当前 brk
pub fn brk(new_brk: usize) -> isize {
    let memory_space = current_memory_space();
    let mut space = memory_space.lock();

    // 获取当前的 brk 值
    let current = space.current_brk().unwrap_or(0);

    // 如果 new_brk 为 0，返回当前 brk（查询模式）
    if new_brk == 0 {
        return current as isize;
    }

    // 尝试设置新的 brk
    match space.brk(new_brk) {
        Ok(addr) => addr as isize,
        Err(e) => {
            pr_err!("brk failed: {:?}, new_brk=0x{:x}, current=0x{:x}", e, new_brk, current);
            // Linux 语义：失败时返回当前 brk
            current as isize
        }
    }
}

/// mmap - 将文件或设备映射到内存
///
/// # 参数
/// - `addr`: 建议的映射起始地址
///   - 如果为 NULL (0)，由内核选择地址
///   - 如果非 NULL，内核会尝试在该地址附近创建映射
///   - 如果指定了 MAP_FIXED，则必须使用该地址（如果不可用则失败）
/// - `len`: 映射的长度（字节）
/// - `prot`: 内存保护标志（PROT_READ | PROT_WRITE | PROT_EXEC）
/// - `flags`: 映射标志（MAP_SHARED | MAP_PRIVATE | MAP_ANONYMOUS 等）
/// - `fd`: 文件描述符（目前仅支持匿名映射，此参数被忽略）
/// - `offset`: 文件内偏移量（目前仅支持匿名映射，此参数被忽略）
///
/// # 返回值
/// - 成功: 返回映射区域的起始地址
/// - 失败: 返回 MAP_FAILED (-1)
///
/// # 当前限制
/// - 仅支持匿名映射 (MAP_ANONYMOUS)
/// - 不支持文件映射
/// - 不支持 MAP_FIXED（会被忽略）
/// - 不支持大页 (MAP_HUGETLB)
pub fn mmap(
    addr: *mut c_void,
    len: usize,
    prot: i32,
    flags: i32,
    _fd: i32,
    _offset: i64,
) -> isize {
    // 1. 参数验证
    if len == 0 {
        return -EINVAL as isize;
    }

    // 解析标志
    let map_flags = MapFlags::from_bits_truncate(flags);
    let prot_flags = ProtFlags::from_bits_truncate(prot);

    // 检查标志的有效性（必须是 SHARED 或 PRIVATE）
    if !map_flags.is_valid() {
        pr_err!("mmap: invalid flags, must specify MAP_SHARED or MAP_PRIVATE");
        return -EINVAL as isize;
    }

    // 当前仅支持匿名映射
    if !map_flags.contains(MapFlags::ANONYMOUS) {
        pr_err!("mmap: only anonymous mappings are supported");
        return -EOPNOTSUPP as isize;
    }

    // 2. 转换保护标志为 uapi::mm::ProtFlags 的位表示
    let prot_bits = prot_flags.bits() as usize;

    // 3. 获取内存空间并执行映射
    let memory_space = current_memory_space();
    let mut space = memory_space.lock();

    let hint = addr as usize;

    match space.mmap(hint, len, prot_bits) {
        Ok(start_addr) => start_addr as isize,
        Err(e) => {
            pr_err!("mmap failed: {:?}, addr=0x{:x}, len=0x{:x}, prot=0x{:x}, flags=0x{:x}",
                    e, hint, len, prot, flags);
            // 返回 MAP_FAILED
            MAP_FAILED
        }
    }
}

/// munmap - 解除内存映射
///
/// # 参数
/// - `addr`: 要解除映射的起始地址
/// - `len`: 要解除映射的长度（字节）
///
/// # 返回值
/// - 成功: 返回 0
/// - 失败: 返回 -errno
///
/// # 注意
/// - 如果 addr 未对齐到页边界，会向下对齐
/// - 如果范围跨越多个映射区域，会部分解除每个区域
/// - 如果地址未映射，返回成功（幂等操作）
pub fn munmap(addr: *mut c_void, len: usize) -> isize {
    // 1. 参数验证
    if len == 0 {
        return 0; // POSIX: len=0 是合法的，什么都不做
    }

    let start = addr as usize;

    // 2. 获取内存空间并执行解除映射
    let memory_space = current_memory_space();
    let mut space = memory_space.lock();

    match space.munmap(start, len) {
        Ok(()) => 0,
        Err(e) => {
            pr_err!("munmap failed: {:?}, addr=0x{:x}, len=0x{:x}", e, start, len);
            -EINVAL as isize
        }
    }
}