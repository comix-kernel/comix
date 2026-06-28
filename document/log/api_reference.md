# Log API 边界

本文不是完整 API reference。完整函数签名, 宏定义和字段请看 rustdoc 与源码注释。这里仅说明哪些 API 属于稳定使用边界, 哪些属于内部实现边界。

## 公共使用边界

- `pr_emerg!`, `pr_alert!`, `pr_crit!`, `pr_err!`, `pr_warn!`, `pr_notice!`, `pr_info!`, `pr_debug!`: 内核分级日志入口。
- `print!`, `println!`: 原始控制台输出, 同时写入日志缓冲。
- `set_global_level`, `get_global_level`: 缓冲过滤阈值。
- `set_console_level`, `get_console_level`: 控制台输出阈值。
- `read_log`, `peek_log`, `log_len`, `log_unread_bytes`, `log_dropped_count`: 内核内读取和状态查询。
- `syslog`: 用户态读取和控制日志缓冲的 syscall。

## 内部边界

- `log_impl`: 由 `pr_*` 宏调用, 普通代码不应绕过宏直接调用。
- `print_impl`: 由 `print!`/`println!` 调用。
- `LogCore`: 日志核心状态对象, 生产路径使用全局实例。
- `GlobalLogBuffer`: 环形缓冲实现细节。
- `LogEntry` 内存布局: 不作为用户态 ABI。

## syslog action 分组

- Open/Close: 兼容 NOP。
- Read/ReadAll/ReadClear: 读取日志文本。
- Clear: 清空缓冲。
- ConsoleOff/ConsoleOn/ConsoleLevel: 调整控制台输出级别。
- SizeUnread/SizeBuffer: 查询大小。

参数校验和权限检查位于 syscall util 和 `sys.rs`。当前权限检查仍是预留实现, 后续接入 capability 后应保持 action 分组语义不变。

## 维护约束

- 修改日志格式时, 同步更新控制台格式, syslog 格式和 formatted length 计算。
- 新增公共 API 前先确认是否可以由现有读取/级别门面表达。
- 不要让用户态 ABI 依赖 `LogEntry` 的 Rust 布局。
- 不要在文档复制完整函数清单, 避免和 rustdoc 分叉。

## 源码索引

- `os/src/log/mod.rs`: 公共门面。
- `os/src/log/macros.rs`: 宏入口。
- `os/src/log/log_core.rs`: `LogCore`, `format_log_entry()`。
- `os/src/log/buffer.rs`: 缓冲读取和统计。
- `os/src/log/entry.rs`: `LogEntry`。
- `os/src/kernel/syscall/sys.rs`: `syslog()`。
- `os/src/kernel/syscall/util.rs`: syslog 参数和权限辅助。
- `os/src/uapi/log.rs`: `SyslogAction`。
