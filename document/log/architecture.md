# Log 架构

Log 架构围绕一个全局 `LogCore` 展开。它把分级过滤, 条目构造, 环形缓冲和控制台即时输出放在同一核心对象中, 对外暴露轻量门面。

## 当前状态

```text
pr_* macro / print!
        |
        v
log::mod public facade
        |
        v
LogCore
   |        |
   v        v
GlobalLogBuffer   Console
        |
        v
syslog syscall
```

`pr_*` 是分级日志入口, `print!`/`println!` 是原始控制台文本入口。两者都会写入缓冲, 但控制台格式不同。

## 目标

- 常规路径: 可过滤, 可缓冲, 可通过 syslog 读取。
- 早期/紧急路径: 不依赖堆, 不依赖复杂初始化, 尽量直接输出。
- 并发路径: 多 CPU 可同时写日志, 读端按序消费。

## 非目标

- 不提供复杂 sink 插件系统。
- 不在日志核心里执行阻塞 I/O。
- 不让用户态直接访问 `LogEntry` 内存布局。

## 双级别过滤

LogCore 有两个阈值:

- global level: 决定是否写入环形缓冲。
- console level: 决定是否即时打印到控制台。

级别数值越小优先级越高, 因此判断逻辑是 `level <= threshold`。默认值由 `config.rs` 给出, 当前全局级别和控制台级别都为 Info。

## 输出路径

### pr_* 日志

`pr_*` 宏先做早期过滤。通过后, LogCore 生成带级别, CPU, task id, timestamp 和消息的 `LogEntry`, 写入缓冲, 再按 console level 选择是否用带前缀格式输出。

### print/println

`print!` 和 `println!` 调用 `print_impl()`。它们保持控制台原文输出, 但同时以 Info 级别写入日志缓冲, 防止普通启动信息绕过 syslog。

### emergency 输出

panic 和部分 trap 路径使用 `console::emergency_print()`。它绕开常规日志核心和控制台锁, 适合系统处于不稳定状态时尽快输出诊断。

## syslog 路径

`syslog` syscall 把缓冲中的 `LogEntry` 格式化为用户可读字符串并复制到用户缓冲。支持破坏性读取, 非破坏性读取, 读取并清空, 清空, 控制台级别调整和大小查询。

## 并发和生命周期约束

- 全局 LogCore 静态初始化, 生命周期覆盖整个内核运行期。
- 缓冲区槽位用 seq 字段作为发布标记, 写入数据后再发布 seq。
- 读端通过 seq 判断槽位是否可读, 读后推进 read sequence。
- 溢出覆盖是设计行为, 不阻塞生产者。
- 控制台即时输出尽量用单次格式化写入, 减少多 CPU 输出交错。

## 已知限制

- 当前是单消费者读取模型。
- 字节计数依赖格式化长度估算, 修改输出格式必须同步更新相关代码。
- emergency 输出不保证进入日志缓冲。

## 源码索引

- `os/src/log/log_core.rs`: `LogCore`, 双过滤器, 格式化输出。
- `os/src/log/mod.rs`: 全局门面。
- `os/src/log/macros.rs`: `pr_*` 宏。
- `os/src/log/buffer.rs`: 环形缓冲。
- `os/src/kernel/syscall/sys.rs`: `syslog()`。
- `os/src/console.rs`: `Stdout`, `emergency_print()`。
