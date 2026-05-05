# 设备与接口

## 设备层

`os/src/device/net/` 只负责网卡数据面：

- `net_device.rs` 定义 `NetDevice`、`NetDeviceError` 和 VirtIO net 设备实现。
- `virtio_net.rs` 初始化 VirtIO net 设备，并调用网络子系统注册入口。
- `loopback.rs` 提供显式 loopback 设备。
- `null_net.rs` 仅作为空设备/占位实现，不表示 loopback。

设备层不得直接依赖 IP、网关、socket、syscall 或 smoltcp socket set。

## 注册路径

真实网卡初始化后调用：

```rust
crate::net::register_net_device(device)
```

该入口负责：

- 注册底层 `NetDevice`。
- 创建 `NetworkInterface`。
- 把接口加入 `NETWORK_INTERFACE_MANAGER`。
- 通过 `NetDriverHandle` 注册中断兼容桥。

## 接口层

`NetworkInterface` 保存控制面状态：

- interface name，如 `eth0`、`lo`。
- MAC address。
- IP CIDR 列表。
- IPv4 gateway。
- 兼容中断开关和最后中断时间。

`NetworkInterface` 不应直接实现 `Driver`。中断体系仍需要 `Driver` 时，使用 `NetDriverHandle`。

## Loopback

默认网络初始化会确保 `lo` 存在。无真实 NIC 时，`lo` 作为 active runtime 接口，配置 `127.0.0.1/8` 且不配置默认网关。
