use super::*;

/// 动态扩展和收缩
impl MappingArea {
    /// 通过在末尾添加页来扩展区域（仅限 4K 页）
    ///
    /// 返回新的结束 VPN
    pub fn extend(
        &mut self,
        page_table: &mut ActivePageTableInner,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() + count);

        // 仅使用 4K 页映射每个新页
        for i in 0..count {
            let vpn = Vpn::from_usize(old_end.as_usize() + i);
            self.map_one(page_table, vpn)?;
        }

        // 更新范围
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }

    /// 通过从末尾移除页来收缩区域（仅限 4K 页）
    ///
    /// 返回新的结束 VPN
    pub fn shrink(
        &mut self,
        page_table: &mut ActivePageTableInner,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        if count > self.vpn_range.len() {
            return Err(page_table::PagingError::ShrinkBelowStart);
        }

        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() - count);

        // 解除映射 [new_end, old_end) 范围内的页
        // 对于 4K 页，解除映射顺序不影响正确性
        for i in 0..count {
            let vpn = Vpn::from_usize(new_end.as_usize() + i);
            self.unmap_one(page_table, vpn)?;
        }

        // 更新范围
        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }
}
