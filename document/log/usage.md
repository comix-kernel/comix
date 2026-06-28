# Log 使用方法

本文只保留使用边界和约束。具体函数签名和宏定义以 rustdoc 和源码为准。

## 常规写日志

优先使用 `pr_*` 宏:

```rust
pr_info!("mounted root filesystem");
pr_warn!("retrying transient operation");
pr_err!("failed to open device: {}", name);
pr_debug!("state={:?}", state);
```

这些宏会先检查 global level, 被过滤的日志不会进入后续格式化和缓冲路径。

## 原始控制台输出

`print!` 和 `println!` 适合启动流程或测试输出。它们保持原始控制台文本, 同时把文本作为 Info 日志写入缓冲, 方便 syslog 读取。

不要把 `println!` 当成绕过日志系统的调试通道。若信息有明确严重度, 使用 `pr_*`。

## 紧急输出

panic, trap 异常或锁状态不可信时使用 `console::emergency_print()` 或架构 trap 中的 emergency helper。它绕开常规日志核心, 目标是尽量把诊断打印出来。

## 调整级别

- global level: 控制进入缓冲区的日志。
- console level: 控制即时打印的日志。

开发时可以临时调低阈值观察更多日志。提交前应避免把全局 Debug 输出留在高频路径。

## 读取日志

内核内可通过读取门面消费 `LogEntry`。用户态通过 `syslog` syscall 读取格式化文本:

- destructive read: 读取并推进 read sequence。
- read all: 非破坏性 peek。
- read clear/clear: 读取后或直接清空剩余日志。
- size unread/size buffer: 查询未读格式化字节数或缓冲容量。
- console level: 调整控制台输出阈值。

## 使用约束

- 日志消息保持简短, 超过固定上限会截断。
- 不要在持有关键锁时写大量日志, 即使缓冲无锁, 控制台输出仍可能拖慢路径。
- 不要依赖日志作为同步机制。
- 用户态读取 syslog 时要能处理 ANSI 颜色码。
- 热路径中优先用 `pr_debug!`, 让默认过滤承担开销控制。

## 已知限制

- 当前 syslog 权限检查尚未完整接入 capability。
- 多个用户态读取者会竞争同一个破坏性 read sequence。
- emergency 输出可能不进入缓冲, 只保证尽量打印。

## 源码索引

- `os/src/log/macros.rs`: 日志宏。
- `os/src/log/mod.rs`: 读取和级别门面。
- `os/src/kernel/syscall/sys.rs`: `syslog()`。
- `os/src/uapi/log.rs`: syslog action。
- `os/src/console.rs`: `print!`, `println!`, emergency 输出。
