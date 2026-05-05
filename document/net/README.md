# 网络模块概览

网络模块负责把内核网卡设备、接口配置、smoltcp 协议栈、VFS socket 文件和网络 syscall 连接起来。

## 源码入口

- `os/src/device/net/`：网卡设备抽象和 VirtIO/loopback/null 设备。
- `os/src/net/interface.rs`：接口注册表、接口配置和 smoltcp interface 兼容工厂。
- `os/src/net/stack.rs`：`NetworkStack` 状态对象和协议栈运行时实现。
- `os/src/net/socket.rs`：`SocketFile`、fd/socket 映射、UDP per-fd 队列和公开 socket 包装 API。
- `os/src/kernel/syscall/network.rs`：网络 syscall ABI 层。

## 文档导航

- `architecture.md`：整体分层和依赖方向。
- `device_and_interface.md`：设备注册、接口对象和中断兼容桥。
- `stack_runtime.md`：smoltcp runtime、poll、socket set 和 loopback link。
- `socket_syscall.md`：SocketFile、fd 映射、syscall errno 边界。
- `loopback_poll.md`：显式 loopback 和 poll/select 唤醒。
- `testing.md`：验证矩阵和常见排查。
- `network_implementation_guide.md`：维护指南。
