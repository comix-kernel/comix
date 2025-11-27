# Log 子系统 API 参考

## 概述

本文档提供 Log 子系统所有公共 API 的完整参考，包括宏接口、写入 API、读取 API、配置 API 以及核心类型定义。每个 API 都包含函数签名、功能描述、源代码位置和使用示例。

## 目录

- [宏接口](#宏接口)
- [写入 API](#写入-api)
- [读取 API](#读取-api)
  - [read_log](#read_log) - 破坏性读取
  - [peek_log](#peek_log) - 非破坏性读取
  - [log_len](#log_len) - 日志条目数量
  - [log_unread_bytes](#log_unread_bytes) - 未读字节数
  - [log_reader_index](#log_reader_index) - 读指针位置
  - [log_writer_index](#log_writer_index) - 写指针位置
  - [log_dropped_count](#log_dropped_count) - 丢弃计数
- [配置 API](#配置-api)
- [核心类型](#核心类型)
- [系统调用](#系统调用)
  - [syslog](#syslog) - 用户空间日志控制

---

## 宏接口

Log 子系统提供 8 个宏，对应 8 个日志级别。这些宏是用户代码记录日志的主要接口。

### pr_emerg!

**级别**：Emergency (0)

**位置**：`os/src/log/macros.rs:60-67`

**签名**：
```rust
macro_rules! pr_emerg {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Emergency 级别的日志，表示系统不可用或即将崩溃。

**使用示例**：
```rust
pr_emerg!("Kernel panic: unable to continue");
pr_emerg!("Critical hardware failure: {}", device_name);
```

---

### pr_alert!

**级别**：Alert (1)

**位置**：`os/src/log/macros.rs:79-86`

**签名**：
```rust
macro_rules! pr_alert {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Alert 级别的日志，表示必须立即采取行动的严重情况。

**使用示例**：
```rust
pr_alert!("Filesystem corruption detected");
pr_alert!("Critical device failure: {}", error_code);
```

---

### pr_crit!

**级别**：Critical (2)

**位置**：`os/src/log/macros.rs:98-105`

**签名**：
```rust
macro_rules! pr_crit {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Critical 级别的日志，表示临界错误，系统功能受到严重影响。

**使用示例**：
```rust
pr_crit!("Failed to initialize memory subsystem");
pr_crit!("Security violation detected: {}", violation_type);
```

---

### pr_err!

**级别**：Error (3)

**位置**：`os/src/log/macros.rs:118-127`

**签名**：
```rust
macro_rules! pr_err {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Error 级别的日志，表示错误条件，某个功能无法正常工作。

**使用示例**：
```rust
pr_err!("Failed to open file: {}", filename);
pr_err!("Device driver error: code = {}", error_code);
```

---

### pr_warn!

**级别**：Warning (4)

**位置**：`os/src/log/macros.rs:139-148`

**签名**：
```rust
macro_rules! pr_warn {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Warning 级别的日志，表示警告条件，可能导致问题但当前没有错误。

**使用示例**：
```rust
pr_warn!("Memory usage high: {}%", usage_percent);
pr_warn!("Deprecated API called: use {} instead", new_api);
```

---

### pr_notice!

**级别**：Notice (5)

**位置**：`os/src/log/macros.rs:158-167`

**签名**：
```rust
macro_rules! pr_notice {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Notice 级别的日志，表示正常但重要的信息，值得注意但不是错误。

**使用示例**：
```rust
pr_notice!("Network interface {} is up", interface_name);
pr_notice!("User {} logged in", username);
```

---

### pr_info!

**级别**：Info (6)

**位置**：`os/src/log/macros.rs:178-187`

**签名**：
```rust
macro_rules! pr_info {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Info 级别的日志，表示信息性消息，记录系统的正常操作。

**使用示例**：
```rust
pr_info!("Kernel initialized successfully");
pr_info!("Loading module: {}", module_name);
```

---

### pr_debug!

**级别**：Debug (7)

**位置**：`os/src/log/macros.rs:199-208`

**签名**：
```rust
macro_rules! pr_debug {
    ($($arg:tt)*) => { ... }
}
```

**功能**：记录 Debug 级别的日志，表示调试级别的详细信息，仅供开发和问题诊断使用。

**使用示例**：
```rust
pr_debug!("Entering function: allocate_frame()");
pr_debug!("Page table entry: PTE[{}] = {:#x}", index, value);
```

---

## 写入 API

### log_impl

**位置**：`os/src/log/mod.rs:93-95`

**签名**：
```rust
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments)
```

**功能**：日志写入的核心函数，由宏调用。直接调用此函数会绕过早期过滤，不推荐用户代码直接使用。

**参数**：
- `level: LogLevel` - 日志级别
- `args: core::fmt::Arguments` - 格式化参数（由 `format_args!` 生成）

**返回值**：无

**使用示例**：
```rust
use log::{log_impl, LogLevel};
use core::format_args;

// 不推荐直接使用，应使用宏
log_impl(LogLevel::Info, format_args!("Message: {}", value));

// 推荐使用宏，有早期过滤优化
pr_info!("Message: {}", value);
```

**注意事项**：
- 直接调用会绕过早期过滤，即使级别被禁用，格式化参数仍然会被求值
- 宏接口（`pr_*!`）会自动进行早期过滤，性能更好

---

### is_level_enabled

**位置**：`os/src/log/mod.rs:99-101`

**签名**：
```rust
pub fn is_level_enabled(level: LogLevel) -> bool
```

**功能**：检查指定的日志级别是否启用（即是否达到或超过 global_level）。宏展开时使用此函数进行早期过滤。

**参数**：
- `level: LogLevel` - 要检查的日志级别

**返回值**：
- `bool` - 如果级别启用返回 `true`，否则返回 `false`

**使用示例**：
```rust
use log::{is_level_enabled, LogLevel, pr_debug};

// 检查 Debug 级别是否启用
if is_level_enabled(LogLevel::Debug) {
    // 执行昂贵的计算
    let result = expensive_calculation();
    pr_debug!("Result: {}", result);
}

// 宏内部使用此函数进行早期过滤
// pr_info!("message") 展开为：
// if is_level_enabled(LogLevel::Info) {
//     log_impl(LogLevel::Info, format_args!("message"));
// }
```

**注意事项**：
- 此函数只检查 `global_level`，不检查 `console_level`
- 返回 `true` 表示日志会被缓存，但不一定会显示到控制台

---

## 读取 API

### read_log

**位置**：`os/src/log/mod.rs:104-106`

**签名**：
```rust
pub fn read_log() -> Option<LogEntry>
```

**功能**：从环形缓冲区读取一条日志。日志按 FIFO（先进先出）顺序返回。如果缓冲区为空，返回 `None`。

**参数**：无

**返回值**：
- `Option<LogEntry>` - 成功返回 `Some(LogEntry)`，缓冲区为空返回 `None`

**使用示例**：
```rust
use log::read_log;

// 读取单条日志
if let Some(entry) = read_log() {
    println!("{}", entry);
}

// 读取所有日志
while let Some(entry) = read_log() {
    println!("{}", entry);
}

// 处理日志条目
if let Some(entry) = read_log() {
    println!("Level: {:?}", entry.level());
    println!("CPU: {}", entry.cpu_id());
    println!("Timestamp: {}", entry.timestamp());
    println!("Message: {}", entry.message());
}
```

**注意事项**：
- 每次调用消费一条日志，下次调用返回下一条
- 只能有一个读取者（MPSC 模型），多个读取者会导致竞争条件
- 读取是非阻塞的，如果缓冲区为空立即返回 `None`

---

### log_len

**位置**：`os/src/log/mod.rs:109-111`

**签名**：
```rust
pub fn log_len() -> usize
```

**功能**：返回缓冲区中当前有多少条日志等待读取。

**参数**：无

**返回值**：
- `usize` - 缓冲区中的日志数量

**使用示例**：
```rust
use log::{log_len, read_log};

// 检查缓冲区状态
let count = log_len();
println!("Buffered logs: {}", count);

// 批量读取
if count > 0 {
    println!("Reading {} logs:", count);
    for i in 0..count {
        if let Some(entry) = read_log() {
            println!("{}: {}", i, entry);
        }
    }
}

// 检查缓冲区是否接近满
let capacity = 58;  // 缓冲区容量约 58 条
if count > capacity * 80 / 100 {
    println!("Warning: log buffer is {}% full", count * 100 / capacity);
}
```

**注意事项**：
- 返回值是快照，可能在读取过程中发生变化（其他 CPU 可能并发写入）
- 不保证能读取到返回的数量，因为可能被覆盖

---

### log_dropped_count

**位置**：`os/src/log/mod.rs:114-116`

**签名**：
```rust
pub fn log_dropped_count() -> usize
```

**功能**：返回由于缓冲区溢出而被丢弃的日志数量。这是一个累计计数，系统启动后持续增长。

**参数**：无

**返回值**：
- `usize` - 被丢弃的日志总数

**使用示例**：
```rust
use log::log_dropped_count;

// 检查是否有日志被丢弃
let dropped = log_dropped_count();
if dropped > 0 {
    pr_warn!("Warning: {} logs were dropped due to buffer overflow", dropped);
}

// 监控丢弃率
let mut last_dropped = 0;
loop {
    sleep_ms(1000);

    let current_dropped = log_dropped_count();
    let rate = current_dropped - last_dropped;
    last_dropped = current_dropped;

    if rate > 0 {
        println!("Dropping {} logs per second", rate);
    }
}

// 诊断性能问题
if log_dropped_count() > 1000 {
    pr_err!("Excessive log dropping detected, consider:");
    pr_err!("  1. Increasing buffer size (BUFFER_SIZE in config.rs)");
    pr_err!("  2. Reading logs more frequently");
    pr_err!("  3. Reducing log verbosity (increase global_level)");
}
```

**注意事项**：
- 这是累计计数，不会重置
- 非零值表示日志读取速度跟不上写入速度
- 频繁丢弃日志表示系统存在性能问题或配置不当

---

### peek_log

**位置**：`os/src/log/mod.rs:103-105`

**签名**：
```rust
pub fn peek_log(index: usize) -> Option<LogEntry>
```

**功能**：非破坏性读取：按索引 peek 日志条目，不移动读指针。允许读取缓冲区中的日志而不删除它们，主要用于 `SyslogAction::ReadAll` 操作。

**参数**：
- `index: usize` - 全局序列号（从读指针开始计数）

**返回值**：
- `Option<LogEntry>` - 成功返回 `Some(LogEntry)`，索引超出范围或条目已被覆盖返回 `None`

**使用示例**：
```rust
use log::{peek_log, log_reader_index, log_writer_index};

// 读取所有可用日志（不删除）
let start = log_reader_index();
let end = log_writer_index();

for index in start..end {
    if let Some(entry) = peek_log(index) {
        println!("{}", entry);
        // 日志仍保留在缓冲区中
    }
}

// 可以重复读取
for index in start..end {
    if let Some(entry) = peek_log(index) {
        // 再次读取相同的日志
        process_entry(&entry);
    }
}
```

**注意事项**：
- 不移除日志，可以重复读取
- 索引必须在 `[log_reader_index(), log_writer_index())` 范围内
- 如果缓冲区已满并发生覆盖，旧索引可能返回 `None`
- 并发安全：可以与 write 并发调用

---

### log_reader_index

**位置**：`os/src/log/mod.rs:108-110`

**签名**：
```rust
pub fn log_reader_index() -> usize
```

**功能**：获取当前可读取的起始索引（读指针位置）。

**参数**：无

**返回值**：
- `usize` - 当前读指针位置（全局序列号）

**使用示例**：
```rust
use log::{log_reader_index, log_writer_index, peek_log};

// 获取可读范围
let start = log_reader_index();
let end = log_writer_index();
let count = end - start;

println!("Available logs: {} (from {} to {})", count, start, end);

// 遍历所有可用日志
for index in start..end {
    if let Some(entry) = peek_log(index) {
        println!("Log #{}: {}", index, entry);
    }
}
```

**注意事项**：
- 返回值是快照，可能在使用过程中发生变化
- 配合 `log_writer_index()` 使用可以获取可读范围

---

### log_writer_index

**位置**：`os/src/log/mod.rs:113-115`

**签名**：
```rust
pub fn log_writer_index() -> usize
```

**功能**：获取当前写入位置（下一个要写入的索引）。

**参数**：无

**返回值**：
- `usize` - 当前写指针位置（全局序列号）

**使用示例**：
```rust
use log::{log_reader_index, log_writer_index};

// 计算未读日志数量
let start = log_reader_index();
let end = log_writer_index();
let unread_count = end - start;

println!("Unread logs: {}", unread_count);

// 检查缓冲区使用率
let capacity = 58;  // 缓冲区容量约 58 条
let usage_percent = (unread_count * 100) / capacity;
println!("Buffer usage: {}%", usage_percent);
```

**注意事项**：
- 返回值是快照，其他 CPU 可能并发写入导致值变化
- 配合 `log_reader_index()` 使用可以获取可读范围

---

### log_unread_bytes

**位置**：`os/src/log/mod.rs:118-120`

**签名**：
```rust
pub fn log_unread_bytes() -> usize
```

**功能**：返回未读日志的总字节数（格式化后）。精确计算所有未读日志格式化为字符串后的总字节数，用于 `SyslogAction::SizeUnread` 系统调用。

**参数**：无

**返回值**：
- `usize` - 未读日志的总字节数（格式化后）

**使用示例**：
```rust
use log::{log_len, log_unread_bytes};

// 查询缓冲区状态
let count = log_len();
let bytes = log_unread_bytes();

println!("Buffered logs: {} entries, {} bytes", count, bytes);

// 分配足够的缓冲区读取所有日志
let mut buffer = vec![0u8; bytes];
// ... 使用 syslog 系统调用读取 ...

// 检查是否需要刷新日志
if bytes > 4096 {
    println!("Log buffer has {} bytes, consider flushing", bytes);
}
```

**注意事项**：
- 返回值是精确的字节数，包括 ANSI 颜色代码、时间戳等格式化内容
- 每次 `read_log()` 会减少相应的字节数
- 并发安全：使用原子操作维护计数

---

## 配置 API

### set_global_level

**位置**：`os/src/log/mod.rs:119-121`

**签名**：
```rust
pub fn set_global_level(level: LogLevel)
```

**功能**：设置全局日志级别。低于此级别的日志会被完全忽略（宏展开时就跳过），达到或超过此级别的日志会被缓存。

**参数**：
- `level: LogLevel` - 新的全局级别

**返回值**：无

**使用示例**：
```rust
use log::{set_global_level, LogLevel};

// 缓存所有日志（包括 Debug）
set_global_level(LogLevel::Debug);

// 只缓存 Info 及以上级别（默认）
set_global_level(LogLevel::Info);

// 只缓存警告和错误
set_global_level(LogLevel::Warning);

// 只缓存错误
set_global_level(LogLevel::Error);

// 临时调整级别
let old_level = get_global_level();
set_global_level(LogLevel::Debug);
// ... 执行需要调试的代码 ...
set_global_level(old_level);
```

**注意事项**：
- 设置立即生效，影响所有后续的日志调用
- 应该小于或等于 `console_level`，否则部分缓存的日志无法显示
- 降低级别（如设置为 Error）可以减少日志开销，提高性能

---

### get_global_level

**位置**：`os/src/log/mod.rs:124-126`

**签名**：
```rust
pub fn get_global_level() -> LogLevel
```

**功能**：获取当前的全局日志级别。

**参数**：无

**返回值**：
- `LogLevel` - 当前的全局级别

**使用示例**：
```rust
use log::{get_global_level, set_global_level, LogLevel};

// 查询当前级别
let level = get_global_level();
println!("Current global level: {:?}", level);

// 保存和恢复级别
let saved_level = get_global_level();
set_global_level(LogLevel::Debug);
// ... 执行需要详细日志的代码 ...
set_global_level(saved_level);

// 条件设置
if get_global_level() > LogLevel::Info {
    println!("Info logs are disabled, enabling...");
    set_global_level(LogLevel::Info);
}
```

---

### set_console_level

**位置**：`os/src/log/mod.rs:129-131`

**签名**：
```rust
pub fn set_console_level(level: LogLevel)
```

**功能**：设置控制台日志级别。低于此级别的日志不会打印到控制台（但仍可能被缓存），达到或超过此级别的日志会立即显示。

**参数**：
- `level: LogLevel` - 新的控制台级别

**返回值**：无

**使用示例**：
```rust
use log::{set_console_level, LogLevel};

// 显示所有日志（包括 Debug）
set_console_level(LogLevel::Debug);

// 显示 Info 及以上级别
set_console_level(LogLevel::Info);

// 只显示警告和错误（默认）
set_console_level(LogLevel::Warning);

// 只显示错误
set_console_level(LogLevel::Error);

// 完全禁用控制台输出
set_console_level(LogLevel::Emergency);  // 只有 Emergency 才显示
// 或者使用一个不存在的高级别（但不推荐，使用最高级别即可）
```

**注意事项**：
- 设置立即生效，影响所有后续的日志调用
- 应该大于或等于 `global_level`，否则被过滤的日志不会被缓存
- 控制台输出较慢，提高级别可以减少串口通信开销

---

### get_console_level

**位置**：`os/src/log/mod.rs:134-136`

**签名**：
```rust
pub fn get_console_level() -> LogLevel
```

**功能**：获取当前的控制台日志级别。

**参数**：无

**返回值**：
- `LogLevel` - 当前的控制台级别

**使用示例**：
```rust
use log::{get_console_level, set_console_level, LogLevel};

// 查询当前级别
let level = get_console_level();
println!("Current console level: {:?}", level);

// 保存和恢复级别
let saved_level = get_console_level();
set_console_level(LogLevel::Info);
// ... 执行需要详细控制台输出的代码 ...
set_console_level(saved_level);

// 比较两个级别
let global = get_global_level();
let console = get_console_level();
if console < global {
    println!("Warning: console_level < global_level, some logs won't be displayed");
}
```

---

## 核心类型

### LogLevel

**位置**：`os/src/log/level.rs:23-36`

**定义**：
```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Emergency = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
}
```

**功能**：定义 8 个日志级别，数值越小优先级越高。

**方法**：

#### LogLevel::from_u8

```rust
pub fn from_u8(value: u8) -> Option<LogLevel>
```

从 u8 值创建 LogLevel，如果值无效返回 None。

**使用示例**：
```rust
use log::LogLevel;

let level = LogLevel::Info;
println!("Level: {:?}", level);

// 级别比较
if level >= LogLevel::Warning {
    println!("This is a warning or error");
}

// 从整数创建
if let Some(level) = LogLevel::from_u8(6) {
    println!("Level: {:?}", level);  // Info
}

// 使用 match
match level {
    LogLevel::Emergency | LogLevel::Alert | LogLevel::Critical => {
        println!("Critical situation!");
    }
    LogLevel::Error | LogLevel::Warning => {
        println!("Problem detected");
    }
    _ => {
        println!("Normal operation");
    }
}
```

---

### LogEntry

**位置**：`os/src/log/entry.rs:20-30`

**定义**：
```rust
#[repr(C, align(8))]
pub struct LogEntry {
    seq: AtomicUsize,
    level: LogLevel,
    cpu_id: usize,
    length: usize,
    task_id: u32,
    timestamp: usize,
    message: [u8; MAX_MESSAGE_LEN],
}
```

**功能**：表示单条日志记录，包含所有元数据和消息内容。

**方法**：

#### LogEntry::level

```rust
pub fn level(&self) -> LogLevel
```

返回日志级别。

#### LogEntry::cpu_id

```rust
pub fn cpu_id(&self) -> usize
```

返回记录日志的 CPU 核心 ID。

#### LogEntry::timestamp

```rust
pub fn timestamp(&self) -> usize
```

返回时间戳（架构相关单位）。

#### LogEntry::task_id

```rust
pub fn task_id(&self) -> u32
```

返回记录日志的任务 ID。

#### LogEntry::message

```rust
pub fn message(&self) -> &str
```

返回日志消息字符串。

**使用示例**：
```rust
use log::read_log;

if let Some(entry) = read_log() {
    // 使用 Display trait 格式化输出
    println!("{}", entry);

    // 访问各个字段
    println!("Level: {:?}", entry.level());
    println!("CPU: {}", entry.cpu_id());
    println!("Timestamp: {}", entry.timestamp());
    println!("Task: {}", entry.task_id());
    println!("Message: {}", entry.message());

    // 条件处理
    if entry.level() <= LogLevel::Error {
        // 错误日志需要特殊处理
        send_alert(&entry);
    }

    // 过滤特定 CPU 的日志
    if entry.cpu_id() == 0 {
        println!("Log from CPU 0: {}", entry.message());
    }
}
```

---

### LogCore

**位置**：`os/src/log/log_core.rs:15-20`

**定义**：
```rust
pub struct LogCore {
    buffer: GlobalLogBuffer,
    global_level: AtomicU8,
    console_level: AtomicU8,
}
```

**功能**：日志系统的核心结构，管理环形缓冲区和双过滤器。用户代码通常不直接使用此类型，而是通过 `GLOBAL_LOG` 单例和公共 API。

**全局单例**：

```rust
// 定义在 os/src/log/mod.rs:87
pub static GLOBAL_LOG: LogCore = LogCore::new();
```

**使用示例**：
```rust
// 用户代码不需要直接使用 LogCore
// 所有操作都通过公共 API 进行

// 如果需要访问全局单例（不推荐）
use log::GLOBAL_LOG;

// 但通常应该使用公共 API
use log::{pr_info, read_log, set_global_level};
pr_info!("Use public APIs instead");
```

---

## 完整示例

### 示例 1：基本日志记录

```rust
use log::*;

fn main() {
    // 配置日志级别
    set_global_level(LogLevel::Info);
    set_console_level(LogLevel::Warning);

    // 记录不同级别的日志
    pr_debug!("This won't be logged (below Info)");
    pr_info!("System starting...");
    pr_warn!("Memory usage: 80%");
    pr_err!("Failed to load module");

    // Info 被缓存但不显示（低于 Warning）
    // Warning 和 Error 被缓存并显示
}
```

### 示例 2：读取和处理日志

```rust
use log::*;

fn log_processor() {
    loop {
        // 检查缓冲区状态
        let count = log_len();
        if count > 0 {
            println!("Processing {} logs", count);

            // 读取所有日志
            while let Some(entry) = read_log() {
                // 根据级别处理
                match entry.level() {
                    LogLevel::Emergency | LogLevel::Alert | LogLevel::Critical => {
                        // 发送紧急通知
                        send_emergency_alert(&entry);
                    }
                    LogLevel::Error => {
                        // 记录到错误文件
                        write_to_error_log(&entry);
                    }
                    _ => {
                        // 正常处理
                        write_to_log_file(&entry);
                    }
                }
            }
        }

        // 检查溢出
        let dropped = log_dropped_count();
        if dropped > last_dropped {
            pr_warn!("Dropped {} logs since last check", dropped - last_dropped);
            last_dropped = dropped;
        }

        sleep_ms(100);
    }
}
```

### 示例 3：性能分析

```rust
use log::*;
use arch::timer::get_time;

fn benchmark_logging() {
    // 测试缓冲区写入性能（无控制台输出）
    set_console_level(LogLevel::Emergency);

    let start = get_time();
    for i in 0..1000 {
        pr_info!("Message {}", i);
    }
    let buffered_time = get_time() - start;

    // 测试控制台输出性能
    set_console_level(LogLevel::Info);

    let start = get_time();
    for i in 0..100 {
        pr_info!("Message {}", i);
    }
    let console_time = get_time() - start;

    pr_info!("Benchmark results:");
    pr_info!("  Buffered: {} cycles for 1000 logs ({} cycles/log)",
             buffered_time, buffered_time / 1000);
    pr_info!("  Console: {} cycles for 100 logs ({} cycles/log)",
             console_time, console_time / 100);
}
```

---

## 注意事项

### 线程安全

所有公共 API 都是线程安全的，可以在多核环境下并发调用：

- 写入 API（`log_impl`、宏）：多核并发安全，使用原子操作协调
- 读取 API（`read_log`）：只能有一个读取者（MPSC 模型）
- 配置 API（`set_*_level`、`get_*_level`）：多核并发安全，使用原子操作

### 中断上下文

Log 子系统可以在中断处理程序中安全使用：

- 无锁设计，不会导致死锁
- 固定大小分配，不使用堆内存
- 原子操作由硬件支持，不需要禁用中断

但应注意：

- 中断处理程序应该快速完成，避免大量日志记录
- 控制台输出较慢，中断中应避免触发控制台输出

### 性能考虑

- 早期过滤：被禁用级别的日志零开销（宏展开时跳过）
- 缓冲区写入：无锁，非常快（约 100-200 纳秒）
- 控制台输出：较慢，取决于串口速度（约几毫秒）

建议：

- 热路径使用 `pr_debug!`，生产环境禁用 Debug 级别
- 提高 `console_level` 减少控制台输出
- 定期读取日志避免缓冲区溢出

---

## 系统调用

### syslog

**位置**：`os/src/kernel/syscall/sys.rs:127-378`

**签名**：
```rust
pub fn syslog(type_: i32, bufp: *mut u8, len: i32) -> isize
```

**功能**：读取和控制内核日志缓冲区。完全兼容 Linux `syslog(2)` 系统调用,允许用户空间程序查询、读取和控制内核日志。

**参数**：
- `type_: i32` - 操作类型 (0-10),详见 `SyslogAction`
- `bufp: *mut u8` - 用户空间缓冲区指针（某些操作需要）
- `len: i32` - 缓冲区长度或命令参数（取决于操作类型）

**返回值**：
* **成功**：
  - 类型 2/3/4: 读取的字节数
  - 类型 8: 旧的 console_loglevel (1-8)
  - 类型 9: 未读字节数
  - 类型 10: 缓冲区总大小
  - 其他: 0
* **失败**：负的 errno
  - `-EINVAL`: 无效参数
  - `-EPERM`: 权限不足
  - `-EINTR`: 被信号中断
  - `-EFAULT`: 无效的用户空间指针

**操作类型** (`SyslogAction`):

| 值 | 名称 | 描述 |
|----|------|------|
| 0 | CLOSE | 关闭日志（NOP） |
| 1 | OPEN | 打开日志（NOP） |
| 2 | READ | 破坏性读取日志 |
| 3 | READ_ALL | 非破坏性读取所有日志 |
| 4 | READ_CLEAR | 读取并清空日志 |
| 5 | CLEAR | 清空日志缓冲区 |
| 6 | CONSOLE_OFF | 禁用控制台输出 |
| 7 | CONSOLE_ON | 启用控制台输出 |
| 8 | CONSOLE_LEVEL | 设置控制台日志级别 |
| 9 | SIZE_UNREAD | 查询未读字节数 |
| 10 | SIZE_BUFFER | 查询缓冲区总大小 |

**使用示例**：

```c
#include <sys/klog.h>
#include <sys/syscall.h>
#include <unistd.h>

// 1. 读取内核日志（破坏性）
char buf[8192];
int len = syscall(SYS_syslog, 2, buf, sizeof(buf));
if (len > 0) {
    write(STDOUT_FILENO, buf, len);
}

// 2. 读取所有日志（非破坏性）
len = syscall(SYS_syslog, 3, buf, sizeof(buf));

// 3. 查询未读字节数
int unread = syscall(SYS_syslog, 9, NULL, 0);
printf("Unread bytes: %d\n", unread);

// 4. 查询缓冲区总大小
int size = syscall(SYS_syslog, 10, NULL, 0);
printf("Buffer size: %d\n", size);

// 5. 设置控制台日志级别（1-8）
// 返回旧的级别
int old_level = syscall(SYS_syslog, 8, NULL, 5);  // 设置为 5 (Notice)
printf("Old level: %d\n", old_level);

// 6. 清空日志缓冲区
syscall(SYS_syslog, 5, NULL, 0);

// 7. 禁用控制台输出
syscall(SYS_syslog, 6, NULL, 0);

// 8. 启用控制台输出
syscall(SYS_syslog, 7, NULL, 0);
```

**日志级别映射**:

Linux `console_loglevel` 使用 1-8 的值,其中数值越小优先级越高：
- `console_loglevel = N` 表示显示级别 < N 的消息
- Comix 内部使用 0-7 (LogLevel::Emergency 到 Debug)
- 转换公式：`comix_level = linux_level - 1`

| Linux Level | Comix Level | 显示级别 |
|-------------|-------------|----------|
| 1 | 0 (Emergency) | 只显示 Emergency |
| 2 | 1 (Alert) | Emergency, Alert |
| 3 | 2 (Critical) | Emergency, Alert, Critical |
| 4 | 3 (Error) | Emergency ~ Error |
| 5 | 4 (Warning) | Emergency ~ Warning |
| 6 | 5 (Notice) | Emergency ~ Notice |
| 7 | 6 (Info) | Emergency ~ Info |
| 8 | 7 (Debug) | 显示所有级别 |

**权限要求**：

1. **特殊情况：ReadAll 和 SizeBuffer**
   - 如果 `dmesg_restrict == 0`：允许所有用户访问
   - 如果 `dmesg_restrict != 0`：需要特权
2. **其他操作**：需要以下任一权限：
   - `euid == 0` (root 用户)
   - `CAP_SYSLOG` (推荐)
   - `CAP_SYS_ADMIN` (向后兼容)

**注意事项**：
- READ (类型 2) 是破坏性的，读取后日志从缓冲区删除
- READ_ALL (类型 3) 是非破坏性的，可以重复读取
- CONSOLE_LEVEL 的参数范围是 1-8，超出范围返回 `-EINVAL`
- SIZE_UNREAD 返回的是精确的格式化后字节数，可用于分配缓冲区
- 当前权限检查未完全实现，等待用户管理系统完善

**用户空间工具示例** (`dmesg` 实现):

```c
// 简化的 dmesg 工具实现
#include <stdio.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <unistd.h>

#define SYSLOG_ACTION_READ_ALL 3
#define SYSLOG_ACTION_SIZE_UNREAD 9

int main() {
    // 查询需要多少空间
    int size = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_UNREAD, NULL, 0);
    if (size < 0) {
        perror("syslog");
        return 1;
    }

    // 分配缓冲区
    char *buf = malloc(size + 1);
    if (!buf) {
        perror("malloc");
        return 1;
    }

    // 读取所有日志（非破坏性）
    int len = syscall(SYS_syslog, SYSLOG_ACTION_READ_ALL, buf, size);
    if (len < 0) {
        perror("syslog");
        free(buf);
        return 1;
    }

    // 显示日志
    buf[len] = '\0';
    printf("%s", buf);

    free(buf);
    return 0;
}
```

---

## 相关文档

- [整体架构](architecture.md) - Log 子系统的设计和架构
- [使用指南](usage.md) - 详细的使用示例和最佳实践，包括 syslog 使用
- [日志级别](level.md) - 日志级别的语义和使用建议
- [缓冲区和条目](buffer_and_entry.md) - 环形缓冲区和日志条目的实现细节
