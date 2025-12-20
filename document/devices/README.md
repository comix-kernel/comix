# 设备与驱动概览

面向内核贡献者的设备子系统说明，覆盖驱动模型、设备树探测、VirtIO 适配、块/网/控制台/RTC 等核心组件。架构与代码入口位于 os/src/device/ 下。

## 驱动模型与注册表

- 抽象：所有驱动实现 `Driver`，统一提供 `try_handle_interrupt`、`device_type`、`get_id`，并通过可选的 `as_block/as_net/as_rtc/as_serial` 返回具体接口。
- 注册：全局表 `DRIVERS/BLK_DRIVERS/RTC_DRIVERS/SERIAL_DRIVERS`（见 os/src/device/mod.rs）存放已初始化驱动；`register_driver()` 用于统一登记并在中断路径可遍历。
- 中断派发：`IRQ_MANAGER`（根级）基于中断号或全局列表调用驱动的 `try_handle_interrupt`（见 os/src/device/irq/mod.rs）。

## 设备树探测流程

- 引导期将 DTP 指针指向内核可见的 FDT，`device_tree::init()` 解析 CPU/时钟/内存信息并读取 `bootargs`（见 os/src/device/device_tree.rs）。
- `DEVICE_TREE_REGISTRY` 按 `compatible` 注册探测函数；初始化时分两轮遍历：先初始化中断控制器，再初始化其他设备。
- `DEVICE_TREE_INTC` 保存 phandle→中断控制器驱动映射，供设备解析其 `interrupts` 属性时使用。

## 中断控制器：PLIC

- 驱动位于 os/src/device/irq/plic.rs，使用 MMIO 寄存器完成 claim/complete。
- 初始化：在 device tree 中匹配 `riscv,plic0`，映射 MMIO，注册到根 `IRQ_MANAGER` 的 `SUPERVISOR_EXTERNAL` 路径。
- 提供 `IntcDriver::register_local_irq` 辅助驱动将中断号与处理器上下文绑定。

## 总线与 VirtIO 传输

- bus/virtio_mmio.rs、bus/pcie.rs 提供传输层占位/适配（目前主要使用 VirtIO MMIO）。
- VirtIO 设备驱动共享 `VirtIOHal`（os/src/device/virtio_hal.rs）作为 DMA/内存屏障实现。

## 块设备

- 接口：`BlockDriver` trait（os/src/device/block/mod.rs）定义读写/flush/块大小/容量。
- RAMDisk：`ram_disk.rs`，纯内存实现，用于测试或引导阶段；无中断，支持读写与容量查询。
- VirtIO-Block：`virtio_blk.rs`，基于 virtio-drivers 的 `VirtIOBlk`；初始化后注册到 `DRIVERS`、`BLK_DRIVERS`、`IRQ_MANAGER`。块大小 512 字节，容量由设备报告。
- 文件系统集成：VFS/ext4 通过 `BlockDriver` 适配层访问块设备，首次构建会生成 ext4 镜像 `fs.img` 并通过 virtio-blk 挂载。

## 网络设备

- 接口：`NetDevice` trait（os/src/device/net/net_device.rs）提供 send/receive/MTU/MAC 信息。
- VirtIO-Net：`virtio_net.rs` 使用 `VirtioNetDevice` 包装 virtio-drivers 的实现，默认 MTU 1500。`init()` 创建设备后同时：
  - 加入 `NETWORK_DEVICES` 列表；
  - 创建 `NetworkInterface`（os/src/net/interface.rs）并注册到接口管理器；
  - 通过 `register_driver` 让 IRQ 路径可见。

## 控制台与串口

- 接口：`Console` trait（os/src/device/console/mod.rs）提供读写/flush；`CONSOLES` 与 `MAIN_CONSOLE` 管理活动控制台。
- 实现：`uart_console.rs`、`frame_console.rs`（后者可用于图形帧缓冲输出）。串口驱动还可通过 `SerialDriver`（os/src/device/serial/mod.rs）统一暴露给 VFS/日志。

## RTC 与时间

- RTC 驱动接口在 os/src/device/rtc/mod.rs，当前实现 `rtc_goldfish.rs` 对接 virtio/goldfish RTC（用于墙钟时间/定时）。

## 其他占位

- GPU：`gpu/virtio_gpu.rs` 占位实现，提供未来图形输出路径。
- 输入：`input/virtio_input.rs` 占位，为键鼠/触摸设备预留。
- IRQ：`irq/mod.rs` 定义通用中断管理逻辑，除 PLIC 外可扩展本地或板级控制器。

## 初始化顺序（概览）

1. device_tree::init() 解析 FDT，注册 compatible→init 钩子。
2. 先初始化中断控制器（如 PLIC），完成根 IRQ 管理器设置。
3. 逐个设备匹配 compatible：VirtIO-MMIO → virtio-blk / virtio-net / virtio-gpu / virtio-input / virtio-console 等。
4. 驱动完成自注册：加入 DRIVERS/子列表，必要时在 IRQ_MANAGER 中登记。
5. 上层子系统使用对应 trait（BlockDriver/NetDevice/Console/SerialDriver/RTC）完成挂载与服务暴露。

## 调试提示

- 查看已注册驱动：在调试或日志中读取 DRIVERS/BLK_DRIVERS/NETWORK_DEVICES 等全局表。
- 中断无法响应：确认 PLIC `register_local_irq` 是否被调用、IRQ 号与设备树一致、`IRQ_MANAGER.try_handle_interrupt` 返回路径。
- 块设备异常：检查 `fs.img` 是否生成、块大小与 `config::VIRTIO_BLK_SECTOR_SIZE` 保持一致。
- 网络收发异常：确认 virtio-net 设备已添加到接口管理器、MTU 未超限，并检查队列是否因 `QueueFull/QueueEmpty` 返回错误。
