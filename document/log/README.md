# Log 子系统概述

Log 子系统提供内核级分级日志, 固定大小环形缓冲, 控制台即时输出和 `syslog` 读取接口。当前设计偏向早期可用和低依赖, 具体 API 细节以 rustdoc 和源码注释为准。

## 当前状态

- 全局 `LogCore` 使用 `const fn` 静态初始化, 无需运行时 init。
- `pr_*` 宏按级别过滤后写入日志核心。
- `print!`/`println!` 保持原始控制台输出, 同时以 Info 级别写入日志缓冲。
- 日志条目固定大小, 消息超过上限会截断。
- 环形缓冲使用多生产者单消费者模型, 溢出时覆盖最旧未读日志并累计 dropped count。
- `syslog` syscall 支持读取, 非破坏性读取, 清空, 控制台级别和大小查询。
- panic/trap 关键路径可使用 `console::emergency_print()` 绕开常规日志路径。

## 目标

- 让内核任意阶段都能输出关键诊断信息。
- 把常规日志保存在环形缓冲中, 供用户态 syslog/dmesg 风格读取。
- 让控制台噪声可控, 缓冲记录和即时输出使用独立级别阈值。
- 避免日志路径依赖堆分配或阻塞锁。

## 非目标

- 不提供持久化日志文件。
- 不在文档维护完整 API/宏清单。
- 不保证多消费者无锁读取。
- 不让日志系统承担审计, tracing 或结构化事件系统职责。

## 模块边界

- `os/src/log/mod.rs`: 全局单例, 公共门面。
- `os/src/log/macros.rs`: `pr_*` 宏和早期级别过滤。
- `os/src/log/log_core.rs`: 双过滤器, 条目创建, 控制台输出,格式化。
- `os/src/log/buffer.rs`: MPSC 环形缓冲。
- `os/src/log/entry.rs`: 固定大小日志条目。
- `os/src/log/context.rs`: CPU, task, timestamp 收集。
- `os/src/kernel/syscall/sys.rs`: `syslog` syscall。
- `os/src/console.rs`: 常规控制台和 emergency 输出。

## 关键流程

1. 调用 `pr_info!` 等宏。
2. 宏先检查 global level, 被过滤的日志不进入格式化。
3. `LogCore` 收集上下文并构造固定大小 `LogEntry`。
4. 条目写入环形缓冲。
5. 若级别达到 console level, 直接格式化输出到控制台。
6. 用户态通过 `syslog` 读取格式化后的日志文本。

## 并发和生命周期约束

- 写入端使用原子序列号分配槽位。
- 读取端按单消费者模型推进 read sequence。
- 溢出时写入端推进 read sequence 并增加 dropped count。
- `context::collect_context()` 使用 `try_lock` 读取当前 task id, 避免在已经持有 task lock 的路径死锁。
- `direct_print_entry()` 和 `format_log_entry()` 的格式需要和缓冲区字节计数逻辑保持一致。

## 已知限制

- `syslog` 权限检查当前是兼容 stub, 预留完整 capability/dmesg_restrict 逻辑。
- `copy_to_user` 失败在部分 syslog 读取路径中没有细粒度回滚。
- 缓冲区大小固定为编译期常量, 高日志量场景会覆盖旧日志。
- ANSI 颜色码会进入 syslog 格式化输出。

## 文档导航

- [架构设计](architecture.md)
- [日志级别](level.md)
- [缓冲区和条目](buffer_and_entry.md)
- [使用方法](usage.md)
- [API 边界](api_reference.md)
