# 网络重构施工文档

本目录保存 Codex 使用的网络模块重构、迁移和执行类文档。它们用于指导补丁顺序、检查跨层边界和记录兼容路径，不作为当前实现的主要解释文档。

当前网络模块的解释说明请阅读：

- `document/net/README.md`
- `document/net/architecture.md`
- `document/net/device_and_interface.md`
- `document/net/stack_runtime.md`
- `document/net/socket_syscall.md`
- `document/net/loopback_poll.md`
- `document/net/testing.md`

## 文件说明

- `network_implementation_guide.md`：历史实现指南和问题记录。
- `network_rearchitecture.md`：网络子系统重新分层蓝图。
- `rearchitecture_execution_plan.md`：重构执行阶段、补丁边界和验收矩阵。
- `rearchitecture_device.md`：设备层迁移说明。
- `rearchitecture_interface.md`：接口控制面迁移说明。
- `rearchitecture_stack_runtime.md`：协议栈运行时迁移说明。
- `rearchitecture_socket_syscall.md`：SocketFile 与 syscall 迁移说明。
- `rearchitecture_loopback_poll.md`：loopback、poll、RX/TX 迁移说明。

