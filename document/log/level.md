# 日志级别

日志级别兼容 Linux printk 的 0 到 7 优先级模型。数值越小表示越严重, 阈值比较使用 `level <= threshold`。

## 当前状态

| 值 | 级别 | 宏 | 用途 |
| --- | --- | --- | --- |
| 0 | Emergency | `pr_emerg!` | 系统不可用 |
| 1 | Alert | `pr_alert!` | 必须立即处理 |
| 2 | Critical | `pr_crit!` | 关键错误 |
| 3 | Error | `pr_err!` | 普通错误 |
| 4 | Warning | `pr_warn!` | 可恢复风险 |
| 5 | Notice | `pr_notice!` | 正常但重要 |
| 6 | Info | `pr_info!` | 常规信息 |
| 7 | Debug | `pr_debug!` | 调试信息 |

## 目标

- 给内核日志提供统一严重度语言。
- 让宏层能在格式化前做早期过滤。
- 让缓冲和控制台输出可以使用不同阈值。

## 非目标

- 不规定每个子系统必须用某个具体级别。
- 不把日志级别当成错误处理或审计等级。
- 不在文档维护宏展开实现清单。

## 过滤设计

- global level 控制是否进入环形缓冲。
- console level 控制是否即时输出到控制台。
- `pr_*` 宏先调用 `is_level_enabled()`, 被过滤时不求值 `format_args!` 后续路径。
- `print!`/`println!` 不按 `pr_*` 级别过滤, 它们按原始输出语义打印, 同时写入 Info 缓冲记录。

## 颜色和格式

当前 `LogLevel` 为控制台和 syslog 格式提供级别标签和 ANSI 颜色码。格式大致包含:

- 级别标签。
- timestamp。
- CPU id。
- task id。
- message。

颜色码会出现在 `format_log_entry()` 的输出中。消费 syslog 的用户态工具如果不希望显示颜色, 需要自行剥离 ANSI escape。

## 使用约束

- 热路径调试信息使用 `pr_debug!`, 依赖默认过滤降低开销。
- 可恢复但需要注意的情况使用 `pr_warn!`, 不要滥用 `pr_err!`。
- panic 或 trap 中无法信任普通路径时使用 emergency 输出, 而不是提高日志级别。

## 已知限制

- 默认 console level 当前为 Info, 和部分旧文档描述的 Warning 不一致。
- `LogLevel::from_u8()` 对未知值回落到默认日志级别。

## 源码索引

- `os/src/log/level.rs`: 级别枚举, 标签, 颜色。
- `os/src/log/config.rs`: 默认 global/console level。
- `os/src/log/macros.rs`: 宏入口和早期过滤。
- `os/src/log/log_core.rs`: 阈值检查和控制台输出。
