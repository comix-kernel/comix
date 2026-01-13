use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::mm::address::{PageNum, UsizeConvert, Vaddr, Vpn};
use crate::mm::memory_space::MemorySpace;
use crate::mm::memory_space::mapping_area::AreaType;
use crate::mm::page_table::{PagingError, UniversalPTEFlag};
use crate::vfs::{FsError, Inode, InodeType};

#[derive(Debug)]
pub enum ExecImageError {
    Fs(FsError),
    InvalidElf,
    Paging(PagingError),
}

pub struct PreparedExecImage {
    pub space: MemorySpace,
    /// 初始 PC：无动态链接器时为程序入口；有 PT_INTERP 时为动态链接器入口
    pub initial_pc: usize,
    pub user_sp_high: usize,
    /// auxv AT_BASE：动态链接器 load bias（无动态链接器时为 0）
    pub at_base: usize,
    /// auxv AT_ENTRY：主程序入口（非动态链接器入口）
    pub at_entry: usize,
    pub phdr_addr: usize,
    pub phnum: usize,
    pub phent: usize,
}

const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;

const ET_DYN: u16 = 3;

const EM_RISCV: u16 = 243;
const EM_LOONGARCH: u16 = 258;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;

const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

// Dynamic tags
const DT_NULL: i64 = 0;
const DT_SYMTAB: i64 = 6;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_SYMENT: i64 = 11;

// riscv64 relocations
const R_RISCV_64: u32 = 2;
const R_RISCV_RELATIVE: u32 = 3;

// loongarch64 relocations
const R_LARCH_64: u32 = 2;
const R_LARCH_RELATIVE: u32 = 3;

#[derive(Clone, Copy, Debug)]
struct ElfHdr {
    e_type: u16,
    e_machine: u16,
    e_entry: u64,
    e_phoff: u64,
    e_phentsize: u16,
    e_phnum: u16,
}

#[derive(Clone, Copy, Debug)]
struct Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
}

fn le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
fn le_u64(b: &[u8]) -> u64 {
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn read_exact_at(inode: &dyn Inode, offset: usize, buf: &mut [u8]) -> Result<(), ExecImageError> {
    let mut read_total = 0usize;
    while read_total < buf.len() {
        let n = inode
            .read_at(offset + read_total, &mut buf[read_total..])
            .map_err(ExecImageError::Fs)?;
        if n == 0 {
            return Err(ExecImageError::InvalidElf);
        }
        read_total += n;
    }
    Ok(())
}

fn parse_elf_header(inode: &dyn Inode) -> Result<ElfHdr, ExecImageError> {
    let mut hdr = [0u8; 64];
    read_exact_at(inode, 0, &mut hdr)?;

    if &hdr[0..4] != b"\x7fELF" {
        return Err(ExecImageError::InvalidElf);
    }
    if hdr[4] != ELFCLASS64 || hdr[5] != ELFDATA2LSB {
        return Err(ExecImageError::InvalidElf);
    }

    let e_type = le_u16(&hdr[16..18]);
    let e_machine = le_u16(&hdr[18..20]);
    let e_entry = le_u64(&hdr[24..32]);
    let e_phoff = le_u64(&hdr[32..40]);
    let e_phentsize = le_u16(&hdr[54..56]);
    let e_phnum = le_u16(&hdr[56..58]);

    #[cfg(target_arch = "riscv64")]
    if e_machine != EM_RISCV {
        return Err(ExecImageError::InvalidElf);
    }
    #[cfg(target_arch = "loongarch64")]
    if e_machine != EM_LOONGARCH {
        return Err(ExecImageError::InvalidElf);
    }
    if e_phentsize as usize != 56 {
        return Err(ExecImageError::InvalidElf);
    }
    if e_phnum == 0 {
        return Err(ExecImageError::InvalidElf);
    }

    Ok(ElfHdr {
        e_type,
        e_machine,
        e_entry,
        e_phoff,
        e_phentsize,
        e_phnum,
    })
}

fn parse_program_headers(inode: &dyn Inode, eh: &ElfHdr) -> Result<Vec<Phdr>, ExecImageError> {
    let phnum = eh.e_phnum as usize;
    let entsz = eh.e_phentsize as usize;
    let total = phnum.checked_mul(entsz).ok_or(ExecImageError::InvalidElf)?;

    let mut buf = vec![0u8; total];
    read_exact_at(inode, eh.e_phoff as usize, &mut buf)?;

    let mut out = Vec::with_capacity(phnum);
    for i in 0..phnum {
        let base = i * entsz;
        let e = &buf[base..base + entsz];

        out.push(Phdr {
            p_type: le_u32(&e[0..4]),
            p_flags: le_u32(&e[4..8]),
            p_offset: le_u64(&e[8..16]),
            p_vaddr: le_u64(&e[16..24]),
            p_filesz: le_u64(&e[32..40]),
            p_memsz: le_u64(&e[40..48]),
        });
    }
    Ok(out)
}

fn find_interp_path(inode: &dyn Inode, phdrs: &[Phdr]) -> Result<Option<String>, ExecImageError> {
    for ph in phdrs {
        if ph.p_type != PT_INTERP {
            continue;
        }
        let len = ph.p_filesz as usize;
        if len == 0 || len > 4096 {
            return Err(ExecImageError::InvalidElf);
        }
        let mut buf = vec![0u8; len];
        read_exact_at(inode, ph.p_offset as usize, &mut buf)?;
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let s = core::str::from_utf8(&buf[..end]).map_err(|_| ExecImageError::InvalidElf)?;
        return Ok(Some(s.to_string()));
    }
    Ok(None)
}

fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

fn load_segments_into_space(
    space: &mut MemorySpace,
    inode: &dyn Inode,
    eh: &ElfHdr,
    phdrs: &[Phdr],
    base_hint: Option<usize>,
    as_mmap_area: bool,
) -> Result<(usize, usize, usize, usize, usize, usize), ExecImageError> {
    // Determine total load range
    let mut min_vaddr = usize::MAX;
    let mut max_vaddr = 0usize;
    let mut has_load = false;

    for ph in phdrs {
        if ph.p_type != PT_LOAD {
            continue;
        }
        has_load = true;
        let vaddr = ph.p_vaddr as usize;
        let end = (ph.p_vaddr + ph.p_memsz) as usize;
        min_vaddr = core::cmp::min(min_vaddr, vaddr);
        max_vaddr = core::cmp::max(max_vaddr, end);
    }
    if !has_load {
        return Err(ExecImageError::InvalidElf);
    }

    let seg_start = align_down(min_vaddr, crate::config::PAGE_SIZE);
    let seg_end = align_up(max_vaddr, crate::config::PAGE_SIZE);
    let total_size = seg_end.saturating_sub(seg_start);
    if total_size == 0 {
        return Err(ExecImageError::InvalidElf);
    }

    let load_bias = if eh.e_type == ET_DYN {
        if let Some(bias) = base_hint {
            bias
        } else {
            let map_start = space
                .find_free_region(total_size, crate::config::PAGE_SIZE)
                .ok_or(ExecImageError::Paging(PagingError::OutOfMemory))?;
            map_start.saturating_sub(seg_start)
        }
    } else {
        0
    };

    // Map PT_LOAD and copy file data incrementally
    let mut tmp = [0u8; 4096];
    let zero_page = [0u8; 4096];
    let mut max_end = 0usize;

    for ph in phdrs {
        if ph.p_type != PT_LOAD {
            continue;
        }
        if ph.p_filesz > ph.p_memsz {
            return Err(ExecImageError::InvalidElf);
        }

        let start_va = load_bias + ph.p_vaddr as usize;
        let end_va = load_bias + (ph.p_vaddr + ph.p_memsz) as usize;
        max_end = core::cmp::max(max_end, end_va);

        // Basic sanity: prevent mapping into user stack range
        if start_va >= crate::config::USER_STACK_TOP - crate::config::USER_STACK_SIZE {
            return Err(ExecImageError::Paging(PagingError::InvalidAddress));
        }

        let vpn_range = crate::mm::address::VpnRange::new(
            Vpn::from_addr_floor(Vaddr::from_usize(start_va)),
            Vpn::from_addr_ceil(Vaddr::from_usize(end_va)),
        );

        let mut perm = UniversalPTEFlag::USER_ACCESSIBLE | UniversalPTEFlag::VALID;
        if (ph.p_flags & PF_R) != 0 {
            perm |= UniversalPTEFlag::READABLE;
        }
        if (ph.p_flags & PF_W) != 0 {
            perm |= UniversalPTEFlag::WRITEABLE;
        }
        if (ph.p_flags & PF_X) != 0 {
            perm |= UniversalPTEFlag::EXECUTABLE;
        }

        let area_type = if as_mmap_area {
            AreaType::UserMmap
        } else if (ph.p_flags & PF_X) != 0 {
            AreaType::UserText
        } else if (ph.p_flags & PF_W) != 0 {
            AreaType::UserData
        } else {
            AreaType::UserRodata
        };

        space
            .insert_framed_area(vpn_range, area_type, perm, None, None)
            .map_err(ExecImageError::Paging)?;

        // Copy file bytes
        let mut remain = ph.p_filesz as usize;
        let mut src_off = ph.p_offset as usize;
        let mut dst_va = start_va;
        while remain > 0 {
            let take = core::cmp::min(remain, tmp.len());
            let n = inode
                .read_at(src_off, &mut tmp[..take])
                .map_err(ExecImageError::Fs)?;
            if n == 0 {
                return Err(ExecImageError::InvalidElf);
            }
            space
                .write_bytes_at(dst_va, &tmp[..n])
                .map_err(ExecImageError::Paging)?;
            src_off += n;
            dst_va += n;
            remain -= n;
        }

        // Zero BSS tail (memsz > filesz)
        let mut zero_remain = (ph.p_memsz - ph.p_filesz) as usize;
        let mut zero_va = start_va + ph.p_filesz as usize;
        while zero_remain > 0 {
            let take = core::cmp::min(zero_remain, zero_page.len());
            space
                .write_bytes_at(zero_va, &zero_page[..take])
                .map_err(ExecImageError::Paging)?;
            zero_va += take;
            zero_remain -= take;
        }
    }

    // Compute PHDR runtime address (for auxv AT_PHDR)
    let mut phdr_addr = 0usize;
    let phoff = eh.e_phoff as usize;
    for ph in phdrs {
        if ph.p_type != PT_LOAD {
            continue;
        }
        let off = ph.p_offset as usize;
        let filesz = ph.p_filesz as usize;
        if phoff >= off && phoff < off + filesz {
            phdr_addr = load_bias + ph.p_vaddr as usize + (phoff - off);
            break;
        }
    }

    let entry = load_bias + eh.e_entry as usize;

    Ok((
        load_bias,
        entry,
        phdr_addr,
        eh.e_phnum as usize,
        eh.e_phentsize as usize,
        max_end,
    ))
}

fn apply_static_pie_relocs(
    space: &mut MemorySpace,
    phdrs: &[Phdr],
    load_bias: usize,
) -> Result<(), ExecImageError> {
    // Find PT_DYNAMIC to locate relocation tables.
    let dyn_ph = phdrs.iter().find(|p| p.p_type == PT_DYNAMIC);
    let Some(dyn_ph) = dyn_ph else { return Ok(()) };

    let mut dt_rela = 0usize;
    let mut dt_relasz = 0usize;
    let mut dt_relaent = 24usize;
    let mut dt_symtab = 0usize;
    let mut dt_syment = 24usize;

    let mut dyn_addr = load_bias + dyn_ph.p_vaddr as usize;
    let dyn_end = dyn_addr + dyn_ph.p_memsz as usize;

    while dyn_addr + 16 <= dyn_end {
        let tag = space
            .read_i64_at(dyn_addr)
            .map_err(ExecImageError::Paging)?;
        let val = space
            .read_u64_at(dyn_addr + 8)
            .map_err(ExecImageError::Paging)? as usize;
        dyn_addr += 16;
        match tag {
            DT_NULL => break,
            DT_RELA => dt_rela = val,
            DT_RELASZ => dt_relasz = val,
            DT_RELAENT => dt_relaent = val,
            DT_SYMTAB => dt_symtab = val,
            DT_SYMENT => dt_syment = val,
            _ => {}
        }
    }

    if dt_rela == 0 || dt_relasz == 0 {
        return Ok(());
    }
    if dt_relaent == 0 {
        return Err(ExecImageError::InvalidElf);
    }
    if dt_syment == 0 {
        return Err(ExecImageError::InvalidElf);
    }

    let rel_base = load_bias + dt_rela;
    let count = dt_relasz / dt_relaent;

    for i in 0..count {
        let r = rel_base + i * dt_relaent;
        let r_offset = space.read_u64_at(r).map_err(ExecImageError::Paging)? as usize;
        let r_info = space.read_u64_at(r + 8).map_err(ExecImageError::Paging)?;
        let r_addend = space.read_i64_at(r + 16).map_err(ExecImageError::Paging)? as isize;

        let r_type = (r_info & 0xffff_ffff) as u32;
        let r_sym = (r_info >> 32) as usize;

        let target_va = load_bias + r_offset;
        #[cfg(target_arch = "riscv64")]
        let value = match r_type {
            R_RISCV_RELATIVE => (load_bias as isize + r_addend) as usize,
            R_RISCV_64 => {
                if dt_symtab == 0 {
                    return Err(ExecImageError::InvalidElf);
                }
                let sym_addr = load_bias + dt_symtab + r_sym * dt_syment;
                let st_value = space
                    .read_u64_at(sym_addr + 8)
                    .map_err(ExecImageError::Paging)? as usize;
                let s = load_bias + st_value;
                (s as isize + r_addend) as usize
            }
            _ => return Err(ExecImageError::InvalidElf),
        };
        #[cfg(target_arch = "loongarch64")]
        let value = match r_type {
            R_LARCH_RELATIVE => (load_bias as isize + r_addend) as usize,
            R_LARCH_64 => {
                if dt_symtab == 0 {
                    return Err(ExecImageError::InvalidElf);
                }
                let sym_addr = load_bias + dt_symtab + r_sym * dt_syment;
                let st_value = space
                    .read_u64_at(sym_addr + 8)
                    .map_err(ExecImageError::Paging)? as usize;
                let s = load_bias + st_value;
                (s as isize + r_addend) as usize
            }
            _ => return Err(ExecImageError::InvalidElf),
        };

        space
            .write_usize_at(target_va, value)
            .map_err(ExecImageError::Paging)?;
    }

    Ok(())
}

pub fn prepare_exec_image_from_path(path: &str) -> Result<PreparedExecImage, ExecImageError> {
    let dentry = crate::vfs::vfs_lookup(path).map_err(ExecImageError::Fs)?;
    let inode = dentry.inode.clone();
    let meta = inode.metadata().map_err(ExecImageError::Fs)?;
    if meta.inode_type != InodeType::File {
        return Err(ExecImageError::Fs(FsError::IsDirectory));
    }

    let eh = parse_elf_header(inode.as_ref())?;
    let phdrs = parse_program_headers(inode.as_ref(), &eh)?;
    let interp = find_interp_path(inode.as_ref(), &phdrs)?;

    let mut space = MemorySpace::new_user_with_kernel_mappings().map_err(ExecImageError::Paging)?;

    // Main program: keep deterministic base for PIE/static-pie to avoid mapping at 0.
    let main_base_hint = if eh.e_type == ET_DYN {
        Some(0x10000usize)
    } else {
        None
    };
    let (main_bias, main_entry, phdr_addr, phnum, phent, main_max_end) = load_segments_into_space(
        &mut space,
        inode.as_ref(),
        &eh,
        &phdrs,
        main_base_hint,
        false,
    )?;

    // Heap starts after end of main segments
    let heap_start_vpn = Vpn::from_addr_ceil(Vaddr::from_usize(main_max_end));
    space.set_heap_start(heap_start_vpn);

    // User stack
    let user_stack_bottom = Vpn::from_addr_floor(Vaddr::from_usize(
        crate::config::USER_STACK_TOP - crate::config::USER_STACK_SIZE,
    ));
    let user_stack_top = Vpn::from_addr_ceil(Vaddr::from_usize(crate::config::USER_STACK_TOP));
    space
        .insert_framed_area(
            crate::mm::address::VpnRange::new(user_stack_bottom, user_stack_top),
            AreaType::UserStack,
            UniversalPTEFlag::user_rw(),
            None,
            None,
        )
        .map_err(ExecImageError::Paging)?;

    let mut initial_pc = main_entry;
    let at_entry = main_entry;
    let mut at_base = 0usize;

    if let Some(interp_path) = interp {
        let interp_dentry = crate::vfs::vfs_lookup(&interp_path).map_err(ExecImageError::Fs)?;
        let interp_inode = interp_dentry.inode.clone();
        let interp_meta = interp_inode.metadata().map_err(ExecImageError::Fs)?;
        if interp_meta.inode_type != InodeType::File {
            return Err(ExecImageError::InvalidElf);
        }

        let interp_eh = parse_elf_header(interp_inode.as_ref())?;
        let interp_phdrs = parse_program_headers(interp_inode.as_ref(), &interp_eh)?;
        let (interp_bias, interp_entry, _, _, _, _) = load_segments_into_space(
            &mut space,
            interp_inode.as_ref(),
            &interp_eh,
            &interp_phdrs,
            None,
            true,
        )?;

        initial_pc = interp_entry;
        at_base = interp_bias;
    } else if eh.e_type == ET_DYN {
        // static-pie: apply minimal relocations when no interpreter is present
        apply_static_pie_relocs(&mut space, &phdrs, main_bias)?;
    }

    Ok(PreparedExecImage {
        space,
        initial_pc,
        user_sp_high: crate::config::USER_STACK_TOP,
        at_base,
        at_entry,
        phdr_addr,
        phnum,
        phent,
    })
}
