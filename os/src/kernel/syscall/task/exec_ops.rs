use super::*;

/// 执行一个新程序（execve）
/// # 参数
/// - `path`: 可执行文件路径
/// - `argv`: 命令行参数
/// - `envp`: 环境变量
/// TODO: 目前该函数可用但亟待完善
pub fn execve(
    path: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    let path_str = match get_path_safe(path as usize) {
        Ok(s) => s,
        Err(_) => {
            return FsError::InvalidArgument.to_errno() as i32;
        }
    };
    let argv_strings = get_args_safe(argv as usize, "argv").unwrap_or_else(|_| Vec::new());
    let envp_strings = get_args_safe(envp as usize, "envp").unwrap_or_else(|_| Vec::new());

    let mut exec_path_str = path_str.clone();
    let (argv_strings, envp_strings, exec_path_str) = {
        // 只读取文件头部用于 hashbang 判断，避免一次性把整个 ELF 读入内存。
        let dentry = match crate::vfs::vfs_lookup(&path_str) {
            Ok(d) => d,
            Err(FsError::NotFound) => return -ENOENT,
            Err(FsError::IsDirectory) => return -EISDIR,
            Err(_) => return -EIO,
        };
        let inode = dentry.inode.clone();
        let meta = match inode.metadata() {
            Ok(m) => m,
            Err(FsError::NotFound) => return -ENOENT,
            Err(FsError::IsDirectory) => return -EISDIR,
            Err(_) => return -EIO,
        };
        if meta.inode_type != crate::vfs::InodeType::File {
            return -EISDIR;
        }

        let prefix_len = core::cmp::min(meta.size, 256);
        if prefix_len == 0 {
            return -ENOEXEC;
        }
        let mut prefix = alloc::vec![0u8; prefix_len];
        let mut read_total = 0usize;
        while read_total < prefix.len() {
            let n = match inode.read_at(read_total, &mut prefix[read_total..]) {
                Ok(n) => n,
                Err(FsError::NotFound) => return -ENOENT,
                Err(FsError::IsDirectory) => return -EISDIR,
                Err(_) => return -EIO,
            };
            if n == 0 {
                break;
            }
            read_total += n;
        }
        if read_total == 0 {
            return -ENOEXEC;
        }
        prefix.truncate(read_total);

        if prefix.len() >= 2 && prefix[0] == b'#' && prefix[1] == b'!' {
            if let Ok((path, args)) = parse_hashbang(&prefix) {
                let mut new_argv = Vec::new();
                new_argv.push(path.to_string());
                // XXX: 目前仅支持单个参数
                if let Some(arg) = args {
                    new_argv.push(arg.to_string());
                }
                new_argv.push(path_str.clone());
                new_argv.extend(argv_strings.iter().skip(1).cloned());
                exec_path_str = path.to_string();
                (new_argv, envp_strings, exec_path_str)
            } else {
                return -EINVAL;
            }
        } else {
            (argv_strings, envp_strings, exec_path_str)
        }
    };

    // // 构造 &str 切片（String 的所有权在本函数内，切片在调用 t.execve 时仍然有效）
    // let argv_refs: Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();
    // let envp_refs: Vec<&str> = envp_strings.iter().map(|s| s.as_str()).collect();

    // /proc/[pid]/exe 使用尽量稳定的绝对路径
    let exe_path = match crate::vfs::vfs_lookup(&exec_path_str) {
        Ok(d) => d.full_path(),
        Err(_) => exec_path_str.clone(),
    };

    // 解析 ELF 并准备新的地址空间（但不切换）
    let (space, initial_pc, sp, phdr_addr, phnum, phent, at_base, at_entry) =
        match do_execve_prepare(&exec_path_str) {
            Ok(res) => res,
            Err(e) => return e,
        };

    drop(path_str);

    // 切换到新的地址空间并恢复到用户态（此函数不会返回）
    do_execve_switch(
        space,
        initial_pc,
        sp,
        exe_path,
        argv_strings, // Pass ownership
        envp_strings, // Pass ownership
        phdr_addr,
        phnum,
        phent,
        at_base,
        at_entry,
    )
}

/// 辅助函数：解析 Hashbang 行
fn parse_hashbang(data: &[u8]) -> Result<(&str, Option<&str>), ()> {
    // 查找第一个换行符 ('\n')，只读取第一行
    let line_end = data.iter().position(|&b| b == b'\n').unwrap_or(data.len());
    // 跳过开头的空格和制表符
    let line_start = data[2..line_end]
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(line_end - 2)
        + 2;
    let line = &data[line_start..line_end];

    // 假设用空格分隔解释器路径和可选参数
    let parts: Vec<&[u8]> = line
        .split(|&b| b == b' ' || b == b'\t')
        .filter(|p| !p.is_empty()) // 过滤空串
        .collect();

    if parts.is_empty() {
        return Err(()); // 格式错误或只包含 #!
    }

    // 解释器路径
    let interpreter_path = core::str::from_utf8(parts[0]).map_err(|_| ())?;

    // 可选参数
    let interpreter_arg = parts
        .get(1)
        .map(|p| core::str::from_utf8(p))
        .transpose()
        .map_err(|_| ())?;

    Ok((interpreter_path, interpreter_arg))
}

/// 执行一个新程序（execve）的准备阶段：解析 ELF 并创建新的地址空间
fn do_execve_prepare(
    path: &str,
) -> Result<(Arc<SpinLock<MemorySpace>>, VA, VA, VA, usize, usize, VA, VA), c_int> {
    let prepared = match crate::kernel::task::prepare_exec_image_from_path(path) {
        Ok(p) => p,
        Err(crate::kernel::task::ExecImageError::Fs(FsError::NotFound)) => return Err(-ENOENT),
        Err(crate::kernel::task::ExecImageError::Fs(FsError::IsDirectory)) => return Err(-EISDIR),
        Err(crate::kernel::task::ExecImageError::Fs(_)) => return Err(-EIO),
        Err(crate::kernel::task::ExecImageError::Paging(
            crate::mm::page_table::PagingError::OutOfMemory,
        )) => return Err(-ENOMEM),
        Err(_) => return Err(-ENOEXEC),
    };

    let space = Arc::new(SpinLock::new(prepared.space));
    Ok((
        space,
        prepared.initial_pc,
        prepared.user_sp_high,
        prepared.phdr_addr,
        prepared.phnum,
        prepared.phent,
        prepared.at_base,
        prepared.at_entry,
    ))
}

/// 执行一个新程序（execve）的切换阶段：切换地址空间并恢复到用户态
/// 注意：此函数不会返回！
fn do_execve_switch(
    space: Arc<SpinLock<MemorySpace>>,
    initial_pc: VA,
    sp: VA,
    exe_path: alloc::string::String,
    argv: Vec<alloc::string::String>,
    envp: Vec<alloc::string::String>,
    phdr_addr: VA,
    phnum: usize,
    phent: usize,
    at_base: VA,
    at_entry: VA,
) -> c_int {
    let task = current_task();

    task.lock().fd_table.close_exec();

    // 换掉当前任务的地址空间，e.g. 切换 satp
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(space.clone());
    }

    // 此时在syscall处理的中断上下文中，中断已关闭，直接修改当前任务的trapframe
    // 注意：space 被 clone 进了 execve，所以这里的 space 变量仍然有效
    {
        // 构造 &str 切片供 execve 使用 (Inner scope to ensure borrows end)
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let envp_refs: Vec<&str> = envp.iter().map(|s| s.as_str()).collect();

        let mut t = task.lock();
        t.exe_path = Some(exe_path);
        t.execve(
            space.clone(),
            initial_pc,
            sp,
            argv_refs.as_slice(),
            envp_refs.as_slice(),
            phdr_addr,
            phnum,
            phent,
            at_base,
            at_entry,
        );
    } // argv_refs/envp_refs dropped here, ending borrow of argv/envp

    let tfp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);

    // Explicitly drop all owned resources before diverging
    drop(argv);
    drop(envp);
    drop(space); // Drop the Arc<MemorySpace> passed in
    drop(task); // Drop current task ref

    // SAFETY: tfp 指向的内存已经被分配且由当前任务拥有
    // 直接按 trapframe 状态恢复并 sret 到用户态
    unsafe {
        restore(&*tfp);
    }
    -1
}
