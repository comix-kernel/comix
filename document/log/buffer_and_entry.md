# 缓冲区和日志条目

日志缓冲区是固定大小的 MPSC 环形缓冲。它优先保证写入端不阻塞, 因此满时覆盖最旧未读日志。

## 当前状态

- 全局缓冲大小由 `GLOBAL_LOG_BUFFER_SIZE` 决定, 当前为 16 KiB。
- 单条消息最大长度由 `MAX_LOG_MESSAGE_LENGTH` 决定, 当前为 256 字节。
- `LogEntry` 固定布局, 第一个字段是原子 `seq` 发布标记。
- 写入端使用单调递增 sequence 分配槽位。
- 读取端是单消费者模型。
- 非破坏性 `peek` 用于 syslog read-all。

## 目标

- 多 CPU 写日志时避免互斥锁竞争。
- 不依赖堆分配。
- 溢出时保留最新日志, 同时记录 dropped count。
- 提供未读条目数和格式化字节数给 syslog 查询。

## 非目标

- 不支持多个独立消费者各自维护游标。
- 不保证日志永不丢失。
- 不保存超过固定消息长度的完整文本。

## 写入流程

1. `write_seq.fetch_add()` 获取唯一序号。
2. 用序号对容量取模定位槽位。
3. 若写入会覆盖未读数据, 推进 read sequence 并增加 dropped count。
4. 拷贝条目数据到槽位, 暂不写 seq。
5. 用 Release store 发布 seq。
6. 增加未读格式化字节数。

## 读取流程

1. 读取当前 read sequence。
2. 定位槽位并检查 seq 是否匹配。
3. 匹配则 clone 条目。
4. 减少未读字节数。
5. 推进 read sequence。

## LogEntry 设计

条目保存:

- level。
- CPU id。
- task id。
- timestamp。
- message length。
- 固定大小 message buffer。

消息写入使用内部 `MessageWriter`, 超长消息截断。`message()` 只暴露有效长度范围。

## 并发和生命周期约束

- seq 是生产者和消费者之间的可见性边界。
- 写入发布 seq 前, 读端不能把槽位视为有效。
- 溢出推进 read sequence 可能让消费者看不到旧日志, 这是预期行为。
- `unread_bytes` 是为 syslog size 查询服务的运行时计数, 修改格式化逻辑时必须同步更新计算函数。

## 已知限制

- `peek` 与并发覆盖同时发生时可能返回 None。
- MPSC 设计只假设一个破坏性读取者。
- ANSI 格式化长度纳入字节计数。

## 源码索引

- `os/src/log/buffer.rs`: `GlobalLogBuffer`, sequence, overflow, read/peek。
- `os/src/log/entry.rs`: `LogEntry`, message writer, publish/readiness。
- `os/src/log/config.rs`: 缓冲和消息长度常量。
- `os/src/log/log_core.rs`: 格式化输出和字节计数一致性要求。
