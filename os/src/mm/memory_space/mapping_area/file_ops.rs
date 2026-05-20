use super::*;

impl MappingArea {
    /// 从文件加载数据到已分配的物理页中
    ///
    /// # 错误
    /// - 文件读取失败
    /// - 页面未分配
    pub fn load_from_file(&mut self) -> Result<(), page_table::PagingError> {
        if let Some(ref mmap_file) = self.file {
            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();

            for (vpn, tracked_frame) in &self.frames {
                // 计算文件偏移量
                let page_offset = vpn.as_usize() - start_vpn.as_usize();
                let file_offset = mmap_file.offset + page_offset * PAGE_SIZE;

                // 获取物理页并通过内核直接映射访问
                let ppn = match tracked_frame {
                    TrackedFrames::Single(frame) => frame.ppn(),
                    TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                };

                let paddr = ppn.start_addr();
                let kernel_vaddr = crate::arch::pa_to_va(paddr);
                let buffer = unsafe {
                    core::slice::from_raw_parts_mut(kernel_vaddr.as_usize() as *mut u8, PAGE_SIZE)
                };

                // 计算实际读取长度（处理文件末尾）
                let read_len = min(
                    PAGE_SIZE,
                    mmap_file.len.saturating_sub(page_offset * PAGE_SIZE),
                );

                if read_len == 0 {
                    continue; // 超出文件末尾，页面保持清零状态
                }

                // 从文件读取数据
                let actual_read = inode
                    .read_at(file_offset, &mut buffer[..read_len])
                    .map_err(|_| page_table::PagingError::InvalidAddress)?;

                // 部分读取时记录警告（剩余部分保持为零）
                if actual_read < read_len {
                    pr_warn!(
                        "Partial read at offset {}: expected {}, got {}",
                        file_offset,
                        read_len,
                        actual_read
                    );
                }

                // buffer[actual_read..] 保持为零（新分配的物理帧默认清零）
            }
        }
        Ok(())
    }

    /// 将脏页写回文件
    ///
    /// # 参数
    /// - `page_table`: 页表引用，用于检查和清除 Dirty 位
    ///
    /// # 错误
    /// - 文件写入失败
    /// - 部分写入
    pub fn sync_file(
        &self,
        page_table: &mut ActivePageTableInner,
    ) -> Result<(), page_table::PagingError> {
        use crate::arch::mm::TlbBatchContext;

        if let Some(ref mmap_file) = self.file {
            // 只有 MAP_SHARED 映射才需要写回
            if !mmap_file.flags.contains(MapFlags::SHARED) {
                return Ok(());
            }

            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();

            TlbBatchContext::execute(|batch| {
                for (vpn, tracked_frame) in &self.frames {
                    // 获取页表项的标志位，检查 Dirty 位
                    let (_, _, flags) = match page_table.walk(*vpn) {
                        Ok(result) => result,
                        Err(_) => continue, // 页面未映射，跳过
                    };

                    if !flags.contains(UniversalPTEFlag::DIRTY) {
                        continue; // 未被修改，跳过
                    }

                    // 计算文件偏移量
                    let page_offset = vpn.as_usize() - start_vpn.as_usize();
                    let file_offset = mmap_file.offset + page_offset * PAGE_SIZE;

                    // 获取物理页内容
                    let ppn = match tracked_frame {
                        TrackedFrames::Single(frame) => frame.ppn(),
                        TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                    };

                    let paddr = ppn.start_addr();
                    let kernel_vaddr = crate::arch::pa_to_va(paddr);
                    let buffer = unsafe {
                        core::slice::from_raw_parts(kernel_vaddr.as_usize() as *const u8, PAGE_SIZE)
                    };

                    // 计算实际写入长度（处理文件末尾）
                    let write_len = min(
                        PAGE_SIZE,
                        mmap_file.len.saturating_sub(page_offset * PAGE_SIZE),
                    );

                    if write_len == 0 {
                        continue; // 超出文件范围
                    }

                    // 写回文件
                    let actual_written = inode
                        .write_at(file_offset, &buffer[..write_len])
                        .map_err(|_| page_table::PagingError::InvalidAddress)?;

                    // 检查是否完全写入
                    if actual_written != write_len {
                        pr_err!(
                            "Partial write at offset {}: expected {}, got {}",
                            file_offset,
                            write_len,
                            actual_written
                        );
                        return Err(page_table::PagingError::InvalidAddress);
                    }

                    // 清除 Dirty 位
                    page_table.update_flags_with_batch(
                        *vpn,
                        flags & !UniversalPTEFlag::DIRTY,
                        Some(batch),
                    )?;
                }
                Ok(())
            })
        } else {
            Ok(())
        }
    }
}
