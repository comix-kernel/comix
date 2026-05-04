# 网络接口层重构说明

本文档覆盖 `net::if`，也就是内核控制面中的网络接口对象。接口层描述 `lo`、`eth0` 这类接口，而不是协议栈运行时。

## 当前代码

- `os/src/net/interface.rs` 中的 `NetworkInterface` 同时保存接口配置、持有设备、创建 `SmoltcpInterface`、实现 `Driver`。
- `NETWORK_INTERFACE_MANAGER` 当前维护 `Vec<Arc<NetworkInterface>>`。
- `getifaddrs` 和网络配置路径依赖接口枚举与 IP 配置。

## 目标对象

建议拆出：

- `NetIf`：接口身份和只读查询入口。
- `NetIfConfig`：MAC、IP 地址列表、gateway、MTU、flags。
- `NetIfState`：up/running/link/loopback 等运行状态。
- `InterfaceRegistry`：接口注册、枚举、按名查找。

接口对象可以持有 `Arc<dyn NetDevice>` 或 loopback 后端引用，但不能持有 `smoltcp::Interface` 或 `SocketSet`。

## 必做改造

1. 把 `NetworkInterface::create_smoltcp_interface()` 从接口对象中移除或改成仅供兼容层内部调用。
2. 把 `NetworkInterface` 实现 `Driver` 的逻辑迁出到设备/中断适配对象。
3. 将 `interrupt_enabled`、`last_interrupt_time` 这类中断推进状态从接口配置对象中剥离。
4. `NETWORK_INTERFACE_MANAGER` 改名或收缩为 `InterfaceRegistry`，只保留注册、枚举、查找。
5. `NetworkConfigManager::set_interface_config()` 只修改接口配置，并通过 `NetworkStack` 通知协议栈刷新地址和路由。

## 接口 flags

至少应稳定表达：

- `UP`
- `RUNNING`
- `LOOPBACK`
- `BROADCAST`
- `MULTICAST`

`lo` 必须显式带 `LOOPBACK`；真实网卡不能因无 loopback 技巧而伪装成 `lo`。

## 迁移兼容

- 类型名 `NetworkInterface` 可暂时保留，但语义要逐步收缩。
- 旧的 `NETWORK_INTERFACE_MANAGER` 可作为 `InterfaceRegistry` 的别名保留一个阶段。
- `getifaddrs` 可先继续读取旧接口对象，但新增字段必须来自接口层而不是协议栈运行时。

## 验收点

- 接口层不 import `smoltcp::iface::Interface`。
- 接口层不 import `SocketSet`。
- 接口枚举能区分 `lo` 和 `ethN`。
- IP/gateway 修改有单一入口，并能触发协议栈配置刷新。

## 当前执行状态

- `NetworkInterface` 已从 `Driver` trait 实现中拆出；兼容中断注册由 `NetDriverHandle` 负责。
- `virtio_net` 不再直接创建或注册接口对象，而是调用网络子系统的 `register_net_device(device)`。
- 默认网络初始化会确保 `lo` 接口存在；真实 `ethN` 与 `lo` 的枚举身份已分开。

保留的兼容点：

- `NetworkInterface::create_smoltcp_interface()` 仍作为单 active runtime 的兼容工厂存在，后续可继续下沉到 `NetworkStack` 初始化路径。
- `interrupt_enabled` 和 `last_interrupt_time` 暂留在 `NetworkInterface`，由 `NetDriverHandle` 通过接口对象访问。
