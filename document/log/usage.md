# Log 子系统使用指南

## 概述

本文档提供 Log 子系统的实用指南，包括基本使用、配置方法、格式化技巧、性能最佳实践、多核并发场景、常见陷阱和调试技巧。通过丰富的代码示例，帮助开发者快速掌握日志系统的使用。

## 基本使用

### 引入宏

在需要使用日志的模块中引入对应的宏：

```rust
use log::{pr_info, pr_err, pr_warn, pr_debug};
```

或者引入所有宏：

```rust
use log::*;
```

### 记录简单日志

最基本的用法是记录字符串消息：

```rust
pr_info!("System initialization started");
pr_warn!("Low memory warning");
pr_err!("Failed to initialize device");
```

### 格式化日志

使用 Rust 的格式化语法，类似于 `println!` 和 `format!`：

```rust
let pid = 42;
let name = "init";
pr_info!("Process started: pid={}, name={}", pid, name);

let count = 100;
pr_debug!("Allocated {} frames", count);

let addr = 0x80001000usize;
pr_info!("Page table at {:#x}", addr);  // 十六进制格式
```

### 不同级别的使用示例

```rust
// Emergency: 系统即将崩溃
pr_emerg!("Kernel panic: unable to handle page fault");

// Alert: 需要立即采取行动
pr_alert!("Filesystem corruption detected");

// Critical: 严重错误
pr_crit!("Failed to initialize memory subsystem");

// Error: 普通错误
pr_err!("Cannot open file: {}", filename);

// Warning: 警告
pr_warn!("Memory usage: {}%", usage_percent);

// Notice: 重要信息
pr_notice!("Network interface {} is up", interface);

// Info: 常规信息
pr_info!("Loading module: {}", module_name);

// Debug: 调试信息
pr_debug!("Entering function: allocate_frame()");
```

## 配置级别过滤器

### 查询当前级别

```rust
use log::{get_global_level, get_console_level, LogLevel};

let global = get_global_level();
let console = get_console_level();

pr_info!("Current levels: global={:?}, console={:?}", global, console);
```

### 设置全局级别

控制哪些日志被缓存：

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
```

### 设置控制台级别

控制哪些日志立即打印：

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
```

### 典型配置场景

#### 开发调试配置

```rust
// 缓存所有日志，但只显示 Info 及以上
// 这样 Debug 日志被保留，需要时可以读取缓冲区查看
set_global_level(LogLevel::Debug);
set_console_level(LogLevel::Info);

pr_debug!("This will be buffered but not shown");
pr_info!("This will be buffered and shown");
```

#### 正常运行配置

```rust
// 默认配置：缓存常规信息，只显示警告和错误
set_global_level(LogLevel::Info);
set_console_level(LogLevel::Warning);

pr_info!("Normal operation");      // 缓存，不显示
pr_warn!("Warning condition");     // 缓存，显示
```

#### 生产环境配置

```rust
// 最小化开销：只记录和显示问题
set_global_level(LogLevel::Warning);
set_console_level(LogLevel::Error);

pr_info!("This will be ignored");    // 完全跳过
pr_warn!("Warning");                 // 缓存，不显示
pr_err!("Error");                    // 缓存，显示
```

#### 临时启用详细日志

```rust
// 调试特定问题时，临时启用所有日志
let old_global = get_global_level();
let old_console = get_console_level();

set_global_level(LogLevel::Debug);
set_console_level(LogLevel::Debug);

// ... 执行需要调试的代码 ...

// 恢复原来的配置
set_global_level(old_global);
set_console_level(old_console);
```

## 读取日志缓冲区

### 读取单条日志

```rust
use log::read_log;

if let Some(entry) = read_log() {
    // 使用 Display trait 格式化输出
    println!("{}", entry);

    // 或者访问字段
    println!("Level: {:?}", entry.level());
    println!("CPU: {}", entry.cpu_id());
    println!("Timestamp: {}", entry.timestamp());
    println!("Message: {}", entry.message());
}
```

### 读取所有日志

```rust
use log::read_log;

// 顺序读取所有日志（FIFO 顺序）
while let Some(entry) = read_log() {
    println!("{}", entry);
}
```

### 检查缓冲区状态

```rust
use log::{log_len, log_dropped_count};

// 检查有多少条日志等待读取
let buffered_count = log_len();
println!("Buffered logs: {}", buffered_count);

// 检查是否有日志被丢弃（缓冲区溢出）
let dropped = log_dropped_count();
if dropped > 0 {
    pr_warn!("Warning: {} logs were dropped due to buffer overflow", dropped);
}
```

### 定期读取日志（避免溢出）

```rust
use log::{read_log, log_len, log_dropped_count};

// 日志读取任务（可以在内核线程中运行）
fn log_reader_task() {
    loop {
        // 定期检查缓冲区
        let count = log_len();
        if count > 0 {
            println!("=== Reading {} buffered logs ===", count);

            while let Some(entry) = read_log() {
                // 处理日志（打印、写入文件、发送到网络等）
                process_log_entry(entry);
            }
        }

        // 检查溢出
        let dropped = log_dropped_count();
        if dropped > 0 {
            println!("WARNING: {} logs were dropped", dropped);
        }

        // 休眠一段时间
        sleep_ms(1000);
    }
}

fn process_log_entry(entry: LogEntry) {
    // 示例：写入到文件或发送到远程服务器
    // file.write_fmt(format_args!("{}\n", entry)).ok();
    println!("{}", entry);
}
```

### 非破坏性读取

使用 `peek_log()` 可以读取日志而不删除它们：

```rust
use log::{peek_log, log_reader_index, log_writer_index};

// 获取可读范围
let start = log_reader_index();
let end = log_writer_index();

println!("Available logs: {}", end - start);

// 遍历所有日志（不删除）
for index in start..end {
    if let Some(entry) = peek_log(index) {
        println!("Log #{}: {}", index, entry);
    }
}

// 可以再次读取相同的日志
for index in start..end {
    if let Some(entry) = peek_log(index) {
        // 处理日志，但它们仍保留在缓冲区中
        if entry.level() <= LogLevel::Error {
            send_alert(&entry);
        }
    }
}
```

### 查询缓冲区状态

```rust
use log::{log_len, log_unread_bytes, log_dropped_count};

// 查询未读日志数量和字节数
let count = log_len();
let bytes = log_unread_bytes();
let dropped = log_dropped_count();

println!("Buffered logs: {} entries, {} bytes", count, bytes);
println!("Dropped logs: {}", dropped);

// 检查缓冲区使用率
let capacity = 58;  // 约58条
let usage_percent = (count * 100) / capacity;
if usage_percent > 80 {
    pr_warn!("Log buffer is {}% full", usage_percent);
}
```

## 使用 syslog 系统调用

用户空间程序可以通过 `syslog` 系统调用读取和控制内核日志。

### 基本用法

```c
#include <sys/klog.h>
#include <sys/syscall.h>
#include <stdio.h>
#include <unistd.h>

int main() {
    char buf[8192];

    // 读取内核日志（破坏性）
    int len = syscall(SYS_syslog, 2, buf, sizeof(buf));
    if (len > 0) {
        write(STDOUT_FILENO, buf, len);
    }

    return 0;
}
```

### 非破坏性读取

```c
#include <sys/klog.h>
#include <sys/syscall.h>
#include <stdio.h>
#include <stdlib.h>

#define SYSLOG_ACTION_READ_ALL 3
#define SYSLOG_ACTION_SIZE_UNREAD 9

int main() {
    // 1. 查询需要多少空间
    int size = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_UNREAD, NULL, 0);
    if (size < 0) {
        perror("syslog");
        return 1;
    }

    printf("Unread logs: %d bytes\n", size);

    // 2. 分配足够的缓冲区
    char *buf = malloc(size + 1);
    if (!buf) {
        perror("malloc");
        return 1;
    }

    // 3. 读取所有日志（非破坏性）
    int len = syscall(SYS_syslog, SYSLOG_ACTION_READ_ALL, buf, size);
    if (len < 0) {
        perror("syslog");
        free(buf);
        return 1;
    }

    // 4. 显示日志
    buf[len] = '\0';
    printf("%s", buf);

    free(buf);
    return 0;
}
```

### 控制控制台输出

```c
#define SYSLOG_ACTION_CONSOLE_OFF 6
#define SYSLOG_ACTION_CONSOLE_ON 7
#define SYSLOG_ACTION_CONSOLE_LEVEL 8

// 禁用控制台输出（只记录到缓冲区）
syscall(SYS_syslog, SYSLOG_ACTION_CONSOLE_OFF, NULL, 0);

// 启用控制台输出
syscall(SYS_syslog, SYSLOG_ACTION_CONSOLE_ON, NULL, 0);

// 设置控制台级别为 Warning (4)
int old_level = syscall(SYS_syslog, SYSLOG_ACTION_CONSOLE_LEVEL, NULL, 5);
printf("Old console level: %d\n", old_level);
```

### 清空日志缓冲区

```c
#define SYSLOG_ACTION_CLEAR 5

// 清空所有日志
int ret = syscall(SYS_syslog, SYSLOG_ACTION_CLEAR, NULL, 0);
if (ret < 0) {
    perror("syslog");
}
```

### 实现 dmesg 工具

完整的 `dmesg` 工具实现示例：

```c
// dmesg.c - 简化的 dmesg 实现
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <errno.h>

#define SYSLOG_ACTION_READ 2
#define SYSLOG_ACTION_READ_ALL 3
#define SYSLOG_ACTION_READ_CLEAR 4
#define SYSLOG_ACTION_CLEAR 5
#define SYSLOG_ACTION_CONSOLE_LEVEL 8
#define SYSLOG_ACTION_SIZE_UNREAD 9
#define SYSLOG_ACTION_SIZE_BUFFER 10

static void usage(const char *prog) {
    fprintf(stderr, "Usage: %s [options]\n", prog);
    fprintf(stderr, "Options:\n");
    fprintf(stderr, "  -c        Clear the ring buffer\n");
    fprintf(stderr, "  -C        Clear after reading\n");
    fprintf(stderr, "  -r        Print raw (do not consume)\n");
    fprintf(stderr, "  -n <level> Set console log level (1-8)\n");
    fprintf(stderr, "  -s        Show buffer size\n");
    exit(1);
}

int main(int argc, char *argv[]) {
    int opt;
    int action = SYSLOG_ACTION_READ_ALL;  // 默认非破坏性读取
    int clear_only = 0;
    int show_size = 0;
    int set_level = 0;
    int level = 0;

    // 解析命令行参数
    while ((opt = getopt(argc, argv, "cCrn:s")) != -1) {
        switch (opt) {
            case 'c':
                clear_only = 1;
                break;
            case 'C':
                action = SYSLOG_ACTION_READ_CLEAR;
                break;
            case 'r':
                action = SYSLOG_ACTION_READ_ALL;
                break;
            case 'n':
                set_level = 1;
                level = atoi(optarg);
                if (level < 1 || level > 8) {
                    fprintf(stderr, "Invalid level: %d (must be 1-8)\n", level);
                    return 1;
                }
                break;
            case 's':
                show_size = 1;
                break;
            default:
                usage(argv[0]);
        }
    }

    // 设置控制台级别
    if (set_level) {
        int ret = syscall(SYS_syslog, SYSLOG_ACTION_CONSOLE_LEVEL, NULL, level);
        if (ret < 0) {
            perror("syslog");
            return 1;
        }
        printf("Console level set to %d (was %d)\n", level, ret);
        if (!show_size && !clear_only && action == SYSLOG_ACTION_READ_ALL) {
            return 0;  // 只设置级别，不读取日志
        }
    }

    // 显示缓冲区大小
    if (show_size) {
        int size = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_BUFFER, NULL, 0);
        int unread = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_UNREAD, NULL, 0);
        if (size < 0 || unread < 0) {
            perror("syslog");
            return 1;
        }
        printf("Buffer size: %d bytes\n", size);
        printf("Unread: %d bytes\n", unread);
        return 0;
    }

    // 清空缓冲区
    if (clear_only) {
        int ret = syscall(SYS_syslog, SYSLOG_ACTION_CLEAR, NULL, 0);
        if (ret < 0) {
            perror("syslog");
            return 1;
        }
        return 0;
    }

    // 查询未读字节数
    int size = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_UNREAD, NULL, 0);
    if (size < 0) {
        perror("syslog");
        return 1;
    }

    if (size == 0) {
        // 没有日志
        return 0;
    }

    // 分配缓冲区
    char *buf = malloc(size + 1);
    if (!buf) {
        perror("malloc");
        return 1;
    }

    // 读取日志
    int len = syscall(SYS_syslog, action, buf, size);
    if (len < 0) {
        perror("syslog");
        free(buf);
        return 1;
    }

    // 输出日志
    if (len > 0) {
        buf[len] = '\0';
        printf("%s", buf);
    }

    free(buf);
    return 0;
}
```

**编译和使用**：

```bash
# 编译
gcc -o dmesg dmesg.c

# 查看内核日志
./dmesg

# 查看并清空
./dmesg -C

# 只清空
./dmesg -c

# 显示缓冲区状态
./dmesg -s

# 设置控制台级别
./dmesg -n 5  # 设置为 Notice
```

### syslog 操作类型完整列表

| 值 | 宏定义 | 描述 | 参数 |
|----|--------|------|------|
| 0 | SYSLOG_ACTION_CLOSE | 关闭日志（NOP） | - |
| 1 | SYSLOG_ACTION_OPEN | 打开日志（NOP） | - |
| 2 | SYSLOG_ACTION_READ | 破坏性读取 | buf, len |
| 3 | SYSLOG_ACTION_READ_ALL | 非破坏性读取 | buf, len |
| 4 | SYSLOG_ACTION_READ_CLEAR | 读取并清空 | buf, len |
| 5 | SYSLOG_ACTION_CLEAR | 清空缓冲区 | - |
| 6 | SYSLOG_ACTION_CONSOLE_OFF | 禁用控制台 | - |
| 7 | SYSLOG_ACTION_CONSOLE_ON | 启用控制台 | - |
| 8 | SYSLOG_ACTION_CONSOLE_LEVEL | 设置级别 | len (1-8) |
| 9 | SYSLOG_ACTION_SIZE_UNREAD | 查询未读字节 | - |
| 10 | SYSLOG_ACTION_SIZE_BUFFER | 查询缓冲区大小 | - |

## 格式化复杂数据

### 基本格式化选项

```rust
let value = 42;

// 十进制
pr_info!("Value: {}", value);         // Value: 42

// 十六进制
pr_info!("Value: {:#x}", value);      // Value: 0x2a

// 二进制
pr_info!("Value: {:#b}", value);      // Value: 0b101010

// 指定宽度
pr_info!("Value: {:08x}", value);     // Value: 0000002a
```

### 格式化指针和地址

```rust
let addr = 0x80000000usize;
let ptr: *const u8 = 0x80001000 as *const u8;

pr_info!("Physical address: {:#x}", addr);
pr_info!("Pointer: {:p}", ptr);
pr_debug!("Page table entry: PTE[{}] = {:#018x}", index, pte_value);
```

### 格式化多个参数

```rust
let start_addr = 0x80000000usize;
let end_addr = 0x80001000usize;
let size = end_addr - start_addr;

pr_info!("Memory region: {:#x} - {:#x}, size = {} bytes",
         start_addr, end_addr, size);
```

### 使用 Debug trait

```rust
use core::fmt::Debug;

#[derive(Debug)]
struct Frame {
    ppn: usize,
    flags: u8,
}

let frame = Frame { ppn: 0x80000, flags: 0x7 };

// 使用 {:?} 格式化
pr_debug!("Allocated frame: {:?}", frame);
// 输出：Allocated frame: Frame { ppn: 524288, flags: 7 }

// 使用 {:#?} 格式化（多行美化）
pr_debug!("Frame details: {:#?}", frame);
// 输出：Frame details: Frame {
//     ppn: 524288,
//     flags: 7,
// }
```

### 条件格式化

```rust
let result: Result<usize, &str> = Err("out of memory");

match result {
    Ok(value) => pr_info!("Operation succeeded: value = {}", value),
    Err(e) => pr_err!("Operation failed: {}", e),
}

// 或者使用更简洁的方式
pr_info!("Result: {:?}", result);
```

### 格式化字符串切片

```rust
let name = "hello.txt";
let message = b"Hello, world!";

pr_info!("Filename: {}", name);
pr_debug!("Message: {:?}", message);  // 字节数组

// UTF-8 字符串
let utf8_str = core::str::from_utf8(message).unwrap();
pr_info!("Content: {}", utf8_str);
```

## 性能最佳实践

### 避免格式化被禁用的日志

早期过滤会自动处理，但了解其工作原理有助于编写高效代码：

```rust
// 好：使用宏，自动早期过滤
pr_debug!("Value: {}", expensive_calculation());
// 如果 Debug 被禁用，expensive_calculation() 不会被调用

// 坏：手动调用 log_impl，无早期过滤
use log::{log_impl, LogLevel};
log_impl(LogLevel::Debug, format_args!("Value: {}", expensive_calculation()));
// expensive_calculation() 总是被调用，即使 Debug 被禁用
```

### 热路径中的日志

在性能关键的代码路径中，即使是早期过滤也有微小开销：

```rust
// 方案 1：使用条件编译（推荐）
#[cfg(debug_assertions)]
pr_debug!("Processing item {}", i);

// 方案 2：减少日志频率
if i % 1000 == 0 {
    pr_debug!("Processed {} items", i);
}

// 方案 3：使用更低的级别
// 如果日志不是必需的，考虑完全移除
```

### 避免在中断处理程序中大量记录日志

中断处理程序应该快速完成，避免阻塞系统：

```rust
// 中断处理程序
fn timer_interrupt_handler() {
    // 好：只记录关键错误
    if critical_error {
        pr_err!("Timer interrupt error");
    }

    // 坏：记录每次中断（会严重影响性能）
    // pr_debug!("Timer interrupt fired");  // 不要这样做！
}
```

### 控制台输出的性能影响

控制台输出（串口通信）比缓冲区写入慢得多：

```rust
// 性能测试示例
use arch::timer::get_time;

let start = get_time();

// 1000 次缓冲区写入（不输出到控制台）
set_console_level(LogLevel::Emergency);  // 禁用控制台
for i in 0..1000 {
    pr_info!("Message {}", i);
}

let buffered_time = get_time() - start;

// 1000 次控制台输出
set_console_level(LogLevel::Info);  // 启用控制台
let start = get_time();
for i in 0..1000 {
    pr_info!("Message {}", i);
}

let console_time = get_time() - start;

pr_info!("Buffered: {} cycles, Console: {} cycles",
         buffered_time, console_time);
// 预期：console_time >> buffered_time（可能是 100 倍以上）
```

### 消息长度优化

避免超过 256 字节的消息：

```rust
// 好：简洁的日志
pr_info!("File opened: {}", filename);

// 坏：过长的日志（会被截断）
pr_info!("File opened with following properties: name={}, size={}, \
          permissions={}, owner={}, group={}, created={}, modified={}, \
          accessed={}, ... [very long message]", ...);

// 更好：分多条日志
pr_info!("File opened: {}", filename);
pr_debug!("File size: {} bytes", size);
pr_debug!("File owner: uid={}, gid={}", uid, gid);
```

## 多核并发场景

### 多核并发写入

Log 子系统是并发安全的，多个 CPU 可以同时记录日志：

```rust
// CPU 0
fn task_on_cpu0() {
    pr_info!("[CPU0] Starting task A");
    // ... 执行任务 ...
    pr_info!("[CPU0] Task A completed");
}

// CPU 1
fn task_on_cpu1() {
    pr_info!("[CPU1] Starting task B");
    // ... 执行任务 ...
    pr_info!("[CPU1] Task B completed");
}

// 两个 CPU 可以同时调用 pr_info!，无需担心竞争条件
// 日志会按照时间戳顺序记录到缓冲区
```

### 日志中包含 CPU ID

日志条目自动包含 CPU ID，帮助追踪多核执行：

```rust
// 在不同 CPU 上执行
for i in 0..100 {
    pr_debug!("Processing item {}", i);
}

// 输出示例：
// [000012345678] [DEBUG] [CPU0/Task1] Processing item 0
// [000012345679] [DEBUG] [CPU1/Task2] Processing item 1
// [000012345680] [DEBUG] [CPU0/Task1] Processing item 2
// [000012345681] [DEBUG] [CPU2/Task3] Processing item 3
```

### 使用时间戳分析并发行为

```rust
pr_info!("Task started");
// ... 执行任务 ...
pr_info!("Task completed");

// 读取日志后，通过时间戳计算执行时间
// [000012000000] [INFO] [CPU0/Task1] Task started
// [000012005000] [INFO] [CPU0/Task1] Task completed
// 执行时间：5000 个时钟周期
```

### 竞态条件的调试

```rust
// 使用日志追踪竞态条件
static SHARED_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn increment_counter() {
    let old = SHARED_COUNTER.fetch_add(1, Ordering::SeqCst);
    pr_debug!("Counter: {} -> {}", old, old + 1);
}

// 多个 CPU 并发调用 increment_counter()
// 日志会显示每个 CPU 看到的值和顺序
```

## 常见陷阱

### 陷阱 1：消息被截断

**问题**：消息超过 256 字节会被截断

```rust
// 错误示例：超长消息
let long_string = "a".repeat(300);
pr_info!("Data: {}", long_string);
// 只会记录前 256 字节，后面的内容丢失
```

**解决方案**：分多条日志记录

```rust
// 正确做法：分段记录
let data = vec![1, 2, 3, /* ... 很多数据 */];
pr_info!("Data (total {} items):", data.len());
for (i, chunk) in data.chunks(10).enumerate() {
    pr_debug!("  Chunk {}: {:?}", i, chunk);
}
```

### 陷阱 2：忘记读取日志导致缓冲区溢出

**问题**：日志写入速度超过读取速度，旧日志被覆盖

```rust
// 持续写入日志，但从不读取
for i in 0..1000 {
    pr_info!("Message {}", i);
}

// 缓冲区只能容纳约 60 条日志
// 早期的日志（0-940）会被覆盖，只能读取到后 60 条
```

**解决方案**：定期读取日志

```rust
// 创建日志读取任务
fn log_reader() {
    loop {
        while let Some(entry) = read_log() {
            // 处理日志（打印、存储等）
            handle_log(entry);
        }
        sleep_ms(100);  // 每 100ms 读取一次
    }
}
```

### 陷阱 3：在日志中使用昂贵的计算

**问题**：即使有早期过滤，但如果计算在宏参数中，仍然会执行

```rust
// 错误示例：昂贵的计算
pr_debug!("Hash: {}", compute_expensive_hash(&data));
// 即使 Debug 被禁用，compute_expensive_hash 仍然会被调用！
```

**解决方案**：先检查级别再计算

```rust
// 正确做法：条件计算
use log::is_level_enabled;
if is_level_enabled(LogLevel::Debug) {
    let hash = compute_expensive_hash(&data);
    pr_debug!("Hash: {}", hash);
}

// 或者使用条件编译
#[cfg(debug_assertions)]
{
    let hash = compute_expensive_hash(&data);
    pr_debug!("Hash: {}", hash);
}
```

**注意**：这个陷阱是 Rust 宏的特性决定的，宏参数在宏展开前求值。

### 陷阱 4：日志级别配置不当

**问题**：global_level 低于 console_level，导致部分日志无法显示

```rust
// 错误配置
set_global_level(LogLevel::Warning);  // 只缓存 Warning 及以上
set_console_level(LogLevel::Info);    // 期望显示 Info 及以上

pr_info!("This message will not appear!");
// Info < Warning，不会被缓存，console_level 无效
```

**解决方案**：确保 global_level <= console_level

```rust
// 正确配置
set_global_level(LogLevel::Info);      // 缓存 Info 及以上
set_console_level(LogLevel::Warning);  // 显示 Warning 及以上

pr_info!("This will be buffered but not shown");
pr_warn!("This will be buffered and shown");
```

### 陷阱 5：在 panic handler 中记录日志

**问题**：panic handler 可能在不稳定状态下运行，日志系统可能无法正常工作

```rust
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // 谨慎使用日志，此时系统状态可能不一致
    // pr_emerg! 是最安全的选择
    pr_emerg!("Kernel panic: {}", info);

    // 不要尝试读取日志缓冲区或复杂操作
    // 直接 shutdown 或进入死循环
    loop {}
}
```

## 调试技巧

### 追踪函数调用

```rust
fn allocate_frame() -> Result<Frame, Error> {
    pr_debug!(">>> Entering allocate_frame()");

    let result = do_allocate();

    match &result {
        Ok(frame) => pr_debug!("<<< allocate_frame() -> Ok(Frame {{ ppn: {:#x} }})", frame.ppn),
        Err(e) => pr_debug!("<<< allocate_frame() -> Err({:?})", e),
    }

    result
}
```

### 使用条件日志

```rust
// 只在特定条件下记录日志
if unlikely_condition {
    pr_warn!("Rare condition occurred: {}", details);
}

// 使用断言 + 日志
debug_assert!({
    pr_debug!("Assertion check: value = {}", value);
    value > 0
});
```

### 性能分析

```rust
use arch::timer::get_time;

fn performance_critical_function() {
    let start = get_time();

    // ... 执行代码 ...

    let elapsed = get_time() - start;
    pr_debug!("Function took {} cycles", elapsed);
}
```

### 状态转换日志

```rust
#[derive(Debug)]
enum State {
    Idle,
    Running,
    Blocked,
    Terminated,
}

fn set_task_state(task: &mut Task, new_state: State) {
    pr_debug!("Task {} state: {:?} -> {:?}", task.id, task.state, new_state);
    task.state = new_state;
}
```

### 使用日志分析死锁

```rust
// 记录锁的获取和释放
pr_debug!("Trying to acquire lock: {}", lock_name);
lock.acquire();
pr_debug!("Acquired lock: {}", lock_name);

// ... 临界区代码 ...

pr_debug!("Releasing lock: {}", lock_name);
lock.release();
pr_debug!("Released lock: {}", lock_name);

// 如果系统挂起，查看日志可以发现哪个锁未被释放
```

### 内存泄漏追踪

```rust
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static FREE_COUNT: AtomicUsize = AtomicUsize::new(0);

fn allocate() -> *mut u8 {
    let ptr = do_allocate();
    let count = ALLOC_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    pr_debug!("Allocated: {:p}, total allocations: {}", ptr, count);
    ptr
}

fn deallocate(ptr: *mut u8) {
    let count = FREE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    pr_debug!("Freed: {:p}, total frees: {}", ptr, count);
    do_free(ptr);
}

// 定期检查
fn check_memory_leaks() {
    let allocs = ALLOC_COUNT.load(Ordering::Relaxed);
    let frees = FREE_COUNT.load(Ordering::Relaxed);
    if allocs != frees {
        pr_warn!("Potential memory leak: {} allocations, {} frees", allocs, frees);
    }
}
```

### 使用日志辅助 GDB 调试

```rust
// 在关键点记录日志
pr_info!("Checkpoint A: value = {}", value);

// 在 GDB 中设置断点：
// (gdb) break os/src/module.rs:123
// (gdb) condition 1 value == 42

// 结合日志查看执行流程
```

## 示例：完整的日志使用场景

### 文件系统操作日志

```rust
fn open_file(path: &str, flags: u32) -> Result<FileDescriptor, Error> {
    pr_info!("Opening file: {}, flags: {:#x}", path, flags);

    // 查找文件
    pr_debug!("Looking up inode for: {}", path);
    let inode = match lookup_inode(path) {
        Ok(inode) => {
            pr_debug!("Found inode: {}", inode.number);
            inode
        }
        Err(e) => {
            pr_err!("Failed to lookup file {}: {:?}", path, e);
            return Err(e);
        }
    };

    // 检查权限
    pr_debug!("Checking permissions for inode {}", inode.number);
    if !check_permissions(&inode, flags) {
        pr_warn!("Permission denied: {}, uid={}", path, current_uid());
        return Err(Error::PermissionDenied);
    }

    // 分配文件描述符
    let fd = allocate_fd(inode)?;
    pr_info!("File opened successfully: {} -> fd {}", path, fd);

    Ok(fd)
}
```

### 进程调度日志

```rust
fn schedule() -> ! {
    loop {
        // 选择下一个任务
        let next_task = scheduler::pick_next_task();

        pr_debug!("Scheduling: CPU{} switching to task {} ({})",
                 current_cpu_id(), next_task.id, next_task.name);

        // 上下文切换
        let prev_task = current_task();
        pr_debug!("Context switch: task {} -> task {}",
                 prev_task.id, next_task.id);

        context_switch(prev_task, next_task);

        // 任务恢复执行后
        pr_debug!("Task {} resumed", current_task().id);
    }
}
```

### 内存管理日志

```rust
fn allocate_pages(count: usize) -> Result<PhysAddr, Error> {
    pr_debug!("Allocating {} pages", count);

    // 检查可用内存
    let free_pages = get_free_page_count();
    pr_debug!("Free pages: {}, requested: {}", free_pages, count);

    if free_pages < count {
        pr_warn!("Low memory: {} pages free, {} requested",
                free_pages, count);

        // 尝试回收内存
        pr_info!("Attempting memory reclaim");
        reclaim_pages();

        let free_pages = get_free_page_count();
        if free_pages < count {
            pr_err!("Out of memory: {} pages free, {} requested",
                   free_pages, count);
            return Err(Error::OutOfMemory);
        }
    }

    let addr = frame_allocator::allocate(count)?;
    pr_debug!("Allocated pages: {:#x}, count: {}", addr, count);

    Ok(addr)
}
```

## 总结

Log 子系统提供了强大而灵活的日志功能：

- **8 个宏**覆盖不同的严重程度
- **双过滤器**平衡日志完整性和实时性
- **无锁设计**支持多核并发
- **早期过滤**保证性能
- **丰富的格式化**支持复杂数据

遵循本文档的最佳实践，可以有效利用日志系统进行开发、调试和问题诊断。
