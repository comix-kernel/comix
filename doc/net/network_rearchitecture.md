# 网络重构施工记录

本文档记录 `new-main` 上网络模块重构的执行顺序。当前事实应写入 `document/net/`，本目录只保留施工上下文。

## 执行顺序

1. 更新 `AGENTS.md`，固定网络重构边界。
2. 修正 `document/net/`，把旧“待实现指南”改成当前架构事实。
3. 引入 `NetworkStack`，把 smoltcp runtime 和 socket 操作收口到 `net::stack`。
4. 拆分设备注册和接口控制面，VirtIO 只注册 `NetDevice`。
5. 显式建模 `lo` 和 loopback link。
6. 清理 syscall 层，移除裸 `SOCKET_SET`、`NET_IFACE` 和 smoltcp socket 类型依赖。
7. 执行 `cargo fmt && cargo check`。

## 验收边界

- `os/src/kernel/syscall/network.rs` 不直接访问协议栈内部状态。
- `NetworkInterface` 不实现 `Driver`，中断兼容通过 `NetDriverHandle`。
- `NullNetDevice` 不承担 loopback 语义。
- `document/net/` 的说明与代码一致。
