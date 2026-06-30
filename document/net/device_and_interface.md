# 设备与接口

设备层和接口层是网络控制面的入口。`NetDevice` 提供帧收发, `NetworkInterface` 保存名称, MAC, IP 和网关等控制面状态。

## 当前状态

- VirtIO net 初始化成功后调用 `crate::net::register_net_device(device)`。
- 注册入口根据设备 id 生成 `ethN` 名称。
- `NetworkInterface` 被加入 `NETWORK_INTERFACE_MANAGER`。
- `NetDriverHandle` 作为兼容 driver 注册到设备框架, 用于中断分发。
- 默认配置会确保 `lo` 存在。

## 目标

- 保持设备驱动只依赖 `NetDevice` 抽象。
- 让接口对象负责控制面配置, 不直接成为设备框架 driver。
- 通过兼容桥处理旧中断注册路径, 避免污染接口抽象。

## 非目标

- `NetworkInterface` 不直接操作 socket set。
- 设备驱动不配置 IP, 网关或 loopback 策略。
- `NullNetDevice` 不代表 loopback。

## register_net_device 流程

1. 用设备 id 构造接口名, 如 `eth0`。
2. 创建 `NetworkInterface` 并保存底层 `NetDevice` 引用。
3. 调用设备子系统注册底层 net device。
4. 把 interface 加入 `NETWORK_INTERFACE_MANAGER`。
5. 创建 `NetDriverHandle` 并注册为设备框架 driver, 供中断路径调用。

## NetworkInterface 边界

`NetworkInterface` 保存:

- name。
- MAC address。
- 底层 `NetDevice`。
- IP CIDR 列表。
- IPv4 gateway。
- 中断兼容状态和最后中断时间。

它可以创建 `SmoltcpInterface` wrapper, 但这个工厂是当前单 runtime 初始化的兼容边界, 不是多接口运行时模型。

## Loopback 初始化

`NetworkConfigManager::ensure_loopback_interface()` 确保 `lo` 存在并配置 `127.0.0.1/8`。如果没有真实 NIC, 默认配置选择 `lo` 作为 active runtime, 且不设置默认网关。若存在真实 NIC, 当前会在选中接口上同时配置默认地址和 loopback CIDR。

## 中断协作

`NetDriverHandle::try_handle_interrupt()`:

- 检查接口中断开关。
- 记录中断事件和时间。
- 尝试从底层设备读取一帧。
- 唤醒 poll waiters。

它不维护第二份协议栈状态。真正协议推进仍在进程上下文中的 `NetworkStack::poll()` 或网络 I/O 路径完成。

## 已知限制

- `last_interrupt_time` 当前用简单递增时间模拟。
- 接口 registry 是简单 Vec, 没有 namespace 或复杂路由策略。
- 默认配置是测试友好的静态配置, 不是通用 DHCP/用户配置系统。

## 源码索引

- `os/src/net/mod.rs`: `register_net_device()`。
- `os/src/net/interface.rs`: 接口对象, registry, `NetDriverHandle`。
- `os/src/net/config.rs`: 默认配置和 loopback 保证。
- `os/src/device/net/virtio_net.rs`: VirtIO net 注册入口。
- `os/src/device/net/loopback.rs`: 显式 loopback 设备。
