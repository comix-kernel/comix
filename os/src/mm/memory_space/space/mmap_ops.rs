use super::*;

impl MemorySpace {
    /// 扩展或收缩堆区域 (brk 系统调用)
    ///
    /// # 错误
    /// - 堆未初始化
    /// - 新的 brk 会超出 MAX_USER_HEAP_SIZE
    /// - 新的 brk 会与现有区域重叠
    pub fn brk(&mut self, new_brk: VA) -> Result<VA, PagingError> {
        let heap_bottom = self.heap_start.ok_or(PagingError::InvalidAddress)?;
        let new_brk_usize = new_brk.as_usize();
        let new_end_vpn = Vpn::from_addr_ceil(new_brk);

        // 边界检查
        if new_brk_usize < heap_bottom.start_addr().as_usize() {
            return Err(PagingError::InvalidAddress);
        }

        let heap_size = new_brk_usize - heap_bottom.start_addr().as_usize();
        if heap_size > MAX_USER_HEAP_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // 检查是否与栈重叠
        if new_brk_usize >= USER_STACK_TOP - USER_STACK_SIZE {
            return Err(PagingError::InvalidAddress);
        }

        // 查找或创建堆区域
        let heap_area_idx = self
            .areas
            .iter()
            .position(|a| a.area_type() == AreaType::UserHeap);

        if let Some(idx) = heap_area_idx {
            // 存在堆区域，调整大小
            let old_end = self.areas[idx].vpn_range().end();

            match new_end_vpn.cmp(&old_end) {
                Ordering::Greater => {
                    // 扩展：检查是否与其他区域冲突
                    let new_range = VpnRange::new(old_end, new_end_vpn);
                    for (i, area) in self.areas.iter().enumerate() {
                        if i != idx && area.vpn_range().overlaps(&new_range) {
                            // 与mmap或其他区域冲突
                            return Err(PagingError::AlreadyMapped);
                        }
                    }

                    let count = new_end_vpn.as_usize() - old_end.as_usize();
                    if count != 0 {
                        self.areas[idx].extend(&mut self.page_table, count)?;
                    }
                }
                Ordering::Less => {
                    // 收缩
                    if new_end_vpn <= heap_bottom {
                        // 收缩到起始位置或更低，删除整个堆区域
                        let mut area = self.areas.remove(idx);
                        area.unmap(&mut self.page_table)?;
                    } else {
                        let count = old_end.as_usize() - new_end_vpn.as_usize();
                        if count != 0 {
                            self.areas[idx].shrink(&mut self.page_table, count)?;
                        }
                    }
                }
                Ordering::Equal => { /* 无操作 */ }
            }
        } else {
            // 第一次分配堆，创建新区域
            if new_end_vpn > heap_bottom {
                // 检查是否与现有区域冲突
                let new_range = VpnRange::new(heap_bottom, new_end_vpn);
                for area in &self.areas {
                    if area.vpn_range().overlaps(&new_range) {
                        return Err(PagingError::AlreadyMapped);
                    }
                }

                self.insert_framed_area(
                    new_range,
                    AreaType::UserHeap,
                    UniversalPTEFlag::user_rw(),
                    None,
                    None, // 非文件映射
                )?;
            }
        }

        Ok(new_brk)
    }

    /// 查找足够大的空闲地址区域
    ///
    /// # 参数
    /// - `size`: 需要的大小（字节）
    /// - `align`: 对齐要求（字节）
    ///
    /// # 返回值
    /// - `Some(addr)`: 找到的空闲区域起始地址（已对齐）
    /// - `None`: 没有足够大的空闲区域
    pub fn find_free_region(&self, size: usize, align: usize) -> Option<VA> {
        // 获取堆的起始和结束
        let heap_start = self.heap_start?.start_addr().as_usize();

        // 获取当前堆的实际结束地址（不包含 mmap 区域）
        let heap_end = self
            .areas
            .iter()
            .filter(|a| a.area_type() == AreaType::UserHeap)
            .map(|a| a.vpn_range().end().start_addr().as_usize())
            .max()
            .unwrap_or(heap_start);

        // 栈的底部地址
        let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;

        // 预留栈增长空间（建议至少 1MB）
        const STACK_GUARD_SIZE: usize = 1024 * 1024;
        let search_limit = stack_bottom.saturating_sub(STACK_GUARD_SIZE);

        // 收集所有用户区域（包括 heap 和 mmap），按起始地址排序
        let mut user_areas: alloc::vec::Vec<(usize, usize)> = self
            .areas
            .iter()
            .filter(|a| {
                matches!(
                    a.area_type(),
                    AreaType::UserHeap
                        | AreaType::UserMmap
                        | AreaType::UserStack
                        | AreaType::UserText
                        | AreaType::UserRodata
                        | AreaType::UserData
                        | AreaType::UserBss
                )
            })
            .map(|a| {
                let start = a.vpn_range().start().start_addr().as_usize();
                let end = a.vpn_range().end().start_addr().as_usize();
                (start, end)
            })
            .collect();

        user_areas.sort_by_key(|&(start, _)| start);

        // Linux 行为更接近 “top-down” 分配：mmap 默认从高地址向低地址找洞，
        // 以避免与 brk(堆) 的向上增长发生冲突。
        //
        // 我们以 search_limit 作为最高可用地址（栈下方保留 guard），在 [heap_end, search_limit) 内自顶向下找洞。
        if size > search_limit.saturating_sub(heap_end) {
            return None;
        }

        let align_down = |addr: usize, align: usize| addr & !(align - 1);

        let mut gap_end = search_limit;
        for &(area_start, area_end) in user_areas.iter().rev() {
            // 只关心 [heap_end, search_limit) 内的区域
            if area_start >= search_limit {
                continue;
            }

            let clamped_end = core::cmp::min(area_end, search_limit);
            if clamped_end < gap_end {
                // gap = [clamped_end, gap_end)
                if gap_end >= heap_end + size {
                    let lowest_ok = core::cmp::max(clamped_end, heap_end);
                    if gap_end > lowest_ok && gap_end - lowest_ok >= size {
                        let candidate = align_down(gap_end - size, align);
                        if candidate >= lowest_ok {
                            return Some(VA::from_usize(candidate));
                        }
                    }
                }
            }

            // 下一段 gap 的上界是当前区域的起点（但也不能低于 heap_end）
            gap_end = core::cmp::max(area_start, heap_end);
            if gap_end < heap_end + size {
                break;
            }
        }

        // 检查堆顶与最低 VMA 之间（或完全没有 VMA 时）的最后一个 gap：
        // gap = [heap_end, gap_end)
        if gap_end >= heap_end + size {
            let candidate = align_down(gap_end - size, align);
            if candidate >= heap_end {
                return Some(VA::from_usize(candidate));
            }
        }

        None
    }

    /// 映射一个匿名区域（简化的 mmap）
    ///
    /// # 参数
    /// - `hint`: 建议的起始地址（0 = 由内核选择）
    /// - `len`: 长度（字节）
    /// - `pte_flags`: 页表项标志（应包含 VALID 和 USER_ACCESSIBLE）
    pub fn mmap(
        &mut self,
        hint: usize,
        len: usize,
        pte_flags: UniversalPTEFlag,
    ) -> Result<usize, PagingError> {
        if len == 0 {
            return Err(PagingError::InvalidAddress);
        }

        // 确定起始地址
        let start = if hint == 0 {
            // 内核选择地址：查找空闲区域
            self.find_free_region(len, crate::config::PAGE_SIZE)
                .ok_or(PagingError::OutOfMemory)?
                .as_usize()
        } else {
            // 用户指定地址

            // 检查是否在有效范围内
            if hint >= USER_STACK_TOP - USER_STACK_SIZE {
                return Err(PagingError::InvalidAddress);
            }

            // 将 hint 向下对齐到页边界（Linux 行为）
            let aligned_hint = hint & !(crate::config::PAGE_SIZE - 1);

            // 检查对齐后的区域是否可用
            let vpn_range_check = VpnRange::new(
                Vpn::from_addr_floor(VA::from_usize(aligned_hint)),
                Vpn::from_addr_ceil(VA::from_usize(aligned_hint + len)),
            );

            // 检查是否与现有区域重叠
            let has_overlap = self
                .areas
                .iter()
                .any(|a| a.vpn_range().overlaps(&vpn_range_check));

            if has_overlap {
                // hint 不可用，尝试查找附近的空闲区域
                // 注意：这里简化处理，直接查找任意空闲区域
                // 更好的实现应该优先查找 hint 附近的区域
                self.find_free_region(len, crate::config::PAGE_SIZE)
                    .ok_or(PagingError::AlreadyMapped)?
                    .as_usize()
            } else {
                aligned_hint
            }
        };

        // 计算 VPN 范围（start 已经是页对齐的）
        let vpn_range = VpnRange::new(
            Vpn::from_addr_floor(VA::from_usize(start)),
            Vpn::from_addr_ceil(VA::from_usize(start + len)),
        );

        // 最终重叠检查（防御性编程）
        for area in &self.areas {
            if area.vpn_range().overlaps(&vpn_range) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        // 创建映射区域
        self.insert_framed_area(vpn_range, AreaType::UserMmap, pte_flags, None, None)?;

        // 返回对齐后的地址
        Ok(start)
    }

    /// 解除映射一个区域（munmap 系统调用）
    ///
    /// # 参数
    /// - `start`: 起始地址（字节）
    /// - `len`: 长度（字节）
    ///
    /// # 返回值
    /// - `Ok(())`: 成功
    /// - `Err(PagingError)`: 失败
    ///
    /// # 语义
    /// - 解除映射 [start, start+len) 范围
    /// - 如果范围跨越多个区域，会部分解除映射每个区域
    /// - 如果只覆盖区域的一部分，会拆分区域
    /// - 如果地址未映射，返回成功（幂等）
    pub fn munmap(&mut self, start: VA, len: usize) -> Result<(), PagingError> {
        // 参数验证
        if len == 0 {
            return Ok(()); // POSIX: len=0 是合法的，什么都不做
        }

        // 计算需要解除映射的 VPN 范围
        let start_vpn = Vpn::from_addr_floor(start);
        let end_vpn = Vpn::from_addr_ceil(VA::from_usize(start.as_usize() + len));
        let unmap_range = VpnRange::new(start_vpn, end_vpn);

        // 收集需要处理的区域
        // 注意：不能在迭代时修改 self.areas，所以先收集索引
        let mut affected_indices = alloc::vec::Vec::new();

        for (idx, area) in self.areas.iter().enumerate() {
            if area.vpn_range().overlaps(&unmap_range) {
                affected_indices.push(idx);
            }
        }

        // 如果没有重叠的区域，直接返回成功（幂等）
        if affected_indices.is_empty() {
            return Ok(());
        }

        // 处理每个受影响的区域
        // 从后往前处理，避免索引失效
        affected_indices.reverse();

        for idx in affected_indices {
            // 移除原区域
            let area = self.areas.remove(idx);

            // 只处理 Framed / Reserved，Direct 映射不应该被 munmap
            if area.map_type() == MapType::Direct {
                // 重新插入原区域
                self.areas.insert(idx, area);
                continue;
            }

            // 在解除映射之前，先尝试写回文件（如果是文件映射）
            // 注意：即使 sync_file 失败，仍然继续 munmap，避免内存泄漏
            let sync_result = area.sync_file(&mut self.page_table);

            // 部分解除映射
            match area.partial_unmap(&mut self.page_table, start_vpn, end_vpn)? {
                None => {
                    // 整个区域被解除映射，不需要重新插入
                }
                Some((left, None)) => {
                    // 只剩一个区域
                    self.areas.insert(idx, left);
                }
                Some((left, Some(right))) => {
                    // 拆分为两个区域
                    self.areas.insert(idx, left);
                    self.areas.insert(idx + 1, right);
                }
            }

            // 如果写回失败，返回错误（但映射已经被解除）
            sync_result?;
        }

        Ok(())
    }

    /// 修改内存区域的保护权限（mprotect 系统调用）
    ///
    /// # 参数
    /// - `start`: 起始地址（字节），必须页对齐
    /// - `len`: 长度（字节）
    /// - `prot`: 新的保护标志
    ///
    /// # 返回值
    /// - 成功: 返回 Ok(())
    /// - 失败: 返回 PagingError
    ///
    /// # 注意
    /// - 地址必须页对齐
    /// - 范围必须完全在现有映射区域内
    /// - 如果 mprotect 只应用于区域的一部分，会自动分割区域
    /// - 只能修改 Framed 类型的映射区域
    pub fn mprotect(
        &mut self,
        start: VA,
        len: usize,
        prot: UniversalPTEFlag,
    ) -> Result<(), PagingError> {
        // 参数验证
        if len == 0 {
            return Ok(()); // len=0 是合法的，什么都不做
        }

        // 检查地址对齐
        if !start.as_usize().is_multiple_of(PAGE_SIZE) {
            return Err(PagingError::InvalidAddress);
        }

        // 计算需要修改权限的 VPN 范围
        let start_vpn = Vpn::from_addr_floor(start);
        let end_vpn = Vpn::from_addr_ceil(VA::from_usize(start.as_usize() + len));
        let change_range = VpnRange::new(start_vpn, end_vpn);

        // 收集需要处理的区域
        // 注意：不能在迭代时修改 self.areas，所以先收集索引
        let mut affected_indices = alloc::vec::Vec::new();

        for (idx, area) in self.areas.iter().enumerate() {
            if area.vpn_range().overlaps(&change_range) {
                // 只处理 Framed / Reserved / Shared，Direct 映射不允许修改权限
                match area.map_type() {
                    MapType::Framed | MapType::Reserved | MapType::Shared => {
                        affected_indices.push(idx)
                    }
                    MapType::Direct => return Err(PagingError::UnsupportedMapType),
                }
            }
        }

        // 如果没有重叠的区域，返回错误（地址无效）
        if affected_indices.is_empty() {
            return Err(PagingError::InvalidAddress);
        }

        // 验证所有需要修改的 VPN 都在某个可修改的用户区域中
        for vpn in start_vpn.as_usize()..end_vpn.as_usize() {
            let vpn = Vpn::from_usize(vpn);
            let found = self.areas.iter().any(|area| {
                area.vpn_range().contains(vpn)
                    && matches!(
                        area.map_type(),
                        MapType::Framed | MapType::Reserved | MapType::Shared
                    )
            });
            if !found {
                return Err(PagingError::InvalidAddress);
            }
        }

        // 处理每个受影响的区域
        // 从后往前处理，避免索引失效
        affected_indices.reverse();

        for idx in affected_indices {
            // 移除原区域
            let area = self.areas.remove(idx);

            // 使用 partial_change_permission 方法处理区域
            let new_areas =
                area.partial_change_permission(&mut self.page_table, start_vpn, end_vpn, prot)?;

            // 将新区域按顺序插入回 areas 列表
            for (offset, new_area) in new_areas.into_iter().enumerate() {
                self.areas.insert(idx + offset, new_area);
            }
        }

        Ok(())
    }
}
