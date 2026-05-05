# 网络架构

## 目标模型

```text
device/net NetDevice
        |
        v
net::register_net_device()
        |
        v
NetworkInterface registry
        |
        v
NetworkStack runtime
        |
        v
SocketFile / syscall ABI
```

依赖方向只能自上而下或通过明确门面调用。设备层不理解 socket，syscall 层不理解 smoltcp 的内部 socket set。

## 当前边界

- `NetworkStack` 是协议栈状态宿主，持有 smoltcp socket set、active interface runtime、loopback link，并提供 TCP/UDP/socket 文件级 API。
- `NetworkInterface` 是控制面接口对象，保存名称、MAC、IP 地址、网关和兼容中断状态。
- `SocketFile` 是 VFS 文件对象，保存 fd 相关逻辑状态，如 local/remote endpoint、flags、shutdown 状态、UDP per-fd 接收队列。
- `kernel::syscall::network` 只负责用户参数、fd table、sockaddr 编解码和 errno。

## 兼容点

- 当前仍是单 active smoltcp runtime，多接口 runtime 需要后续引入真正的 `StackSocketId` 和接口路由。
- `SocketHandle` 仍包装 smoltcp handle，作为旧路径兼容类型。
- `NetworkInterface::create_smoltcp_interface()` 暂时保留为 runtime 初始化工厂。
