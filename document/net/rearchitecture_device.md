# 网络设备层重构说明

本文档只覆盖 `device::net`。设备层的任务是提供 NIC 收发能力，不承载接口配置、协议栈状态或用户 socket 语义。

## 当前代码

- `os/src/device/net/net_device.rs` 定义 `NetDevice` 和 `VirtioNetDevice`。
- `os/src/device/net/virtio_net.rs` 初始化 VirtIO 网卡，同时创建 `NetworkInterface` 并注册为 `Driver`。
- `os/src/device/net/null_net.rs` 提供 `NullNetDevice`，当前被 loopback 兼容路径间接依赖。
- `os/src/device/net/mod.rs` 维护 `NETWORK_DEVICES`。

## 目标边界

`NetDevice` 只允许表达：

- 二层帧 `send()` / `receive()`
- `device_id()`
- `mtu()`
- `name()`
- `mac_address()`
- 链路状态和设备能力查询

`NetDevice` 禁止表达：

- IP 地址、网关、路由
- socket 或 fd 状态
- `smoltcp` 类型
- loopback 特判

## 必做改造

1. 为 `NetDevice` 增加最小能力描述类型，例如 `NetDeviceCaps` 和 `LinkState`。
2. 把 `VirtioNetDevice` 的职责限制为 VirtIO 队列收发、MAC、MTU、设备状态。
3. 新增网络子系统设备接入口，例如 `net::register_net_device(Arc<dyn NetDevice>)`。
4. `virtio_net::init()` 和 `virtio_net::init_pci()` 只注册设备，不直接创建接口对象。
5. 如果中断注册仍需要 `Driver`，新增独立的 net driver shim，避免 `NetworkInterface` 继续实现 `Driver`。

## 迁移兼容

- 第一阶段可保留 `NETWORK_DEVICES`，但它只能是设备表，不能成为接口表。
- 第一阶段可保留 `NullNetDevice`，但文档和注释必须标明它不是 loopback。
- 若必须保留旧 `virtio_net::init()` 行为，使用薄包装调用新的网络子系统注册入口。

## 验收点

- 替换 `VirtioNetDevice` 实现不需要修改 `os/src/kernel/syscall/network.rs`。
- 设备层文件不 import `smoltcp`。
- 设备层文件不 import `SocketFile`、`SOCKET_SET`、`NET_IFACE`。
- `NetDeviceAdapter` 不在 `device::net` 公开 API 中出现。
