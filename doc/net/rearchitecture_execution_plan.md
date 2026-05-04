# 网络重构执行计划

本计划用于将旧 `main` 中已经验证的网络重构重新落实到 `new-main`。

## 阶段

1. `AGENTS.md`：补齐各目录边界规则，尤其是 `net`、`device/net`、`kernel/syscall`。
2. 文档：直接修正 `document/net/` 中的旧文档，拆分为架构、设备接口、栈运行时、socket syscall、loopback poll、测试。
3. 协议栈：新增 `os/src/net/stack.rs`，让 `NetworkStack` 持有 runtime 状态并提供门面 API。
4. Socket/syscall：`SocketFile` 和网络 syscall 经 `NetworkStack` 操作协议栈。
5. 设备接口：新增 `register_net_device()`，VirtIO 只交出 `NetDevice`；接口注册由网络子系统负责。
6. Loopback：新增 `LoopbackNetDevice`，默认配置确保 `lo` 存在。
7. 验证：格式化、编译检查、grep 边界。

## 保留兼容

- `SocketHandle` 暂时仍包装 smoltcp handle。
- 当前仍是单 active smoltcp interface runtime。
- `NetworkInterface::create_smoltcp_interface()` 暂时保留为初始化工厂。
