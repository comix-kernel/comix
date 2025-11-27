# Log 子系统文档

## 简介

Log 子系统是 Comix 内核的日志记录系统，提供类似 Linux 内核 `printk` 的日志功能。该系统专为裸机环境设计，采用无锁并发架构，支持多核环境下的高效日志记录。

Log 子系统的核心特点是**双路输出策略**：日志既会被缓存到环形缓冲区供后续读取，又可以根据级别立即输出到控制台。这种设计平衡了实时监控和日志持久化的需求，使得开发者既能在运行时观察关键信息，又能在事后分析完整的日志记录。

作为内核基础设施的一部分，Log 子系统在系统启动早期即可使用，无需复杂的初始化过程。它采用编译期初始化的单例模式，保证零运行时开销，并且完全避免动态内存分配，适合在资源受限的裸机环境中运行。

### 主要功能

- **分级日志记录**：提供 8 个级别的日志分类（Emergency、Alert、Critical、Error、Warning、Notice、Info、Debug），模仿 Linux 内核的日志级别系统
- **双路输出策略**：日志同时写入环形缓冲区和控制台，支持异步读取和实时监控
- **无锁并发设计**：采用 MPSC（多生产者单消费者）模型的环形缓冲区，支持多核并发写入而无需锁
- **级别过滤机制**：支持全局级别和控制台级别的独立过滤，灵活控制日志的缓存和显示
- **零动态分配**：所有数据结构在编译期确定大小，无堆内存分配，适合裸机环境
- **彩色控制台输出**：根据日志级别使用不同的 ANSI 颜色，提高可读性
- **早期过滤优化**：在宏展开阶段检查级别，避免格式化被禁用的日志，降低性能开销
- **syslog 系统调用**：提供兼容 Linux 的 `syslog` 系统调用，允许用户空间程序读取和控制内核日志
- **非破坏性读取**：支持 peek 操作，可以读取日志而不从缓冲区删除它们
- **精确字节计数**：实时追踪未读日志的格式化字节数，支持缓冲区状态查询

## 模块结构

```
os/src/log/
├── mod.rs              # 模块入口，导出公共 API 和全局单例 GLOBAL_LOG
├── macros.rs           # 用户宏接口 (pr_info!, pr_err! 等)
├── log_core.rs         # 核心日志系统，封装缓冲区和双过滤器
├── buffer.rs           # 无锁环形缓冲区实现 (MPSC 模型)
├── entry.rs            # 日志条目结构和序列化逻辑
├── level.rs            # 日志级别枚举定义
├── context.rs          # 上下文信息收集 (CPU ID、时间戳等)
├── config.rs           # 配置常量 (缓冲区大小、消息长度限制等)
└── tests/              # 测试模块
    ├── mod.rs          # 测试入口和辅助宏
    ├── basic.rs        # 基本读写和 FIFO 测试
    ├── filter.rs       # 日志级别过滤测试
    ├── overflow.rs     # 缓冲区溢出测试
    ├── format.rs       # 消息格式化测试
    ├── byte_counting.rs # 字节计数测试
    └── nondestructive_read.rs # 非破坏性读取测试
```

### 模块职责

- **mod.rs**：模块的统一入口，导出全局单例 `GLOBAL_LOG` 和所有公共 API，对外屏蔽内部实现细节
- **macros.rs**：提供用户友好的宏接口（`pr_emerg!`、`pr_alert!`、`pr_crit!`、`pr_err!`、`pr_warn!`、`pr_notice!`、`pr_info!`、`pr_debug!`），负责早期级别过滤和格式化参数的传递
- **log_core.rs**：日志系统的核心逻辑，管理环形缓冲区和双过滤器（全局级别和控制台级别），协调日志的写入和读取
- **buffer.rs**：实现无锁环形缓冲区，使用原子操作和票号系统保证多核并发安全，处理缓冲区溢出和数据同步
- **entry.rs**：定义日志条目的内存布局，实现日志序列化和格式化显示，管理固定大小的消息缓冲区
- **level.rs**：定义 8 级日志分类和颜色映射，提供级别比较和序列化功能
- **context.rs**：收集日志的上下文信息，包括 CPU ID、任务 ID、时间戳等，依赖架构特定的接口
- **config.rs**：集中管理配置常量，如环形缓冲区大小、消息最大长度等，便于调整和维护

## 文档导航

### 核心概念

- **[整体架构](architecture.md)**：Log 子系统的分层架构、模块依赖、双路输出策略、同步机制、设计决策和性能考量

### 子模块详解

- **[环形缓冲区与日志条目](buffer_and_entry.md)**：无锁 MPSC 环形缓冲区的实现原理、票号系统、溢出处理、日志条目的内存布局和序列化机制
- **[日志级别与宏接口](level.md)**：8 级日志分类的语义、颜色映射、宏接口说明、双过滤器工作原理
- **[使用指南](usage.md)**：日志系统的基本使用、配置方法、最佳实践、常见陷阱和调试技巧

### API 参考

- **[API 索引](api_reference.md)**：所有公共 API 的完整列表、函数签名、使用示例和源代码位置

## 设计原则

Log 子系统的设计遵循以下核心原则：

### 1. 无锁并发

采用 MPSC（多生产者单消费者）模型的环形缓冲区，通过原子操作和票号系统实现无锁并发写入。多个 CPU 核心可以同时记录日志而无需等待锁，避免了传统锁机制带来的性能瓶颈和优先级反转问题。这种设计特别适合裸机环境，在中断处理程序和关键路径中也能安全使用。

### 2. 早期过滤

在宏展开阶段检查日志级别，避免格式化被禁用级别的日志。通过 `is_level_enabled()` 函数提前判断，确保被过滤的日志不会产生任何格式化开销。这种优化使得即使代码中存在大量 Debug 级别的日志，在生产环境中禁用 Debug 后也不会影响性能。

### 3. 固定大小分配

所有数据结构在编译期确定大小，完全避免堆内存分配。日志条目使用固定 256 字节的消息缓冲区，环形缓冲区的容量在配置文件中静态定义。这种设计保证了内存使用的可预测性，避免了动态分配的不确定性和碎片化问题，特别适合资源受限的嵌入式环境。

### 4. 双路输出策略

日志同时写入环形缓冲区和控制台，通过独立的级别过滤器控制两条路径。环形缓冲区缓存所有达到全局级别的日志供后续分析，控制台立即显示达到控制台级别的日志供实时监控。这种策略既保证了日志的完整性，又提供了灵活的实时反馈，满足不同场景的需求。

## 重要约定

### MPSC 并发模型

环形缓冲区采用**多生产者单消费者**（MPSC）模型：
- **多生产者**：多个 CPU 核心可以并发调用日志宏写入日志，通过原子操作的票号系统协调，无需锁
- **单消费者**：只能有一个读取者顺序读取日志，通常是用户态工具或内核日志线程
- **同步保证**：写入使用 Release 语义发布数据，读取使用 Acquire 语义获取数据，保证内存可见性

### 消息长度限制

每条日志消息最多 **256 字节**（定义在 `config.rs:MAX_MESSAGE_LEN`）：
- 超过限制的消息会被自动截断，不会报错或丢失整条日志
- UTF-8 字符边界会被尊重，避免截断产生无效字符序列
- 建议在日志中使用简洁的描述，避免冗长的字符串

### 缓冲区容量

环形缓冲区大小为 **16 KB**（定义在 `config.rs:BUFFER_SIZE`），约可容纳 50-60 条日志：
- 当缓冲区满时，新日志会覆盖最旧的日志（FIFO 策略）
- 系统会记录被丢弃的日志数量，可通过 `log_dropped_count()` 查询
- 频繁丢弃日志表示消费速度不足，应考虑提高读取频率或增大缓冲区

### 默认级别配置

- **全局级别**：默认为 `Info`，控制哪些日志被缓存
- **控制台级别**：默认为 `Warning`，控制哪些日志立即显示

这意味着 Info 及以上级别的日志会被缓存，但只有 Warning 及以上级别的日志会立即打印到控制台。开发时可以调低控制台级别以查看更多实时信息。

## 快速开始

### 基本使用

Log 子系统在内核启动时自动初始化，无需显式调用初始化函数。使用日志宏即可记录日志：

```rust
use log::{pr_info, pr_err, pr_warn};

// 记录信息性日志
pr_info!("Kernel initialized successfully");

// 记录错误
pr_err!("Failed to mount filesystem: {}", error_code);

// 记录警告
pr_warn!("Memory usage: {} MB", usage);

// 带变量的格式化输出
let pid = 42;
let name = "init";
pr_info!("Starting process {} ({})", pid, name);
```

### 配置级别过滤器

可以动态调整全局级别和控制台级别：

```rust
use log::{set_global_level, set_console_level, LogLevel};

// 设置全局级别为 Debug，缓存所有级别的日志
set_global_level(LogLevel::Debug);

// 设置控制台级别为 Info，显示 Info 及以上级别的日志
set_console_level(LogLevel::Info);

// 生产环境可以提高级别减少日志量
set_global_level(LogLevel::Warning);
set_console_level(LogLevel::Error);
```

### 读取日志

从环形缓冲区读取缓存的日志：

```rust
use log::{read_log, log_len, log_dropped_count, log_unread_bytes};

// 检查有多少条日志和未读字节数
let count = log_len();
let bytes = log_unread_bytes();
println!("Buffered logs: {}, unread bytes: {}", count, bytes);

// 顺序读取所有日志（破坏性读取）
while let Some(entry) = read_log() {
    println!("{}", entry);
}

// 非破坏性读取（不移除日志）
use log::{peek_log, log_reader_index, log_writer_index};
let start = log_reader_index();
let end = log_writer_index();
for index in start..end {
    if let Some(entry) = peek_log(index) {
        println!("{}", entry);
    }
}

// 检查是否有日志被丢弃
let dropped = log_dropped_count();
if dropped > 0 {
    println!("Warning: {} logs were dropped due to buffer overflow", dropped);
}
```

### syslog 系统调用

用户空间程序可以通过 `syslog` 系统调用读取和控制内核日志：

```c
#include <sys/klog.h>

// 读取内核日志（破坏性）
char buf[8192];
int len = syscall(SYS_syslog, SYSLOG_ACTION_READ, buf, sizeof(buf));

// 读取所有日志（非破坏性）
len = syscall(SYS_syslog, SYSLOG_ACTION_READ_ALL, buf, sizeof(buf));

// 查询未读字节数
int unread = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_UNREAD, NULL, 0);

// 查询缓冲区总大小
int size = syscall(SYS_syslog, SYSLOG_ACTION_SIZE_BUFFER, NULL, 0);

// 设置控制台日志级别
int old_level = syscall(SYS_syslog, SYSLOG_ACTION_CONSOLE_LEVEL, NULL, 5);

// 清空日志缓冲区
syscall(SYS_syslog, SYSLOG_ACTION_CLEAR, NULL, 0);
```

## 相关资源

### 源代码位置

- **主模块**：`os/src/log/mod.rs`
- **核心实现**：`os/src/log/log_core.rs`
- **环形缓冲区**：`os/src/log/buffer.rs`
- **完整源码**：`os/src/log/` 目录

### 配置文件

- **配置常量**：`os/src/log/config.rs`
  - `BUFFER_SIZE`：环形缓冲区大小（16 KB）
  - `MAX_MESSAGE_LEN`：单条日志消息最大长度（256 字节）
  - `MAX_ENTRIES`：缓冲区可容纳的最大日志条目数（自动计算）

### 依赖模块

- **arch::timer**：提供时间戳功能（`get_time()`）
- **arch::kernel::cpu**：提供 CPU ID 获取功能（`cpu_id()`）
- **kernel::cpu**：提供当前任务信息（`current_cpu()` → `current_task`）
- **console::Stdout**：控制台输出接口

### 测试

运行 Log 模块的测试：

```bash
cd os && make test
```

测试覆盖了基本读写、级别过滤、缓冲区溢出、消息格式化等核心功能。

### 版本信息

- **Rust 版本**：nightly-2025-01-13
- **目标架构**：riscv64gc-unknown-none-elf
- **支持架构**：RISC-V (当前)，LoongArch (规划中)
