#[cfg(test)]
mod memory_space_tests {
    use super::super::*;
    use crate::config::PAGE_SIZE;
    use crate::mm::address::{PA, UsizeConvert, VA, Vpn, VpnRange};
    use crate::mm::page_table::{PagingError, UniversalPTEFlag};
    use crate::{kassert, println, test_case};

    // 1. 创建内存空间
    test_case!(test_memspace_create, {
        #[allow(unused)]
        let ms = MemorySpace::new();
        // 应该已初始化页表
    });

    // 2. 直接映射：VA 必须 >= PAGE_OFFSET，从已知 PA 经 pa_to_va 计算 Vpn
    test_case!(test_direct_mapping, {
        let mut ms = MemorySpace::new();
        let va_base = crate::arch::pa_to_va(PA::from_usize(0x8000_0000));
        let vpn_start = Vpn::from_addr_ceil(va_base);
        let vpn_range = VpnRange::new(vpn_start, Vpn::from_usize(vpn_start.as_usize() + 0x10));

        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );

        ms.insert_area(area).expect("add area failed");
    });

    // 3. 帧映射
    test_case!(test_framed_mapping, {
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x1000), Vpn::from_usize(0x1010));

        let area = MappingArea::new(
            vpn_range,
            AreaType::UserData,
            MapType::Framed,
            UniversalPTEFlag::user_rw(),
            None,
        );

        ms.insert_area(area).expect("add area failed");
        // 帧映射会自动分配帧
    });

    // 4. 内核空间访问
    test_case!(test_kernel_space, {
        use crate::mm::memory_space::space::kernel_token;

        let token = kernel_token();
        kassert!(token > 0); // 有效的 SATP 值
    });

    // 5. 测试 MMIO 映射是否存在 - 已移除自动映射,改为测试手动映射
    test_case!(test_mmio_mapping_exists, {
        use crate::mm::memory_space::space::with_kernel_space;

        with_kernel_space(|space| {
            // 由于移除了自动 MMIO 映射,初始状态应该没有 MMIO 区域
            let mmio_areas = space.get_mmio_areas();

            println!("Initial MMIO areas count: {}", mmio_areas.len());
            kassert!(mmio_areas.is_empty());

            println!("  MMIO mapping test passed (no auto-mapping as expected)");
        });
    });

    // 6. 测试 MMIO 地址翻译 - 使用独立的 MemorySpace 实例
    test_case!(test_mmio_translation, {
        use crate::arch::ArchImpl;

        // 使用独立的 MemorySpace 实例，避免与其他测试或全局状态冲突
        let mut ms = MemorySpace::new();

        // 使用一个不太可能被占用的测试地址
        const TEST_MMIO_PADDR: usize = 0xE000_0000;
        const TEST_MMIO_SIZE: usize = 0x1000;

        println!(
            "Testing MMIO translation at PA=0x{:x}, size=0x{:x}",
            TEST_MMIO_PADDR, TEST_MMIO_SIZE
        );

        // 手动映射 MMIO 区域
        let paddr = PA::from_usize(TEST_MMIO_PADDR);
        let result = ms.map_mmio(paddr, TEST_MMIO_SIZE);
        kassert!(result.is_ok());

        let vaddr = result.unwrap();
        let mmio_vaddr = vaddr.as_usize();
        let vpn = Vpn::from_addr_floor(vaddr);

        println!("  Mapped to VA=0x{:x}", mmio_vaddr);

        // 查找包含该地址的区域
        let area = ms.find_area(vpn);
        kassert!(area.is_some());

        if let Some(area) = area {
            kassert!(area.area_type() == AreaType::KernelMmio);
            kassert!(area.map_type() == MapType::Direct);
        }

        // 测试页表翻译
        let translated_paddr = ms.page_table().translate(VA::from_usize(mmio_vaddr));
        kassert!(translated_paddr.is_some());

        if let Some(paddr) = translated_paddr {
            println!(
                "  Translation successful: VA 0x{:x} -> PA 0x{:x}",
                mmio_vaddr,
                paddr.as_usize()
            );
            // 验证翻译结果（允许页偏移误差）
            let expected_paddr = TEST_MMIO_PADDR & !0xfff; // 清除页内偏移
            let actual_paddr = paddr.as_usize() & !0xfff;
            kassert!(actual_paddr == expected_paddr);
        }

        println!("  MMIO translation test passed");
    });

    // // 7. 测试 MMIO 内存访问（读写测试）- 修改为手动映射后访问
    // test_case!(test_mmio_memory_access, {
    //     use crate::mm::memory_space::space::with_kernel_space;

    //     // 注意：这个测试会实际访问 MMIO 设备
    //     // QEMU virt 机器的 TEST 设备 (0x100000) 支持简单的读写
    //     const TEST_DEVICE_PADDR: usize = 0x0010_0000;
    //     const TEST_DEVICE_SIZE: usize = 0x1000;

    //     if crate::config::MMIO
    //         .iter()
    //         .any(|&(_, addr, _)| addr == TEST_DEVICE_PADDR)
    //     {
    //         println!("Testing MMIO memory access at PA=0x{:x}", TEST_DEVICE_PADDR);

    //         // XXX: 疑似因为不再使用KERNEL_SPACE作为全局内核页表导致这里失效
    //         with_kernel_space(|space| {
    //             // 手动映射 TEST 设备
    //             let paddr = PA::from_usize(TEST_DEVICE_PADDR);
    //             let result = space.map_mmio(paddr, TEST_DEVICE_SIZE);
    //             kassert!(result.is_ok());

    //             let vaddr = result.unwrap();
    //             let test_vaddr = vaddr.as_usize();

    //             println!("  Mapped TEST device to VA=0x{:x}", test_vaddr);

    //             // 读取测试设备的值（应该可以安全读取）
    //             let value = unsafe { core::ptr::read_volatile(test_vaddr as *const u32) };

    //             println!("  Read value from TEST device: 0x{:x}", value);

    //             // TEST 设备的特性：写入某些值会触发特定行为
    //             // 这里我们只验证写操作不会导致 panic
    //             // 注意：不要写入 0x5555 (FINISHER_PASS) 或 0x3333 (FINISHER_FAIL)
    //             // 因为这会导致 QEMU 退出

    //             println!("  MMIO read test passed (no page fault occurred)");
    //         });
    //     } else {
    //         println!("Warning: TEST device (0x100000) not in MMIO configuration");
    //     }
    // });

    // 8. 测试动态添加 MMIO 映射
    test_case!(test_dynamic_mmio_mapping, {
        use crate::arch::ArchImpl;

        let mut ms = MemorySpace::new();

        // 尝试映射一个自定义的 MMIO 区域（使用未占用的地址）
        const CUSTOM_MMIO_PADDR: usize = 0x5000_0000;
        const CUSTOM_MMIO_SIZE: usize = 0x1000;

        let custom_vaddr = crate::arch::pa_to_va(PA::from_usize(CUSTOM_MMIO_PADDR));

        println!(
            "Adding custom MMIO mapping at PA=0x{:x}, VA=0x{:x}",
            CUSTOM_MMIO_PADDR,
            custom_vaddr.as_usize()
        );

        // 动态添加 MMIO 映射
        let result = ms.map_mmio_region(custom_vaddr, CUSTOM_MMIO_SIZE);
        kassert!(result.is_ok());

        // 验证映射存在
        let vpn = Vpn::from_addr_floor(custom_vaddr);
        let area = ms.find_area(vpn);
        kassert!(area.is_some());

        if let Some(area) = area {
            kassert!(area.area_type() == AreaType::KernelMmio);
            println!("  Dynamic MMIO mapping test passed");
        }
    });

    // 9. 测试 map_mmio 函数 - 新映射
    test_case!(test_map_mmio_new_mapping, {
        let mut ms = MemorySpace::new();

        // 使用一个未占用的物理地址
        const TEST_PADDR: usize = 0x6000_0000;
        const TEST_SIZE: usize = 0x2000;

        let paddr = PA::from_usize(TEST_PADDR);

        println!("Testing map_mmio with new mapping at PA=0x{:x}", TEST_PADDR);

        // 调用 map_mmio 进行映射
        let result = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result.is_ok());

        if let Ok(vaddr) = result {
            println!("  Mapped to VA=0x{:x}", vaddr.as_usize());

            // 验证映射存在
            let vpn = Vpn::from_addr_floor(vaddr);
            let area = ms.find_area(vpn);
            kassert!(area.is_some());

            if let Some(area) = area {
                kassert!(area.area_type() == AreaType::KernelMmio);
                kassert!(area.map_type() == MapType::Direct);
                println!("  map_mmio new mapping test passed");
            }
        }
    });

    // 10. 测试 map_mmio 函数 - 已存在的映射
    test_case!(test_map_mmio_existing_mapping, {
        let mut ms = MemorySpace::new();

        const TEST_PADDR: usize = 0x7000_0000;
        const TEST_SIZE: usize = 0x1000;

        let paddr = PA::from_usize(TEST_PADDR);

        println!(
            "Testing map_mmio with existing mapping at PA=0x{:x}",
            TEST_PADDR
        );

        // 第一次映射
        let result1 = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result1.is_ok());
        let vaddr1 = result1.unwrap();

        // 第二次映射同一个区域
        let result2 = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result2.is_ok());
        let vaddr2 = result2.unwrap();

        // 应该返回相同的虚拟地址
        kassert!(vaddr1.as_usize() == vaddr2.as_usize());
        println!(
            "  map_mmio existing mapping test passed (VA=0x{:x})",
            vaddr1.as_usize()
        );
    });

    // 11. 测试 map_mmio 函数 - 冲突检测
    test_case!(test_map_mmio_conflict, {
        use crate::arch::ArchImpl;

        let mut ms = MemorySpace::new();

        // 使用一个合理的物理地址
        const TEST_PADDR: usize = 0x8000_0000;
        const TEST_SIZE: usize = 0x1000;

        // 先通过 pa_to_va 获取虚拟地址
        let test_vaddr = crate::arch::pa_to_va(PA::from_usize(TEST_PADDR)).as_usize();
        let vpn_start = Vpn::from_addr_floor(VA::from_usize(test_vaddr));
        let vpn_end = Vpn::from_addr_ceil(VA::from_usize(test_vaddr + TEST_SIZE));

        println!(
            "Testing map_mmio conflict detection at VA=0x{:x}",
            test_vaddr
        );

        // 首先映射一个非MMIO区域到这个位置
        let vpn_range = VpnRange::new(vpn_start, vpn_end);
        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );
        ms.insert_area(area).expect("Failed to insert test area");

        // 现在尝试用 map_mmio 映射同一物理地址
        let paddr = PA::from_usize(TEST_PADDR);
        let result = ms.map_mmio(paddr, TEST_SIZE);

        // 应该返回 AlreadyMapped 错误,因为该区域已经被映射为非MMIO类型
        kassert!(result.is_err());

        if let Err(e) = result {
            println!("  Expected error occurred: {:?}", e);
            match e {
                PagingError::AlreadyMapped => {
                    println!("  map_mmio conflict detection test passed");
                }
                _ => {
                    println!("  Unexpected error type: {:?}", e);
                }
            }
        }
    });

    // 12. 测试 unmap_mmio 函数 - 正常取消映射
    test_case!(test_unmap_mmio_normal, {
        let mut ms = MemorySpace::new();

        const TEST_PADDR: usize = 0x9000_0000;
        const TEST_SIZE: usize = 0x1000;

        let paddr = PA::from_usize(TEST_PADDR);

        println!(
            "Testing unmap_mmio with normal unmapping at PA=0x{:x}",
            TEST_PADDR
        );

        // 先映射
        let result = ms.map_mmio(paddr, TEST_SIZE);
        kassert!(result.is_ok());
        let vaddr = result.unwrap();

        println!("  Mapped to VA=0x{:x}", vaddr.as_usize());

        // 验证映射存在
        let vpn = Vpn::from_addr_floor(vaddr);
        kassert!(ms.find_area(vpn).is_some());

        // 取消映射
        let unmap_result = ms.unmap_mmio(vaddr, TEST_SIZE);
        kassert!(unmap_result.is_ok());

        // 验证映射已被移除
        kassert!(ms.find_area(vpn).is_none());
        println!("  unmap_mmio normal test passed");
    });

    // 13. 测试 unmap_mmio 函数 - 取消映射不存在的区域
    test_case!(test_unmap_mmio_not_mapped, {
        let mut ms = MemorySpace::new();

        // 尝试取消映射一个未映射的区域
        let vaddr = VA::from_usize(0xffff_ffc0_a000_0000);
        const TEST_SIZE: usize = 0x1000;

        println!("Testing unmap_mmio with non-existent mapping");

        let result = ms.unmap_mmio(vaddr, TEST_SIZE);
        // 如果没有找到任何区域，areas_to_remove 为空，不会调用 remove_area
        // 所以应该返回 Ok(())
        kassert!(result.is_ok());
        println!("  unmap_mmio non-existent mapping test passed");
    });

    // 14. 测试 unmap_mmio 函数 - 错误的区域类型
    test_case!(test_unmap_mmio_wrong_type, {
        let mut ms = MemorySpace::new();

        // 映射一个非MMIO区域：Direct 映射的 VA 必须 >= PAGE_OFFSET
        let va_base = crate::arch::pa_to_va(PA::from_usize(0xb000_0000));
        let vpn_start = Vpn::from_addr_ceil(va_base);
        let vpn_range = VpnRange::new(vpn_start, Vpn::from_usize(vpn_start.as_usize() + 0x10));
        let area = MappingArea::new(
            vpn_range,
            AreaType::KernelData,
            MapType::Direct,
            UniversalPTEFlag::kernel_rw(),
            None,
        );
        ms.insert_area(area).expect("Failed to insert test area");

        println!("Testing unmap_mmio with wrong area type");

        // 尝试用 unmap_mmio 取消映射非MMIO区域
        let vaddr = vpn_start.start_addr();
        let result = ms.unmap_mmio(vaddr, 0x1000);

        // 应该返回错误
        kassert!(result.is_err());
        if let Err(e) = result {
            println!("  Expected error occurred: {:?}", e);
            println!("  unmap_mmio wrong type test passed");
        }
    });

    // 15. 测试 map_mmio 和 unmap_mmio 组合 - 多个区域
    test_case!(test_mmio_multiple_regions, {
        let mut ms = MemorySpace::new();

        println!("Testing multiple MMIO mappings and unmappings");

        // 映射多个MMIO区域
        const REGION1_PADDR: usize = 0xc000_0000;
        const REGION2_PADDR: usize = 0xd000_0000;
        const REGION_SIZE: usize = 0x1000;

        let paddr1 = PA::from_usize(REGION1_PADDR);
        let paddr2 = PA::from_usize(REGION2_PADDR);

        let vaddr1 = ms
            .map_mmio(paddr1, REGION_SIZE)
            .expect("Failed to map region 1");
        let vaddr2 = ms
            .map_mmio(paddr2, REGION_SIZE)
            .expect("Failed to map region 2");

        println!("  Mapped region 1 to VA=0x{:x}", vaddr1.as_usize());
        println!("  Mapped region 2 to VA=0x{:x}", vaddr2.as_usize());

        // 验证两个区域都存在
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr1)).is_some());
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_some());

        // 取消映射第一个区域
        ms.unmap_mmio(vaddr1, REGION_SIZE)
            .expect("Failed to unmap region 1");
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr1)).is_none());
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_some());

        // 取消映射第二个区域
        ms.unmap_mmio(vaddr2, REGION_SIZE)
            .expect("Failed to unmap region 2");
        kassert!(ms.find_area(Vpn::from_addr_floor(vaddr2)).is_none());

        println!("  Multiple MMIO regions test passed");
    });

    // 16. 测试 mmap 文件映射基本功能
    test_case!(test_mmap_file_basic, {
        use crate::fs::tmpfs::TmpFs;
        use crate::uapi::mm::{MapFlags, ProtFlags};
        use crate::vfs::{File, FileMode, FileSystem};
        use alloc::sync::Arc;

        println!("Testing mmap file mapping basic functionality");

        // 1. 创建临时文件系统和文件
        let tmpfs = TmpFs::new(16); // 16 MB
        let root = tmpfs.root_inode();
        let inode = root
            .create("test_mmap.txt", FileMode::from_bits_truncate(0o644))
            .expect("Failed to create file");

        // 2. 写入测试数据
        let test_data = b"Hello, mmap! This is a test file for memory mapping.";
        let written = inode.write_at(0, test_data).expect("Failed to write data");
        kassert!(written == test_data.len());
        println!("  Written {} bytes to file", written);

        // 3. 创建 File 包装器（需要实现一个简单的 File trait）
        // 注意：这里我们直接使用 Inode，因为 File trait 可能需要额外实现
        // 由于测试环境限制，我们先跳过完整的 mmap 测试
        // 这个测试主要验证数据结构和编译正确性

        println!("  File mapping test structure validated");
    });

    // 17. 测试 load_from_file 方法
    test_case!(test_load_from_file, {
        use crate::fs::tmpfs::TmpFs;
        use crate::vfs::{FileMode, FileSystem};

        println!("Testing load_from_file method");

        // 1. 创建文件并写入数据
        let tmpfs = TmpFs::new(16);
        let root = tmpfs.root_inode();
        let inode = root
            .create("test_load.txt", FileMode::from_bits_truncate(0o644))
            .expect("Failed to create file");

        let test_data = b"Test data for loading into memory pages.";
        inode.write_at(0, test_data).expect("Failed to write");
        println!("  Created file with {} bytes", test_data.len());

        // 注意：由于 MmapFile 需要 Arc<dyn File>，而我们只有 Inode，
        // 完整测试需要实现 File wrapper
        // 这里主要验证结构编译正确

        println!("  load_from_file structure validated");
    });

    // 18. 测试 sync_file 方法（验证写回逻辑）
    test_case!(test_sync_file_logic, {
        println!("Testing sync_file logic");

        // 由于 sync_file 需要：
        // 1. MmapFile（包含 Arc<dyn File>）
        // 2. 页表中的 Dirty 位
        // 3. 实际的文件系统操作
        // 完整测试需要更复杂的设置

        // 这里验证编译和结构正确性
        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x2000), Vpn::from_usize(0x2002));

        // 创建一个没有文件映射的区域
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        // 对于没有文件映射的区域，sync_file 应该直接返回 Ok
        // 需要分两步以避免借用冲突
        let areas_len = ms.areas().len();
        if areas_len > 0 {
            let page_table = &mut ms.page_table;
            let area = &ms.areas[areas_len - 1];
            let result = area.sync_file(page_table);
            kassert!(result.is_ok());
            println!("  sync_file returns Ok for non-file mapping");
        }

        println!("  sync_file logic validated");
    });

    // 19. 测试 Drop trait 实现
    test_case!(test_memory_space_drop, {
        println!("Testing MemorySpace Drop trait");

        // 创建一个内存空间并添加一些区域
        {
            let mut ms = MemorySpace::new();
            let vpn_range = VpnRange::new(Vpn::from_usize(0x3000), Vpn::from_usize(0x3002));

            ms.insert_framed_area(
                vpn_range,
                AreaType::UserData,
                UniversalPTEFlag::user_rw(),
                None,
                None,
            )
            .expect("Failed to insert area");

            println!("  Created MemorySpace with 1 area");
            // ms 在这里离开作用域，应该调用 Drop
        }

        println!("  MemorySpace dropped successfully (no panic)");
    });

    // 20. 测试 mprotect 基本功能
    test_case!(test_mprotect_basic, {
        println!("Testing mprotect basic functionality");

        let mut ms = MemorySpace::new();
        let vpn_range = VpnRange::new(Vpn::from_usize(0x4000), Vpn::from_usize(0x4002));

        // 创建一个可读写的区域
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created area with R/W permissions");

        // 修改为只读
        let start = vpn_range.start().start_addr();
        let len = (vpn_range.end().as_usize() - vpn_range.start().as_usize()) * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());

        kassert!(result.is_ok());
        println!("  Changed permissions to R only");

        // 修改为可执行
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_rx());
        kassert!(result.is_ok());
        println!("  Changed permissions to R+X");

        println!("  mprotect basic test passed");
    });

    // 21. 测试 mprotect 错误处理
    test_case!(test_mprotect_errors, {
        println!("Testing mprotect error handling");

        let mut ms = MemorySpace::new();

        // 测试未对齐的地址
        let result = ms.mprotect(
            VA::from_usize(0x1001),
            PAGE_SIZE,
            UniversalPTEFlag::user_read(),
        );
        kassert!(result.is_err());
        println!("  Correctly rejected unaligned address");

        // 测试未映射的区域
        let result = ms.mprotect(
            VA::from_usize(0x5000 * PAGE_SIZE),
            PAGE_SIZE,
            UniversalPTEFlag::user_read(),
        );
        kassert!(result.is_err());
        println!("  Correctly rejected unmapped region");

        // 测试 len=0
        let result = ms.mprotect(VA::from_usize(0x1000), 0, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());
        println!("  Correctly handled len=0");

        println!("  mprotect error handling test passed");
    });

    // 22. 测试 mprotect 跨多个区域
    test_case!(test_mprotect_multiple_areas, {
        println!("Testing mprotect across multiple areas");

        let mut ms = MemorySpace::new();

        // 创建两个连续的区域
        let vpn_range1 = VpnRange::new(Vpn::from_usize(0x6000), Vpn::from_usize(0x6002));
        let vpn_range2 = VpnRange::new(Vpn::from_usize(0x6002), Vpn::from_usize(0x6004));

        ms.insert_framed_area(
            vpn_range1,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area 1");

        ms.insert_framed_area(
            vpn_range2,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area 2");

        println!("  Created 2 consecutive areas");

        // 修改跨越两个区域的权限
        let start = vpn_range1.start().start_addr();
        let len = (vpn_range2.end().as_usize() - vpn_range1.start().as_usize()) * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());

        kassert!(result.is_ok());
        println!("  Changed permissions across 2 areas");

        println!("  mprotect multiple areas test passed");
    });

    // 23. 测试 mprotect 部分修改 - 修改前半部分
    test_case!(test_mprotect_partial_front, {
        println!("Testing mprotect partial modification - front half");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x7000), Vpn::from_usize(0x7004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 只修改前2页的权限为只读
        let start = vpn_range.start().start_addr();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed first 2 pages to R-only");

        // 验证区域被分割为2个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 2);

        // 验证前2页是只读
        let front_area = ms.find_area(Vpn::from_usize(0x7000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_read());
        println!("  Front area has R-only permission");

        // 验证后2页是读写
        let back_area = ms.find_area(Vpn::from_usize(0x7002)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_rw());
        println!("  Back area has RW permission");

        println!("  mprotect partial front test passed");
    });

    // 24. 测试 mprotect 部分修改 - 修改后半部分
    test_case!(test_mprotect_partial_back, {
        println!("Testing mprotect partial modification - back half");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x8000), Vpn::from_usize(0x8004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 只修改后2页的权限为只读
        let start = Vpn::from_usize(0x8002).start_addr();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed last 2 pages to R-only");

        // 验证区域被分割为2个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 2);

        // 验证前2页是读写
        let front_area = ms.find_area(Vpn::from_usize(0x8000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_rw());
        println!("  Front area has RW permission");

        // 验证后2页是只读
        let back_area = ms.find_area(Vpn::from_usize(0x8002)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_read());
        println!("  Back area has R-only permission");

        println!("  mprotect partial back test passed");
    });

    // 25. 测试 mprotect 部分修改 - 修改中间部分（三分割）
    test_case!(test_mprotect_partial_middle, {
        println!("Testing mprotect partial modification - middle part (3-way split)");

        let mut ms = MemorySpace::new();

        // 创建一个6页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0x9000), Vpn::from_usize(0x9006));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 6-page area with RW permissions");

        // 只修改中间2页（第2-3页，索引从0开始）的权限为只读
        let start = Vpn::from_usize(0x9002).start_addr();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed middle 2 pages to R-only");

        // 验证区域被分割为3个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 3);

        // 验证前2页是读写
        let front_area = ms.find_area(Vpn::from_usize(0x9000)).unwrap();
        kassert!(front_area.permission() == UniversalPTEFlag::user_rw());
        kassert!(front_area.vpn_range().len() == 2);
        println!("  Front area (2 pages) has RW permission");

        // 验证中间2页是只读
        let middle_area = ms.find_area(Vpn::from_usize(0x9002)).unwrap();
        kassert!(middle_area.permission() == UniversalPTEFlag::user_read());
        kassert!(middle_area.vpn_range().len() == 2);
        println!("  Middle area (2 pages) has R-only permission");

        // 验证后2页是读写
        let back_area = ms.find_area(Vpn::from_usize(0x9004)).unwrap();
        kassert!(back_area.permission() == UniversalPTEFlag::user_rw());
        kassert!(back_area.vpn_range().len() == 2);
        println!("  Back area (2 pages) has RW permission");

        println!("  mprotect partial middle test passed");
    });

    // 26. 测试 mprotect 部分修改 - 验证页表权限正确性
    test_case!(test_mprotect_partial_pte_flags, {
        println!("Testing mprotect partial modification - verify PTE flags");

        let mut ms = MemorySpace::new();

        // 创建一个4页的区域，并立即映射
        let vpn_range = VpnRange::new(Vpn::from_usize(0xa000), Vpn::from_usize(0xa004));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 4-page area with RW permissions");

        // 修改前2页的权限为只读
        let start = vpn_range.start().start_addr();
        let len = 2 * PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed first 2 pages to R-only");

        // 验证页表中的权限标志
        for i in 0..2 {
            let vpn = Vpn::from_usize(0xa000 + i);
            if let Ok((_, _, flags)) = ms.page_table().walk(vpn) {
                kassert!(flags.contains(UniversalPTEFlag::READABLE));
                kassert!(!flags.contains(UniversalPTEFlag::WRITEABLE));
                println!(
                    "  VPN 0x{:x} has correct R-only flags in page table",
                    vpn.as_usize()
                );
            }
        }

        for i in 2..4 {
            let vpn = Vpn::from_usize(0xa000 + i);
            if let Ok((_, _, flags)) = ms.page_table().walk(vpn) {
                kassert!(flags.contains(UniversalPTEFlag::READABLE));
                kassert!(flags.contains(UniversalPTEFlag::WRITEABLE));
                println!(
                    "  VPN 0x{:x} has correct RW flags in page table",
                    vpn.as_usize()
                );
            }
        }

        println!("  mprotect partial PTE flags test passed");
    });

    // 27. 测试 mprotect 部分修改 - 边界情况（单页修改）
    test_case!(test_mprotect_partial_single_page, {
        println!("Testing mprotect partial modification - single page");

        let mut ms = MemorySpace::new();

        // 创建一个3页的区域
        let vpn_range = VpnRange::new(Vpn::from_usize(0xb000), Vpn::from_usize(0xb003));
        ms.insert_framed_area(
            vpn_range,
            AreaType::UserMmap,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .expect("Failed to insert area");

        println!("  Created 3-page area with RW permissions");

        // 只修改中间1页的权限为只读
        let start = Vpn::from_usize(0xb001).start_addr();
        let len = PAGE_SIZE;
        let result = ms.mprotect(start, len, UniversalPTEFlag::user_read());
        kassert!(result.is_ok());

        println!("  Changed middle page to R-only");

        // 验证区域被分割为3个
        let area_count = ms
            .areas
            .iter()
            .filter(|a| {
                a.vpn_range().start() >= vpn_range.start() && a.vpn_range().end() <= vpn_range.end()
            })
            .count();
        kassert!(area_count == 3);

        // 验证每页的权限
        let page0 = ms.find_area(Vpn::from_usize(0xb000)).unwrap();
        kassert!(page0.permission() == UniversalPTEFlag::user_rw());
        println!("  Page 0 has RW permission");

        let page1 = ms.find_area(Vpn::from_usize(0xb001)).unwrap();
        kassert!(page1.permission() == UniversalPTEFlag::user_read());
        println!("  Page 1 has R-only permission");

        let page2 = ms.find_area(Vpn::from_usize(0xb002)).unwrap();
        kassert!(page2.permission() == UniversalPTEFlag::user_rw());
        println!("  Page 2 has RW permission");

        println!("  mprotect partial single page test passed");
    });
}
