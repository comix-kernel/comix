use super::*;

impl MemorySpace {
    /// 从 ELF 文件创建用户内存空间
    ///
    /// 此方法通过创建一个包含用户空间映射（进程私有）和内核空间映射（所有进程共享）的页表，
    /// 实现了方案 2（共享页表）。
    ///
    /// 最终的页表支持零开销的用户/内核模式切换：
    /// 当用户进程陷入内核时，内核代码已被映射且可访问，无需切换 `satp`。
    ///
    /// # 返回
    /// 成功时返回 `Ok((space, entry_point, user_stack_top))`：
    /// - `space`: 包含用户 + 内核映射的内存空间
    /// - `entry_point`: 程序入口地址（来自 ELF 头）
    /// - `user_stack_top`: 用户栈的顶部地址
    ///
    /// # 错误
    /// - ELF 解析失败
    /// - 架构不匹配（非 RISC-V）
    /// - 段与保留区域重叠
    pub fn from_elf(
        elf_data: &[u8],
    ) -> Result<(Self, usize, usize, usize, usize, usize), PagingError> {
        use xmas_elf::ElfFile;
        use xmas_elf::program::{SegmentData, Type};
        use xmas_elf::sections::{SectionData, ShType};
        use xmas_elf::symbol_table::Entry as ElfSymEntry;

        let elf = ElfFile::new(elf_data).map_err(|_| PagingError::InvalidAddress)?;

        // 检查架构
        let machine = elf.header.pt2.machine().as_machine();
        let machine_number = match machine {
            xmas_elf::header::Machine::RISC_V => Some(crate::arch::abi::EM_RISCV),
            xmas_elf::header::Machine::Other(value) => Some(value),
            _ => None,
        };
        if !machine_number
            .map(crate::arch::abi::is_supported_elf_machine)
            .unwrap_or(false)
        {
            crate::pr_err!("[from_elf] machine mismatch: got {:?}", machine);
            return Err(PagingError::InvalidAddress);
        }

        // 对 ET_DYN (PIE/static-pie) 采用固定 load bias，避免把可执行映射放到 VA=0。
        // 这也便于按 ELF relocation 语义处理 R_RISCV_RELATIVE。
        //
        // NOTE: 目前 bias 固定；若未来引入 ASLR，可改为随机。
        let load_bias: usize = match elf.header.pt2.type_().as_type() {
            xmas_elf::header::Type::SharedObject => 0x10000, // ET_DYN
            _ => 0,
        };

        // 创建新的内存空间，只复制内核映射（不复制用户空间数据）
        let current_space = crate::kernel::current_memory_space();
        let current_locked = current_space.lock();

        let mut space = MemorySpace::new()?;

        // 只复制内核空间区域的元数据和映射
        for area in current_locked.areas.iter() {
            let is_kernel = matches!(
                area.area_type(),
                AreaType::KernelText
                    | AreaType::KernelRodata
                    | AreaType::KernelData
                    | AreaType::KernelBss
                    | AreaType::KernelStack
                    | AreaType::KernelHeap
                    | AreaType::KernelMmio
            );
            if is_kernel {
                // 对于内核区域，只需要克隆元数据并重新映射（不复制数据）
                space.clone_direct_area(area)?;
            }
        }

        drop(current_locked);

        let mut max_end_vpn = Vpn::from_usize(0);

        // 1. 解析并映射 ELF 段
        for ph in elf.program_iter() {
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let start_va = load_bias + ph.virtual_addr() as usize;
            let end_va = load_bias + (ph.virtual_addr() + ph.mem_size()) as usize;

            // 检查段是否与栈/陷阱区域重叠
            if start_va >= USER_STACK_TOP - USER_STACK_SIZE {
                crate::pr_err!(
                    "[from_elf] segment overlaps stack: start=0x{:x}, end=0x{:x}, stack_bottom=0x{:x}",
                    start_va,
                    end_va,
                    USER_STACK_TOP - USER_STACK_SIZE
                );
                return Err(PagingError::InvalidAddress);
            }

            let vpn_range = VpnRange::new(
                Vpn::from_addr_floor(VA::from_usize(start_va)),
                Vpn::from_addr_ceil(VA::from_usize(end_va)),
            );

            max_end_vpn = if max_end_vpn.as_usize() > vpn_range.end().as_usize() {
                max_end_vpn
            } else {
                vpn_range.end()
            };

            // 构建权限
            let mut flags = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
            if ph.flags().is_read() {
                flags |= UniversalPTEFlag::READABLE;
            }
            if ph.flags().is_write() {
                flags |= UniversalPTEFlag::WRITEABLE;
            }
            if ph.flags().is_execute() {
                flags |= UniversalPTEFlag::EXECUTABLE;
            }

            // 确定区域类型
            let area_type = if ph.flags().is_execute() {
                AreaType::UserText
            } else if ph.flags().is_write() {
                AreaType::UserData
            } else {
                AreaType::UserRodata
            };

            // 获取段数据
            let data = match ph.get_data(&elf) {
                Ok(SegmentData::Undefined(data)) => Some(data),
                _ => None,
            };

            // 插入区域（将在内部检查重叠）
            if let Err(err) = space.insert_framed_area_with_offset(
                vpn_range,
                area_type,
                flags,
                data,
                start_va % PAGE_SIZE, // bias 是页对齐的，因此等价于 p_vaddr % PAGE_SIZE
                None,                 // 非文件映射
            ) {
                crate::pr_err!(
                    "[from_elf] map segment failed: start=0x{:x}, end=0x{:x}, flags={:?}, err={:?}",
                    start_va,
                    end_va,
                    flags,
                    err
                );
                return Err(err);
            }
        }

        // 1.5 对静态 PIE/PIE 应用最小化重定位：RELATIVE/64
        //
        // 典型的 static-pie（如 data/bin/iperf3）会把 GOT/函数指针以 0 填充，
        // 依赖 .rela.dyn 的 RELATIVE relocations 在加载时写入正确地址。
        // 如果不做这一步，程序往往会在某个间接调用点跳到 sepc=0 执行到 ELF header。
        enum Symtab64<'a> {
            Dyn(&'a [xmas_elf::symbol_table::DynEntry64]),
            Std(&'a [xmas_elf::symbol_table::Entry64]),
        }

        let write_usize_at = |va: usize, value: usize| -> Result<(), PagingError> {
            let paddr = space
                .page_table
                .translate(VA::from_usize(va))
                .ok_or(PagingError::InvalidAddress)?;
            let paddr_usize = paddr.as_usize();
            let page_base = paddr_usize & !(PAGE_SIZE - 1);
            let off = paddr_usize & (PAGE_SIZE - 1);
            let kva = crate::arch::pa_to_va(PA::from_usize(page_base)).as_usize() + off;
            unsafe {
                core::ptr::write_unaligned(kva as *mut usize, value);
            }
            Ok(())
        };

        for sh in elf.section_iter() {
            if sh.get_type() != Ok(ShType::Rela) {
                continue;
            }

            // 解析 rela entries
            let relas = match sh.get_data(&elf) {
                Ok(SectionData::Rela64(entries)) => entries,
                _ => continue,
            };

            // 找到该 rela section 关联的符号表（sh_link）
            let symtab = if sh.link() != 0 {
                match elf.section_header(sh.link() as u16) {
                    Ok(sym_sh) => match sym_sh.get_data(&elf) {
                        Ok(SectionData::DynSymbolTable64(syms)) => Some(Symtab64::Dyn(syms)),
                        Ok(SectionData::SymbolTable64(syms)) => Some(Symtab64::Std(syms)),
                        _ => None,
                    },
                    Err(_) => None,
                }
            } else {
                None
            };

            for rela in relas {
                let r_type = rela.get_type();
                let r_offset = rela.get_offset() as usize;
                let r_sym = rela.get_symbol_table_index() as usize;
                let addend = rela.get_addend() as i64 as isize;

                let target_va = load_bias + r_offset;

                let kind = match crate::arch::abi::classify_relocation(r_type) {
                    Some(kind) => kind,
                    None => {
                        pr_err!("[ELF] Unsupported relocation type: {}", r_type);
                        return Err(PagingError::InvalidAddress);
                    }
                };
                let sym_val = match kind {
                    crate::arch::abi::RelocationKind::Relative => 0,
                    crate::arch::abi::RelocationKind::Absolute64 => {
                        if r_sym == 0 {
                            0
                        } else {
                            let Some(symtab) = symtab.as_ref() else {
                                pr_err!(
                                    "[ELF] absolute relocation requires symtab, but sh_link is missing"
                                );
                                return Err(PagingError::InvalidAddress);
                            };
                            match symtab {
                                Symtab64::Dyn(syms) => {
                                    syms.get(r_sym).ok_or(PagingError::InvalidAddress)?.value()
                                        as usize
                                }
                                Symtab64::Std(syms) => {
                                    syms.get(r_sym).ok_or(PagingError::InvalidAddress)?.value()
                                        as usize
                                }
                            }
                        }
                    }
                };
                let value =
                    crate::arch::abi::resolve_relocation_value(kind, load_bias, sym_val, addend);

                write_usize_at(target_va, value)?;
            }
        }

        // 2. 初始化堆（从 ELF 结束地址开始，页对齐）
        space.heap_start = Some(max_end_vpn);

        // 3. 映射用户栈（带保护页）
        let user_stack_bottom =
            Vpn::from_addr_floor(VA::from_usize(USER_STACK_TOP - USER_STACK_SIZE));
        let user_stack_top = Vpn::from_addr_ceil(VA::from_usize(USER_STACK_TOP));

        space.insert_framed_area(
            VpnRange::new(user_stack_bottom, user_stack_top),
            AreaType::UserStack,
            UniversalPTEFlag::user_rw(),
            None,
            None, // 非文件映射
        )?;

        // Userspace rt_sigreturn trampoline (Linux ABI).
        space.map_user_sigreturn_trampoline()?;

        let entry_point = load_bias + elf.header.pt2.entry_point() as usize;
        let ph_off = elf.header.pt2.ph_offset() as usize;
        let ph_num = elf.header.pt2.ph_count();
        let ph_ent = elf.header.pt2.ph_entry_size();

        // PHDR 在虚拟内存中的地址 = 第一个 LOAD 段的虚拟地址 + PHDR 在文件中的偏移
        // 假设第一个 LOAD 段映射了 ELF 头（通常如此，偏移为 0）
        // 如果第一个 LOAD 段偏移不为 0，则需要更复杂的逻辑，但对于标准 ELF 通常足够
        // 简单起见，我们假设 ELF 头被映射到了 base_addr
        // 实际上，我们应该找到包含 ph_off 的那个段
        let mut phdr_addr = 0;
        for ph in elf.program_iter() {
            if ph.get_type() == Ok(Type::Load) {
                let vaddr = ph.virtual_addr() as usize;
                let offset = ph.offset() as usize;
                let filesz = ph.file_size() as usize;
                if ph_off >= offset && ph_off < offset + filesz {
                    phdr_addr = load_bias + vaddr + (ph_off - offset);
                    break;
                }
            }
        }

        Ok((
            space,
            entry_point,
            USER_STACK_TOP,
            phdr_addr,
            ph_num as usize,
            ph_ent as usize,
        ))
    }
}
