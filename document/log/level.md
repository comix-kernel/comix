# 日志级别与宏接口

## 概述

Log 子系统采用 8 级日志分类系统，模仿 Linux 内核的 `printk` 级别设计。每个级别对应不同的严重程度，从最高优先级的 Emergency（系统不可用）到最低优先级的 Debug（调试信息）。本文档详细介绍日志级别的语义、宏接口的使用、颜色映射以及双过滤器的工作原理。

## 日志级别枚举

`LogLevel` 是一个 8 级枚举，定义在 `os/src/log/level.rs:7-18`：

| 级别值 | 级别名称 | 宏接口 | 语义 |
|-------|---------|--------|------|
| 0 | Emergency | `pr_emerg!()` | 系统不可用，需要立即采取行动 |
| 1 | Alert | `pr_alert!()` | 必须立即采取行动的严重情况 |
| 2 | Critical | `pr_crit!()` | 临界错误，系统功能受到严重影响 |
| 3 | Error | `pr_err!()` | 错误条件，功能无法正常工作 |
| 4 | Warning | `pr_warn!()` | 警告条件，可能导致问题 |
| 5 | Notice | `pr_notice!()` | 正常但重要的信息 |
| 6 | Info | `pr_info!()` | 信息性消息 |
| 7 | Debug | `pr_debug!()` | 调试级别的详细信息 |

**级别排序**：数值越小，优先级越高。Emergency（0）是最高级别，Debug（7）是最低级别。

**枚举表示**：使用 `#[repr(u8)]` 保证枚举值与底层整数对应，便于原子存储和比较。

## 各级别详细说明

### Emergency（紧急）

**数值**：0
**宏**：`pr_emerg!()`
**语义**：系统完全不可用，即将崩溃或已经崩溃

**使用场景**：
- 内核 panic 前的最后一条消息
- 严重的硬件故障（如内存控制器失败）
- 无法恢复的系统状态（如栈溢出）

**示例情况**：
- "Kernel panic: unable to continue"
- "Hardware failure: memory controller not responding"
- "Critical resource exhausted: cannot allocate kernel stack"

### Alert（警报）

**数值**：1
**宏**：`pr_alert!()`
**语义**：必须立即采取行动的严重情况

**使用场景**：
- 文件系统损坏
- 关键设备故障
- 资源即将耗尽（但还有机会恢复）

**示例情况**：
- "Filesystem corruption detected, immediate repair required"
- "Critical device failure: disk controller error"
- "System temperature critical, shutting down soon"

### Critical（严重）

**数值**：2
**宏**：`pr_crit!()`
**语义**：临界错误，系统功能受到严重影响，但系统可能还能运行

**使用场景**：
- 主要功能失败（但系统未完全崩溃）
- 安全相关的严重问题
- 重要资源分配失败

**示例情况**：
- "Unable to initialize network subsystem"
- "Security violation: unauthorized memory access attempt"
- "Failed to allocate memory for critical kernel structure"

### Error（错误）

**数值**：3
**宏**：`pr_err!()`
**语义**：错误条件，某个功能无法正常工作

**使用场景**：
- 系统调用失败
- 设备驱动错误
- 无法完成用户请求

**示例情况**：
- "Failed to open file: permission denied"
- "Device driver error: invalid ioctl command"
- "Unable to create process: resource limit exceeded"

### Warning（警告）

**数值**：4
**宏**：`pr_warn!()`
**语义**：警告条件，当前没有错误但可能导致未来问题

**使用场景**：
- 资源使用率高
- 检测到异常但可恢复的情况
- 配置问题（非致命）

**示例情况**：
- "Memory usage high: 95% of physical memory in use"
- "Retrying operation after transient failure"
- "Deprecated API called, please update code"

### Notice（通知）

**数值**：5
**宏**：`pr_notice!()`
**语义**：正常但重要的信息，值得注意但不是错误

**使用场景**：
- 系统状态变化
- 重要操作完成
- 配置变更

**示例情况**：
- "Network interface eth0 link up"
- "User root logged in"
- "System entering suspend mode"

### Info（信息）

**数值**：6
**宏**：`pr_info!()`
**语义**：信息性消息，记录系统的正常操作

**使用场景**：
- 子系统初始化
- 常规操作日志
- 统计信息

**示例情况**：
- "Filesystem mounted: /dev/sda1 on /"
- "Process 1234 started: /bin/bash"
- "Cache statistics: 1000 hits, 50 misses"

### Debug（调试）

**数值**：7
**宏**：`pr_debug!()`
**语义**：调试级别的详细信息，仅供开发和问题诊断使用

**使用场景**：
- 函数进入/退出跟踪
- 中间变量值
- 详细的状态转换

**示例情况**：
- "Entering function: allocate_frame()"
- "Page table entry: PTE[123] = 0x80001001"
- "State transition: RUNNING -> BLOCKED"

## 与 Linux 内核的对比

Comix Log 子系统的级别设计直接借鉴 Linux 内核的 `printk` 级别：

| Comix 级别 | Linux 级别 | Linux 宏 | 数值 | 说明 |
|-----------|-----------|----------|------|------|
| Emergency | KERN_EMERG | `pr_emerg()` | 0 | 完全一致 |
| Alert | KERN_ALERT | `pr_alert()` | 1 | 完全一致 |
| Critical | KERN_CRIT | `pr_crit()` | 2 | 完全一致 |
| Error | KERN_ERR | `pr_err()` | 3 | 完全一致 |
| Warning | KERN_WARNING | `pr_warn()` | 4 | 完全一致 |
| Notice | KERN_NOTICE | `pr_notice()` | 5 | 完全一致 |
| Info | KERN_INFO | `pr_info()` | 6 | 完全一致 |
| Debug | KERN_DEBUG | `pr_debug()` | 7 | 完全一致 |

**一致性优势**：
- 熟悉 Linux 内核开发的人可以无缝迁移
- 宏名称和语义保持一致，降低学习成本
- 遵循成熟的最佳实践，避免重复设计

**差异**：
- Comix 使用 Rust 的 `format_args!` 宏处理格式化，而 Linux 使用 C 的可变参数
- Comix 实现了早期过滤优化，禁用级别的日志完全零开销
- Comix 的双过滤器设计更灵活（独立的 global_level 和 console_level）

## 颜色映射

控制台输出根据日志级别使用不同的 ANSI 颜色，提高可读性。颜色映射定义在 `os/src/log/level.rs:44-58`。

| 级别 | 颜色 | ANSI 转义码 | 效果 |
|------|------|------------|------|
| Emergency | 亮红色（Bright Red） | `\x1b[91m` | 高优先级，极其显眼 |
| Alert | 亮红色（Bright Red） | `\x1b[91m` | 高优先级，极其显眼 |
| Critical | 亮红色（Bright Red） | `\x1b[91m` | 高优先级，极其显眼 |
| Error | 红色（Red） | `\x1b[31m` | 错误信息，醒目 |
| Warning | 黄色（Yellow） | `\x1b[33m` | 警告信息，引起注意 |
| Notice | 青色（Cyan） | `\x1b[36m` | 重要信息，区别于普通 |
| Info | 绿色（Green） | `\x1b[32m` | 正常信息，表示成功 |
| Debug | 默认颜色 | 无 | 调试信息，不突出显示 |

**颜色分组**：
- **亮红色（Emergency/Alert/Critical）**：最高三级使用相同的亮红色，表示极其严重的情况
- **红色（Error）**：普通错误，醒目但不如亮红
- **黄色（Warning）**：警告，引起注意但不表示错误
- **青色（Notice）**：重要的正常信息，有别于普通信息
- **绿色（Info）**：正常操作，绿色通常表示"正常"或"成功"
- **默认色（Debug）**：调试信息不特别突出，避免干扰

**控制台兼容性**：
- ANSI 颜色码在大多数现代终端和串口工具中支持（如 minicom、screen、PuTTY）
- 不支持颜色的终端会忽略转义码，显示为普通文本
- 可以通过环境变量或配置禁用颜色（未来可扩展）

**颜色效果示例**：

```
[000012345678] [EMERG] [CPU0/Task1] Kernel panic!          ← 亮红色
[000012345679] [ALERT] [CPU0/Task1] Disk failure!          ← 亮红色
[000012345680] [CRIT ] [CPU1/Task2] Out of memory!         ← 亮红色
[000012345681] [ERROR] [CPU2/Task3] File not found         ← 红色
[000012345682] [WARN ] [CPU0/Task1] High temperature       ← 黄色
[000012345683] [NOTIC] [CPU1/Task2] Network connected      ← 青色
[000012345684] [INFO ] [CPU2/Task3] Process started        ← 绿色
[000012345685] [DEBUG] [CPU3/Task4] Function entry         ← 默认色
```

## 宏接口说明

Log 子系统提供 8 个宏，对应 8 个日志级别。所有宏定义在 `os/src/log/macros.rs`。

### 宏列表

| 宏名称 | 级别 | 定义位置 |
|--------|------|---------|
| `pr_emerg!()` | Emergency | `os/src/log/macros.rs:60-67` |
| `pr_alert!()` | Alert | `os/src/log/macros.rs:79-86` |
| `pr_crit!()` | Critical | `os/src/log/macros.rs:98-105` |
| `pr_err!()` | Error | `os/src/log/macros.rs:118-127` |
| `pr_warn!()` | Warning | `os/src/log/macros.rs:139-148` |
| `pr_notice!()` | Notice | `os/src/log/macros.rs:158-167` |
| `pr_info!()` | Info | `os/src/log/macros.rs:178-187` |
| `pr_debug!()` | Debug | `os/src/log/macros.rs:199-208` |

### 宏的功能

所有宏接口具有相同的行为模式：

1. **早期级别检查**：展开时调用 `is_level_enabled()` 判断级别是否启用
2. **条件格式化**：只有级别启用时才格式化参数
3. **调用核心函数**：调用 `log_impl()` 传递级别和格式化的参数
4. **零开销抽象**：被禁用的日志完全不产生运行时开销

### 宏的基本形式

所有宏支持类似 `format!` 的语法：

- **无参数**：`pr_info!("message")`
- **带参数**：`pr_info!("value: {}", x)`
- **多参数**：`pr_info!("x={}, y={}", x, y)`
- **格式化选项**：`pr_info!("hex: {:#x}", value)`

### 各宏的使用建议

#### pr_emerg!() - Emergency

**何时使用**：
- 系统即将崩溃，这是最后的消息
- 严重的硬件故障使系统无法继续
- panic 前记录原因

**使用频率**：极少（理想情况下从不使用）

**注意事项**：
- Emergency 日志应该简洁明了，说明问题的本质
- 这可能是系统记录的最后一条日志

#### pr_alert!() - Alert

**何时使用**：
- 检测到需要立即人工干预的情况
- 系统还能运行但功能严重受损
- 关键资源即将耗尽

**使用频率**：很少

**注意事项**：
- Alert 应触发管理员通知（如果有监控系统）
- 记录足够的上下文帮助快速定位问题

#### pr_crit!() - Critical

**何时使用**：
- 主要功能失败但系统未完全崩溃
- 安全相关的严重问题
- 重要资源初始化失败

**使用频率**：少

**注意事项**：
- Critical 表示系统处于不稳定状态
- 应考虑降级服务或限制功能

#### pr_err!() - Error

**何时使用**：
- 系统调用失败
- 用户请求无法完成
- 设备或驱动错误

**使用频率**：中等

**注意事项**：
- Error 应包含错误码或原因
- 帮助用户理解为什么操作失败
- 常见的错误级别，但不应滥用

#### pr_warn!() - Warning

**何时使用**：
- 检测到可能导致问题的情况
- 使用了不推荐的功能
- 资源使用率高

**使用频率**：中等

**注意事项**：
- Warning 不应泛滥，避免"狼来了"效应
- 应指出潜在的问题和解决方案

#### pr_notice!() - Notice

**何时使用**：
- 系统状态变化（网络连接、设备插拔）
- 重要操作完成
- 安全相关事件（登录、权限变更）

**使用频率**：中等

**注意事项**：
- Notice 和 Info 的界限有时模糊
- 如果事件值得管理员关注，使用 Notice

#### pr_info!() - Info

**何时使用**：
- 子系统初始化
- 常规操作日志
- 统计信息和进度报告

**使用频率**：高

**注意事项**：
- Info 是最常用的级别
- 生产环境通常默认启用 Info 及以上级别
- 应保持日志简洁，避免过于冗长

#### pr_debug!() - Debug

**何时使用**：
- 开发和调试时跟踪代码执行
- 记录中间变量和状态
- 详细的函数调用跟踪

**使用频率**：非常高（开发时），极低（生产时）

**注意事项**：
- Debug 日志在生产环境通常被禁用
- 可以自由使用，不担心性能（感谢早期过滤）
- 应使用描述性的日志，帮助理解代码流程

## 级别过滤配置

Log 子系统使用双过滤器设计：`global_level` 和 `console_level`。

### 双过滤器架构

```
日志写入流程中的双重过滤：

用户调用 pr_info!("message")
         │
         │
         ▼
┌─────────────────────────┐
│ 早期过滤（宏展开时）     │
│ is_level_enabled()?     │ ← 检查 global_level
│ (避免格式化被禁用的日志) │
└──────────┬──────────────┘
           │ (通过)
           ▼
┌─────────────────────────┐
│ 创建 LogEntry           │
│ 格式化消息               │
└──────────┬──────────────┘
           │
           ├───────────────────┬───────────────────┐
           │                   │                   │
           ▼                   ▼                   ▼
    ┌──────────────┐    ┌──────────────┐   ┌──────────────┐
    │ 过滤器 1     │    │ 过滤器 2     │   │ (其他处理)   │
    │ global_level │    │console_level │   │              │
    └──────┬───────┘    └──────┬───────┘   └──────────────┘
           │                   │
           │ (Info >= global)  │ (Info >= console)
           │                   │
           ▼                   ▼
    ┌──────────────┐    ┌──────────────┐
    │ 写入缓冲区    │    │ 打印到控制台  │
    └──────────────┘    └──────────────┘
```

### global_level（全局级别）

**作用**：控制哪些日志被缓存到环形缓冲区

**默认值**：`Info`（级别 6）

**影响**：
- 低于 global_level 的日志会被完全忽略（宏展开时就跳过）
- 达到或超过 global_level 的日志会被缓存

**配置函数**：
- `set_global_level(level: LogLevel)` - 设置全局级别（位于 `os/src/log/mod.rs:86`）
- `get_global_level() -> LogLevel` - 获取当前全局级别（位于 `os/src/log/mod.rs:91`）

**使用场景**：
- **开发阶段**：设置为 `Debug`，捕获所有日志
- **测试阶段**：设置为 `Info`，记录正常操作
- **生产环境**：设置为 `Warning` 或 `Error`，只记录问题

### console_level（控制台级别）

**作用**：控制哪些日志立即打印到控制台

**默认值**：`Warning`（级别 4）

**影响**：
- 低于 console_level 的日志不会打印到控制台（但仍可能被缓存）
- 达到或超过 console_level 的日志会立即打印

**配置函数**：
- `set_console_level(level: LogLevel)` - 设置控制台级别（位于 `os/src/log/mod.rs:96`）
- `get_console_level() -> LogLevel` - 获取当前控制台级别（位于 `os/src/log/mod.rs:101`）

**使用场景**：
- **开发阶段**：设置为 `Info` 或 `Debug`，实时查看所有日志
- **演示阶段**：设置为 `Notice`，显示重要操作
- **生产环境**：设置为 `Error`，只显示错误信息

### 级别过滤矩阵

不同配置下各级别日志的处理方式：

| 日志级别 | global=Debug, console=Debug | global=Info, console=Warning | global=Error, console=Error |
|---------|----------------------------|------------------------------|------------------------------|
| Emergency (0) | 缓存 + 显示 | 缓存 + 显示 | 缓存 + 显示 |
| Alert (1) | 缓存 + 显示 | 缓存 + 显示 | 缓存 + 显示 |
| Critical (2) | 缓存 + 显示 | 缓存 + 显示 | 缓存 + 显示 |
| Error (3) | 缓存 + 显示 | 缓存 + 显示 | 缓存 + 显示 |
| Warning (4) | 缓存 + 显示 | 缓存 + 显示 | 不缓存，不显示 |
| Notice (5) | 缓存 + 显示 | 缓存，不显示 | 不缓存，不显示 |
| Info (6) | 缓存 + 显示 | 缓存，不显示 | 不缓存，不显示 |
| Debug (7) | 缓存 + 显示 | 不缓存，不显示 | 不缓存，不显示 |

**关键观察**：
- global_level 是"第一道防线"，决定日志是否被记录
- console_level 是"第二道防线"，决定日志是否显示
- console_level 必须 >= global_level 才有意义（否则缓存的日志不会被显示）

### 推荐配置

#### 开发调试配置

```rust
set_global_level(LogLevel::Debug);    // 缓存所有日志
set_console_level(LogLevel::Info);    // 显示 Info 及以上级别
```

**效果**：Debug 日志被缓存但不显示，减少控制台刷屏，需要时可以读取缓冲区查看。

#### 正常运行配置

```rust
set_global_level(LogLevel::Info);     // 缓存常规日志
set_console_level(LogLevel::Warning); // 只显示警告和错误
```

**效果**：平衡日志完整性和控制台清洁度，这是默认配置。

#### 生产环境配置

```rust
set_global_level(LogLevel::Warning);  // 只缓存问题
set_console_level(LogLevel::Error);   // 只显示错误
```

**效果**：最小化日志开销，只记录和显示真正的问题。

#### 调试特定问题配置

```rust
set_global_level(LogLevel::Debug);    // 缓存所有日志
set_console_level(LogLevel::Debug);   // 显示所有日志
```

**效果**：最大程度的可见性，用于诊断难以重现的问题。注意：可能产生大量输出。

## 双过滤器工作原理

双过滤器在不同阶段发挥作用，优化性能和灵活性：

```
时间线上的过滤阶段：

┌─────────────────────────────────────────────────────────────────┐
│  阶段 1：编译/宏展开时 - 早期过滤                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  pr_info!("value: {}", expensive_calculation())                  │
│           ↓                                                       │
│  if is_level_enabled(LogLevel::Info) {    ← 检查 global_level   │
│      log_impl(LogLevel::Info, format_args!("value: {}", ...))    │
│  }                                                                │
│                                                                   │
│  如果 Info < global_level：                                      │
│  · expensive_calculation() 不会被调用                            │
│  · format_args! 不会被求值                                       │
│  · 整个 if 块被跳过，零开销                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ (通过 global_level 过滤)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  阶段 2：运行时 - LogCore::log() 内部                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  创建 LogEntry，收集上下文信息（CPU ID、时间戳等）               │
│  格式化消息到 entry.message                                      │
│                                                                   │
│  分支 1：写入缓冲区（无需再检查 global_level，已通过）           │
│  buffer.write(entry)  → 总是执行                                 │
│                                                                   │
│  分支 2：控制台输出（需要检查 console_level）                    │
│  if entry.level <= self.console_level.load(Acquire) {            │
│      println!("{}", entry);  // 带颜色的格式化输出               │
│  }                                                                │
└─────────────────────────────────────────────────────────────────┘
```

### 为什么需要两个过滤器？

**设计理由**：

1. **不同的关注点**：
   - global_level：哪些日志值得保留？（完整性）
   - console_level：哪些日志需要立即看到？（实时性）

2. **性能考量**：
   - 控制台输出慢（串口通信），减少输出量避免阻塞
   - 缓冲区写入快（无锁内存操作），可以记录更多日志

3. **灵活性**：
   - 开发时：console_level=Info，实时查看常规操作
   - 生产时：console_level=Error，只显示严重问题
   - global_level 保持不变，保证日志完整性

4. **避免刷屏**：
   - Debug 日志缓存但不显示，需要时读取缓冲区
   - 控制台保持清洁，不被大量 Debug 信息淹没

### 单过滤器 vs 双过滤器

| 特性 | 单过滤器设计 | 双过滤器设计（当前） |
|------|-------------|---------------------|
| 配置复杂度 | 简单，只有一个级别 | 略复杂，两个级别 |
| 灵活性 | 低，缓存和显示必须同步 | 高，独立控制 |
| 控制台清洁度 | 差，要么全显示要么全不显示 | 好，可以只显示重要日志 |
| 性能 | 中等 | 优，减少控制台输出 |
| Linux 兼容性 | 低 | 高，类似 console_loglevel |

**结论**：双过滤器的额外复杂度是值得的，提供了更好的灵活性和性能。

## 最佳实践

### 选择合适的级别

**决策树**：

```
是否导致系统崩溃或即将崩溃？
├─ 是 → Emergency
└─ 否 → 是否需要立即人工干预？
    ├─ 是 → Alert
    └─ 否 → 是否严重影响系统功能？
        ├─ 是 → Critical
        └─ 否 → 是否导致操作失败？
            ├─ 是 → Error
            └─ 否 → 是否可能导致未来问题？
                ├─ 是 → Warning
                └─ 否 → 是否值得管理员关注？
                    ├─ 是 → Notice
                    └─ 否 → 是否常规操作信息？
                        ├─ 是 → Info
                        └─ 否 → Debug
```

### 避免常见错误

#### 错误 1：过度使用高级别

**不好的做法**：
- 将所有错误都标记为 Critical
- 将所有警告都标记为 Error

**问题**：
- 级别失去意义，无法区分严重程度
- 产生"狼来了"效应，真正严重的问题被淹没

**正确做法**：
- 严格按照语义使用级别
- Critical 只用于真正严重影响系统的情况

#### 错误 2：日志过于冗长

**不好的做法**：
- 在日志中包含大量上下文信息
- 日志消息超过 256 字节被截断

**问题**：
- 浪费缓冲区空间
- 重要信息可能被截断
- 控制台输出缓慢

**正确做法**：
- 日志简洁明了，通常一行足够
- 复杂信息分多条日志记录
- 使用结构化的格式（如 key=value）

#### 错误 3：在热路径使用 Debug 日志

**不好的做法**：
- 在循环中记录 Debug 日志
- 在中断处理程序中大量使用日志

**问题**：
- 即使 Debug 被禁用，早期过滤也有微小开销
- 大量日志调用影响性能

**正确做法**：
- 热路径使用条件编译（`#[cfg(debug_assertions)]`）
- 或者使用专门的性能跟踪工具而不是日志

### 日志的可读性

**好的日志示例**：

- `pr_info!("Filesystem mounted: {} on {}", device, mountpoint)`
- `pr_err!("Failed to allocate memory: size={} bytes, error={}", size, err)`
- `pr_warn!("Memory usage high: {}% ({}MB / {}MB)", percent, used, total)`

**特点**：
- 包含关键信息（设备名、大小、错误码）
- 简洁明了，一眼能看懂
- 使用结构化格式，便于解析

**不好的日志示例**：

- `pr_info!("Operation completed")`  ← 太模糊
- `pr_err!("Error occurred")`  ← 没有上下文
- `pr_debug!("Value: {:?}", huge_structure)`  ← 可能非常长

## 未来扩展

### 按模块过滤

当前只有全局级别，未来可支持按模块设置不同级别：

```
log::mm::set_level(LogLevel::Debug);    // MM 子系统使用 Debug
log::fs::set_level(LogLevel::Info);     // FS 子系统使用 Info
```

### 动态级别调整

支持运行时通过 debugfs 或系统调用动态调整级别，无需重启系统。

### 结构化日志

支持结构化字段（如 JSON），便于机器解析：

```
pr_info_struct!(
    "event" => "process_start",
    "pid" => pid,
    "name" => name
);
```

### 日志分类标签

支持给日志添加标签（如模块名、子系统），便于过滤和分析：

```
pr_info!(tag="mm", "Allocated {} frames", count);
```

这些扩展在不破坏现有 API 的前提下都是可行的。
