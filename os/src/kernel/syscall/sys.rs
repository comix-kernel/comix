//! 系统相关系统调用实现

use core::{
    ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
    sync::atomic::Ordering,
};

use crate::{
    arch::{
        lib::sbi::shutdown,
        timer::{TICKS_PER_SEC, TIMER_TICKS, clock_freq},
        trap::SumGuard,
    },
    kernel::{
        current_task,
        syscall::util::{check_syslog_permission, validate_syslog_args},
        time::update_realtime,
    },
    log::{
        DEFAULT_CONSOLE_LEVEL, LogLevel, format_log_entry, get_console_level, read_log,
        set_console_level,
    },
    pr_alert,
    security::{BiogasPoll, EntropyPool},
    uapi::{
        errno::{EINVAL, ENOSYS},
        log::SyslogAction,
        reboot::{
            REBOOT_CMD_POWER_OFF, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_MAGIC2A, REBOOT_MAGIC2B,
            REBOOT_MAGIC2C,
        },
        sysinfo::SysInfo,
        time::clock_id::{
            CLOCK_MONOTONIC, CLOCK_MONOTONIC_COARSE, CLOCK_MONOTONIC_RAW, CLOCK_REALTIME,
            CLOCK_REALTIME_COARSE, MAX_CLOCKS,
        },
        types::SizeT,
        uts_namespace::{UTS_NAME_LEN, UtsNamespace},
    },
    util::{
        cstr_copy,
        user_buffer::{UserBuffer, write_to_user},
    },
    vfs::TimeSpec,
};

/// 重启系统调用
/// # 参数
/// - `magic`: 第一个魔数，必须为 REBOOT_MAGIC1
/// - `magic2`: 第二个魔数，必须为 REBOOT_MAGIC2 或 REBOOT_MAGIC2A/B/C
/// - `op`: 重启操作码，指定重启类型
/// - `arg`: 可选参数，取决于操作码
/// # 返回值
/// 成功返回 0，失败返回负错误码
/// 对于重启或关机操作，函数不会返回
pub fn reboot(magic: c_int, magic2: c_int, op: c_int, _arg: *mut c_void) -> c_int {
    // TODO: 支持更多重启操作码
    if magic as u32 != REBOOT_MAGIC1 {
        return -EINVAL;
    }
    if magic2 as u32 != REBOOT_MAGIC2
        && magic2 as u32 != REBOOT_MAGIC2A
        && magic2 as u32 != REBOOT_MAGIC2B
        && magic2 as u32 != REBOOT_MAGIC2C
    {
        return -EINVAL;
    }
    match op as u32 {
        REBOOT_CMD_POWER_OFF => {
            shutdown(true);
        }
        _ => {
            pr_alert!("reboot: unsupported reboot operation code {}\n", op);
        }
    }
    0
}

/// 获取系统信息系统调用
/// # 参数
/// - `buf`: 指向用户空间缓冲区的指针，用于存储系统信息
/// # 返回值
/// 成功返回 0，失败返回负错误码
pub fn uname(buf: *mut UtsNamespace) -> c_int {
    let uts = {
        let task = current_task();
        let t = task.lock();
        t.uts_namespace.clone()
    };
    let uts_lock = uts.lock();
    unsafe {
        write_to_user(buf, uts_lock.clone());
    }
    0
    // TODO: EPERM 和 EFAULT
}

/// 设置主机名系统调用
/// # 参数
/// - `name`: 指向包含新主机名的用户缓冲区的指针
/// - `len`: 主机名长度
/// # 返回值
/// 成功返回 0，失败返回负错误码
pub fn set_hostname(name: *const c_char, len: usize) -> c_int {
    if len > UTS_NAME_LEN {
        return -EINVAL;
    }
    let uts = {
        let task = current_task();
        let t = task.lock();
        t.uts_namespace.clone()
    };
    let name_buf = UserBuffer::new(name as *mut _, len);
    let name = unsafe { name_buf.copy_from_user() };
    {
        let mut uts_lock = uts.lock();
        cstr_copy(name.as_ptr(), &mut uts_lock.nodename, len);
    }
    0
    // TODO: EPERM 和 EFAULT
}

/// 获取系统信息系统调用
/// # 参数
/// * `info` - 指向用户空间 SysInfo 结构体的指针
/// # 返回值
/// * **成功**：返回 0，`info` 被填充系统信息
/// * **失败**：返回负的 errno
pub fn sysinfo(info: *mut SysInfo) -> c_int {
    // TODO: 填充更多系统信息字段
    let mut sys_info = SysInfo::new();
    sys_info.uptime = (TIMER_TICKS.load(Ordering::SeqCst) / TICKS_PER_SEC) as c_ulong;
    unsafe {
        write_to_user(info, sys_info);
    }
    0
}

/// 获取指定时钟的时间系统调用
/// # 参数
/// * `clk_id` - 时钟 ID（如 CLOCK_REALTIME）
/// * `tp` - 指向用户空间 TimeSpec 结构体的指针，用于存储时间
/// # 返回值
/// * **成功**：返回 0，`tp` 被填充当前时间
/// * **失败**：返回负的 errno
pub fn clock_gettime(clk_id: c_int, tp: *mut TimeSpec) -> c_int {
    let ts = match clk_id {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE => TimeSpec::now(),
        CLOCK_MONOTONIC | CLOCK_MONOTONIC_COARSE | CLOCK_MONOTONIC_RAW => TimeSpec::monotonic_now(),
        id if id < MAX_CLOCKS as c_int && id >= 0 => {
            return -ENOSYS;
        }
        _ => {
            return -EINVAL;
        }
    };

    unsafe {
        write_to_user(tp, ts);
    }

    0
}

/// 设置指定时钟的时间系统调用
/// # 参数
/// * `clk_id` - 时钟 ID（如 CLOCK_REALTIME）
/// * `tp` - 指向用户空间 TimeSpec 结构体的指针，包含要设置的时间
/// # 返回值
/// * **成功**：返回 0，时钟时间被更新
/// * **失败**：返回负的 errno
pub fn clock_settime(clk_id: c_int, tp: *const TimeSpec) -> c_int {
    match clk_id {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE => {
            let ts: TimeSpec = unsafe { core::ptr::read(tp) };
            update_realtime(&ts);
            0
        }
        CLOCK_MONOTONIC | CLOCK_MONOTONIC_COARSE | CLOCK_MONOTONIC_RAW => {
            // 单调时钟不可设置
            -EINVAL
        }
        id if id < MAX_CLOCKS as c_int && id >= 0 => -ENOSYS,
        _ => -EINVAL,
    }
}

/// 获取指定时钟的分辨率系统调用
/// # 参数
/// * `clk_id` - 时钟 ID（如 CLOCK_REALTIME）
/// * `tp` - 指向用户空间 TimeSpec 结构体的指针，用于存储分辨率
/// # 返回值
/// * **成功**：返回 0，`tp` 被填充时钟分辨率
/// * **失败**：返回负的 errno
pub fn clock_getres(clk_id: c_int, tp: *mut TimeSpec) -> c_int {
    let res = match clk_id {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE => TimeSpec {
            tv_sec: 0,
            tv_nsec: 1_000_000_000 / (clock_freq() as c_long),
        },
        CLOCK_MONOTONIC | CLOCK_MONOTONIC_COARSE | CLOCK_MONOTONIC_RAW => TimeSpec {
            tv_sec: 0,
            tv_nsec: 1_000_000_000 / (clock_freq() as c_long),
        },
        id if id < MAX_CLOCKS as c_int && id >= 0 => {
            return -ENOSYS;
        }
        _ => {
            return -EINVAL;
        }
    };

    unsafe {
        write_to_user(tp, res);
    }

    0
}

/// 读取和控制内核日志缓冲区
/// # 参数
/// * `type_` - 操作类型 (0-10)，详见 `SyslogAction`
/// * `bufp` - 用户空间缓冲区指针（某些操作需要）
/// * `len` - 缓冲区长度或命令参数（取决于操作类型）
///
/// # 返回值
/// * **成功**：
///   - 类型 2/3/4: 读取的字节数
///   - 类型 8: 旧的 console_loglevel (1-8)
///   - 类型 9: 未读字节数
///   - 类型 10: 缓冲区总大小
///   - 其他: 0
/// * **失败**：负的 errno
///   - `-EINVAL`: 无效参数
///   - `-EPERM`: 权限不足
///   - `-EINTR`: 被信号中断
///   - `-EFAULT`: 无效的用户空间指针
pub fn syslog(type_: i32, bufp: *mut u8, len: i32) -> isize {
    // 将原始 i32 转换为类型安全的 enum
    let action = match SyslogAction::from_i32(type_) {
        Ok(a) => a,
        Err(e) => return -(e as isize),
    };

    if let Err(e) = validate_syslog_args(action, bufp, len) {
        return -(e as isize);
    }

    if let Err(e) = check_syslog_permission(action) {
        return -(e as isize);
    }

    match action {
        // 破坏性读取操作
        SyslogAction::Read => {
            // Read: 破坏性读取（移除已读条目）
            let buf_len = len as usize;
            let mut total_written = 0;

            // 开启用户空间访问
            let _guard = SumGuard::new();

            while total_written < buf_len {
                // TODO: 暂时移除，等待信号系统实现
                /*
                // 检查信号中断
                if has_pending_signal() {
                    if total_written == 0 {
                        // 未读取任何数据，返回 EINTR
                        return -(EINTR as isize);
                    } else {
                        // 已读取部分数据，返回已读字节数
                        break;
                    }
                }
                */

                // 读取下一条日志
                let entry = match read_log() {
                    Some(e) => e,
                    None => break, // 没有更多日志
                };

                // 格式化日志条目
                let formatted = format_log_entry(&entry);
                let bytes = formatted.as_bytes();

                // 检查剩余空间
                if total_written + bytes.len() > buf_len {
                    // 缓冲区不足，停止读取
                    // 注意：此条目已从缓冲区移除但未返回给用户
                    // 这是 Linux 的行为
                    break;
                }

                // 复制到用户空间
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        bufp.add(total_written),
                        bytes.len(),
                    );
                }

                total_written += bytes.len();
            }

            // 关闭用户空间访问

            total_written as isize
        }

        // 非破坏性读取操作
        SyslogAction::ReadAll => {
            // ReadAll: 非破坏性读取（保留条目）
            use crate::log::{log_reader_index, log_writer_index, peek_log};

            let buf_len = len as usize;
            let mut total_written = 0;

            // 获取当前可读范围
            let start_index = log_reader_index();
            let end_index = log_writer_index();

            // 开启用户空间访问
            let _guard = SumGuard::new();

            // 遍历所有可用的日志条目
            let mut current_index = start_index;
            while current_index < end_index && total_written < buf_len {
                // TODO: 暂时移除，等待信号系统实现
                /*
                // 检查信号中断
                if has_pending_signal() {
                    if total_written == 0 {
                        return -(EINTR as isize);
                    } else {
                        break;
                    }
                }
                */

                let entry = match peek_log(current_index) {
                    Some(e) => e,
                    None => break, // 条目已被覆盖或无效
                };

                let formatted = format_log_entry(&entry);
                let bytes = formatted.as_bytes();

                if total_written + bytes.len() > buf_len {
                    // 缓冲区不足，停止读取
                    break;
                }

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        bufp.add(total_written),
                        bytes.len(),
                    );
                }

                total_written += bytes.len();
                current_index += 1;
            }

            // 关闭用户空间访问

            total_written as isize
        }

        SyslogAction::ReadClear => {
            // 先读取所有日志（使用与 ReadAll 相同的逻辑）
            let buf_len = len as usize;
            let mut total_written = 0;

            let _guard = SumGuard::new();

            while total_written < buf_len {
                let entry = match read_log() {
                    Some(e) => e,
                    None => break,
                };

                let formatted = format_log_entry(&entry);
                let bytes = formatted.as_bytes();

                if total_written + bytes.len() > buf_len {
                    break;
                }

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        bufp.add(total_written),
                        bytes.len(),
                    );
                }

                total_written += bytes.len();
            }

            // 清空剩余的日志
            while read_log().is_some() {}

            total_written as isize
        }

        // 缓冲区控制
        SyslogAction::Clear => {
            // 清空日志缓冲区
            while read_log().is_some() {}
            0
        }

        // 控制台输出控制
        SyslogAction::ConsoleOff => {
            // 禁用控制台输出
            // 设置为 0，只显示 level <= 0 的消息（即只有 EMERG）
            set_console_level(LogLevel::Emergency);
            0
        }

        SyslogAction::ConsoleOn => {
            // 启用控制台输出
            // 恢复到默认级别（Warning = 4）
            set_console_level(DEFAULT_CONSOLE_LEVEL);
            0
        }

        SyslogAction::ConsoleLevel => {
            // 设置控制台日志级别
            //
            // Linux 语义：
            //   console_loglevel = N 表示显示 level < N 的消息
            //   范围：1-8
            //
            // comix 的 log 模块语义：
            //   console_level = N 表示显示 level <= N 的消息
            //   范围：0-7
            //
            // 转换关系：
            //   Linux len=1 -> 只显示 level < 1 (即 EMERG(0))
            //              -> comix 的 console_level = 0 (Emergency)
            //   Linux len=8 -> 显示 level < 8 (即 0-7 全部)
            //              -> comix 的 console_level = 7 (Debug)
            //
            // 公式：console_level_u8 = len - 1

            let new_level_u8 = (len - 1) as u8; // 1-8 -> 0-7
            let new_level = LogLevel::from_u8(new_level_u8);

            // 获取旧值
            let old_level = get_console_level();
            let old_level_u8 = old_level.to_u8();

            // 设置新值
            set_console_level(new_level);

            // 返回旧值（转换回 1-8 范围）
            (old_level_u8 + 1) as isize
        }

        // 查询操作
        SyslogAction::SizeUnread => {
            // 返回未读日志的精确字节数
            use crate::log::log_unread_bytes;
            log_unread_bytes() as isize
        }

        SyslogAction::SizeBuffer => {
            // 返回日志缓冲区的总大小
            use crate::log::GLOBAL_LOG_BUFFER_SIZE;
            GLOBAL_LOG_BUFFER_SIZE as isize
        }

        // 空操作（这些操作在 Linux 中也是 NOP）
        SyslogAction::Close | SyslogAction::Open => 0,
    }
}

/// 获取随机字节系统调用
/// # 参数
/// * `buf`: 指向用户空间缓冲区的指针，用于存储随机字节
/// * `len`: 最大需要填充的字节数
/// * `_flags`: 标志位（当前未使用）
/// # 返回值
/// * **成功**：返回填充的字节数
/// * **失败**：返回负的 errno
pub fn getrandom(buf: *mut c_void, len: SizeT, _flags: c_uint) -> c_int {
    let mut pool = BiogasPoll::new();
    for i in 0..len {
        let byte = match pool.try_fill(core::slice::from_mut(unsafe {
            &mut *(buf as *mut u8).add(i as usize)
        })) {
            Ok(_) => unsafe { *(buf as *mut u8).add(i as usize) },
            Err(_) => return -EINVAL,
        };
        unsafe {
            *(buf as *mut u8).add(i as usize) = byte;
        }
    }
    len as c_int
}
