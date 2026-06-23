use super::*;

/// 重命名或移动文件/目录
pub fn renameat2(
    olddirfd: i32,
    oldpath: *const c_char,
    newdirfd: i32,
    newpath: *const c_char,
    flags: u32,
) -> isize {
    use crate::uapi::{
        errno::{EEXIST, ENOTDIR},
        fs::RenameFlags,
    };

    // 解析标志
    let rename_flags = match RenameFlags::from_bits(flags) {
        Some(f) => f,
        None => return -(EINVAL as isize),
    };

    // 检查标志组合的合法性
    if !rename_flags.is_valid() {
        return -(EINVAL as isize);
    }

    // 解析旧路径
    let old_path_str = match get_path_safe(oldpath as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };

    // 解析新路径
    let new_path_str = match get_path_safe(newpath as usize) {
        Ok(s) => s,
        Err(e) => return e.to_errno(),
    };
    if old_path_str.is_empty() || new_path_str.is_empty() {
        return FsError::NotFound.to_errno();
    }

    // 分割路径为 (父目录, 文件名)
    let (old_dir_path, old_name) = match split_path(&old_path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    let (new_dir_path, new_name) = match split_path(&new_path_str) {
        Ok(p) => p,
        Err(e) => return e.to_errno(),
    };

    // 查找父目录
    let old_parent = match resolve_at_path(olddirfd, &old_dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return -(ENOENT as isize),
        Err(e) => return e.to_errno(),
    };

    let new_parent = match resolve_at_path(newdirfd, &new_dir_path) {
        Ok(Some(d)) => d,
        Ok(None) => return -(ENOENT as isize),
        Err(e) => return e.to_errno(),
    };

    // 验证父目录是目录
    let old_parent_meta = match old_parent.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };
    if old_parent_meta.inode_type != InodeType::Directory {
        return -(ENOTDIR as isize);
    }

    let new_parent_meta = match new_parent.inode.metadata() {
        Ok(m) => m,
        Err(e) => return e.to_errno(),
    };
    if new_parent_meta.inode_type != InodeType::Directory {
        return -(ENOTDIR as isize);
    }

    // 查找源文件(验证存在)
    let _old_inode = match old_parent.inode.lookup(&old_name) {
        Ok(inode) => inode,
        Err(e) => return e.to_errno(),
    };

    // 处理不同的重命名标志
    if rename_flags.contains(RenameFlags::EXCHANGE) {
        // ⚠️ 非原子交换实现警告 ⚠️
        //
        // 由于 ext4_rs 缺少事务日志支持，此实现通过三步操作模拟原子交换:
        //   1. old_name -> temp_name
        //   2. new_name -> old_name
        //   3. temp_name -> new_name
        //
        // 安全性限制:
        // - 在步骤 2/3 失败时会尝试回滚，但回滚本身可能失败
        // - 系统崩溃可能导致文件丢失或重复
        // - 不满足 POSIX 的原子性要求
        //
        // 建议:
        // - 仅在非关键场景使用
        // - 操作后调用 sync() 减少崩溃风险

        crate::pr_warn!(
            "[renameat2] EXCHANGE is non-atomic: {} <-> {} (no transaction support)",
            old_name,
            new_name
        );

        // 验证目标文件存在
        let _new_inode = match new_parent.inode.lookup(&new_name) {
            Ok(inode) => inode,
            Err(e) => {
                crate::pr_err!(
                    "[renameat2] EXCHANGE failed: target '{}' does not exist (error: {:?})",
                    new_name,
                    e
                );
                return -(ENOENT as isize); // EXCHANGE 要求目标必须存在
            }
        };

        // 生成临时文件名(使用时间戳或特殊前缀避免冲突)
        let temp_name = alloc::format!(".rename_temp_{}_{}", old_name, new_name);

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 1/3: '{}' -> '{}' (temp)",
            old_name,
            temp_name
        );

        // 步骤1: old_name -> temp_name
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, old_parent.inode.clone(), &temp_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 1/3 failed: '{}' -> '{}' (error: {:?})",
                old_name,
                temp_name,
                e
            );
            return e.to_errno();
        }

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 2/3: '{}' -> '{}'",
            new_name,
            old_name
        );

        // 步骤2: new_name -> old_name
        if let Err(e) = new_parent
            .inode
            .rename(&new_name, old_parent.inode.clone(), &old_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 2/3 failed: '{}' -> '{}' (error: {:?})",
                new_name,
                old_name,
                e
            );

            // 尝试回滚步骤1
            crate::pr_warn!(
                "[renameat2] Attempting rollback: '{}' -> '{}'",
                temp_name,
                old_name
            );

            match old_parent
                .inode
                .rename(&temp_name, old_parent.inode.clone(), &old_name)
            {
                Ok(_) => {
                    crate::pr_info!("[renameat2] Rollback successful: restored '{}'", old_name);
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback failed! File '{}' may be lost or duplicated (error: {:?})",
                        old_name,
                        rollback_err
                    );
                    crate::pr_err!(
                        "[renameat2] File system may be in inconsistent state. Temp file '{}' exists.",
                        temp_name
                    );
                }
            }

            return e.to_errno();
        }

        crate::pr_debug!(
            "[renameat2] EXCHANGE step 3/3: '{}' (temp) -> '{}'",
            temp_name,
            new_name
        );

        // 步骤3: temp_name -> new_name
        if let Err(e) = old_parent
            .inode
            .rename(&temp_name, new_parent.inode.clone(), &new_name)
        {
            crate::pr_err!(
                "[renameat2] EXCHANGE step 3/3 failed: '{}' -> '{}' (error: {:?})",
                temp_name,
                new_name,
                e
            );

            // 尝试回滚步骤2和步骤1
            crate::pr_warn!("[renameat2] Attempting full rollback (2 operations)");

            let mut rollback_success = true;

            // 回滚步骤2: old_name -> new_name
            crate::pr_debug!("[renameat2] Rollback 1/2: '{}' -> '{}'", old_name, new_name);
            match old_parent
                .inode
                .rename(&old_name, new_parent.inode.clone(), &new_name)
            {
                Ok(_) => {
                    crate::pr_debug!("[renameat2] Rollback 1/2 successful");
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback 1/2 failed! '{}' -> '{}' (error: {:?})",
                        old_name,
                        new_name,
                        rollback_err
                    );
                    rollback_success = false;
                }
            }

            // 回滚步骤1: temp_name -> old_name
            crate::pr_debug!(
                "[renameat2] Rollback 2/2: '{}' -> '{}'",
                temp_name,
                old_name
            );
            match old_parent
                .inode
                .rename(&temp_name, old_parent.inode.clone(), &old_name)
            {
                Ok(_) => {
                    crate::pr_debug!("[renameat2] Rollback 2/2 successful");
                }
                Err(rollback_err) => {
                    crate::pr_err!(
                        "[renameat2] CRITICAL: Rollback 2/2 failed! '{}' -> '{}' (error: {:?})",
                        temp_name,
                        old_name,
                        rollback_err
                    );
                    rollback_success = false;
                }
            }

            if rollback_success {
                crate::pr_info!(
                    "[renameat2] Full rollback successful: files restored to original state"
                );
            } else {
                crate::pr_err!("[renameat2] CRITICAL: Partial or complete rollback failure!");
                crate::pr_err!(
                    "[renameat2] File system is in INCONSISTENT STATE. Manual recovery may be required."
                );
                crate::pr_err!(
                    "[renameat2] Affected files: '{}', '{}', temp '{}'",
                    old_name,
                    new_name,
                    temp_name
                );
            }

            return e.to_errno();
        }

        crate::pr_info!(
            "[renameat2] EXCHANGE completed: '{}' <-> '{}'",
            old_name,
            new_name
        );

        // 更新 dentry 缓存
        drop_cached_child(&old_parent, &old_name);
        drop_cached_child(&old_parent, &temp_name);
        drop_cached_child(&new_parent, &new_name);
    } else if rename_flags.contains(RenameFlags::NOREPLACE) {
        // 目标存在时失败
        if new_parent.inode.lookup(&new_name).is_ok() {
            return -(EEXIST as isize);
        }

        // 执行重命名
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, new_parent.inode.clone(), &new_name)
        {
            return e.to_errno();
        }

        // 更新 dentry 缓存
        drop_cached_child(&old_parent, &old_name);
    } else if rename_flags.contains(RenameFlags::WHITEOUT) {
        // WHITEOUT 暂不支持(需要 Union FS 支持)
        return FsError::NotSupported.to_errno();
    } else {
        // 普通重命名/移动(允许覆盖目标)
        if let Err(e) = old_parent
            .inode
            .rename(&old_name, new_parent.inode.clone(), &new_name)
        {
            return e.to_errno();
        }

        // 更新 dentry 缓存
        drop_cached_child(&old_parent, &old_name);
        drop_cached_child(&new_parent, &new_name);
    }

    0
}
