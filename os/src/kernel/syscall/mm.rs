use core::ffi::c_void;

use crate::config::PAGE_SIZE;
use crate::kernel::current_memory_space;
use crate::mm::address::{PageNum, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::mm::memory_space::mapping_area::AreaType;
use crate::mm::page_table::UniversalPTEFlag;
use crate::pr_err;
use crate::uapi::errno::{EEXIST, EINVAL, ENOMEM, EOPNOTSUPP};
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
            pr_err!(
                "brk failed: {:?}, new_brk=0x{:x}, current=0x{:x}",
                e, new_brk, current
            );
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
///   - 如果指定了 MAP_FIXED，则必须使用该地址（覆盖现有映射）
///   - 如果指定了 MAP_FIXED_NOREPLACE，则必须使用该地址（不覆盖现有映射）
/// - `len`: 映射的长度（字节）
/// - `prot`: 内存保护标志（PROT_READ | PROT_WRITE | PROT_EXEC）
/// - `flags`: 映射标志（MAP_SHARED | MAP_PRIVATE | MAP_ANONYMOUS 等）
/// - `fd`: 文件描述符（目前仅支持匿名映射，必须为 -1）
/// - `offset`: 文件内偏移量（目前仅支持匿名映射，必须为 0）
///
/// # 返回值
/// - 成功: 返回映射区域的起始地址
/// - 失败: 返回 MAP_FAILED (-1)
///
/// # 支持的特性
/// - ✅ MAP_ANONYMOUS - 匿名映射
/// - ✅ MAP_PRIVATE / MAP_SHARED - 私有/共享映射
/// - ✅ MAP_FIXED - 固定地址映射（覆盖现有）
/// - ✅ MAP_FIXED_NOREPLACE - 固定地址映射（不覆盖）
/// - ✅ 地址 hint 机制
///
/// # 当前限制
/// - ❌ 文件映射（需要 VFS 支持）
/// - ❌ MAP_POPULATE（预分配，当前默认立即分配）
/// - ❌ MAP_NORESERVE（延迟分配，当前默认立即分配）
/// - ❌ 大页 (MAP_HUGETLB)
pub fn mmap(
    addr: *mut c_void,
    len: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: i64,
) -> isize {
    let hint = addr as usize;

    // 参数验证
    if len == 0 {
        pr_err!("mmap: len is zero");
        return -EINVAL as isize;
    }

    // 溢出检查
    if hint.checked_add(len).is_none() {
        pr_err!("mmap: address overflow");
        return -EINVAL as isize;
    }

    // 解析和验证标志
    let map_flags = MapFlags::from_bits_truncate(flags);
    let prot_flags = ProtFlags::from_bits_truncate(prot);

    // 检查 MAP_SHARED / MAP_PRIVATE（必须有且仅有一个）
    if !map_flags.is_valid() {
        pr_err!("mmap: must specify exactly one of MAP_SHARED or MAP_PRIVATE");
        return -EINVAL as isize;
    }

    // 检查 MAP_FIXED 和 MAP_FIXED_NOREPLACE 互斥
    if map_flags.contains(MapFlags::FIXED) && map_flags.contains(MapFlags::FIXED_NOREPLACE) {
        pr_err!("mmap: MAP_FIXED and MAP_FIXED_NOREPLACE are mutually exclusive");
        return -EINVAL as isize;
    }

    // 检查 MAP_FIXED 的地址对齐
    if map_flags.contains(MapFlags::FIXED) && hint & (PAGE_SIZE - 1) != 0 {
        pr_err!("mmap: MAP_FIXED requires page-aligned address");
        return -EINVAL as isize;
    }

    // 匿名映射验证
    if map_flags.contains(MapFlags::ANONYMOUS) {
        if fd != -1 {
            pr_err!("mmap: anonymous mapping requires fd == -1");
            return -EINVAL as isize;
        }
        if offset != 0 {
            pr_err!("mmap: anonymous mapping requires offset == 0");
            return -EINVAL as isize;
        }
    } else {
        // 文件映射（暂未实现）
        pr_err!("mmap: file mapping not yet supported");
        return -EOPNOTSUPP as isize;
    }

    // 确定映射地址
    let memory_space = current_memory_space();
    let mut space = memory_space.lock();

    let start_addr = if map_flags.contains(MapFlags::FIXED) {
        // MAP_FIXED: 强制使用指定地址，覆盖现有映射
        match space.munmap(hint, len) {
            Ok(_) => hint,
            Err(e) => {
                pr_err!("mmap: MAP_FIXED munmap failed: {:?}", e);
                return -EINVAL as isize;
            }
        }
    } else if map_flags.contains(MapFlags::FIXED_NOREPLACE) {
        // MAP_FIXED_NOREPLACE: 强制使用指定地址，不覆盖
        if hint & (PAGE_SIZE - 1) != 0 {
            pr_err!("mmap: MAP_FIXED_NOREPLACE requires page-aligned address");
            return -EINVAL as isize;
        }

        let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(hint));
        let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(hint + len));
        let range = VpnRange::new(start_vpn, end_vpn);

        // 检查是否与现有区域重叠
        let has_overlap = space
            .areas()
            .iter()
            .any(|a| a.vpn_range().overlaps(&range));

        if has_overlap {
            pr_err!("mmap: MAP_FIXED_NOREPLACE address already mapped");
            return -EEXIST as isize;
        }

        hint
    } else {
        // 正常分配（使用 hint）
        if hint == 0 {
            // hint == 0: 内核选择地址
            match space.find_free_region(len, PAGE_SIZE) {
                Some(addr) => addr,
                None => {
                    pr_err!("mmap: out of memory");
                    return -ENOMEM as isize;
                }
            }
        } else {
            // hint != 0: 尝试使用 hint，失败则内核选择
            let aligned_hint = hint & !(PAGE_SIZE - 1);

            let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(aligned_hint));
            let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(aligned_hint + len));
            let range = VpnRange::new(start_vpn, end_vpn);

            let hint_available = !space
                .areas()
                .iter()
                .any(|a| a.vpn_range().overlaps(&range));

            if hint_available {
                aligned_hint
            } else {
                // hint 不可用，内核选择
                match space.find_free_region(len, PAGE_SIZE) {
                    Some(addr) => addr,
                    None => {
                        pr_err!("mmap: out of memory");
                        return -ENOMEM as isize;
                    }
                }
            }
        }
    };

    // 转换权限标志
    let mut pte_flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;

    if prot_flags.contains(ProtFlags::READ) {
        pte_flags |= UniversalPTEFlag::READABLE;
    }
    if prot_flags.contains(ProtFlags::WRITE) {
        pte_flags |= UniversalPTEFlag::WRITEABLE;
        // RISC-V 特性：写权限需要读权限
        pte_flags |= UniversalPTEFlag::READABLE;
    }
    if prot_flags.contains(ProtFlags::EXEC) {
        pte_flags |= UniversalPTEFlag::EXECUTABLE;
    }

    // 创建映射
    let start_vpn = Vpn::from_addr_floor(Vaddr::from_usize(start_addr));
    let end_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(start_addr + len));
    let vpn_range = VpnRange::new(start_vpn, end_vpn);

    match space.insert_framed_area(vpn_range, AreaType::UserMmap, pte_flags, None) {
        Ok(_) => start_addr as isize,
        Err(e) => {
            pr_err!(
                "mmap failed: {:?}, addr=0x{:x}, len=0x{:x}, prot=0x{:x}, flags=0x{:x}",
                e, hint, len, prot, flags
            );
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
    // 参数验证
    if len == 0 {
        return 0; // POSIX: len=0 是合法的，什么都不做
    }

    let start = addr as usize;

    // 获取内存空间并执行解除映射
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
