# Comix

Comix 是一个用 Rust 编写的教学/实验型 RISC-V 64 位内核

## 目标

1. 循序实现从最小可运行内核到具备基础进程 / 虚拟内存 / 系统调用 / 文件系统的原型，并保持结构清晰与可重构性。
2. 实现自举
3. 兼容Linux ABI

## 特性概览

- 启动与引导：RISC-V 裸机入口、分页开启、陷入向量安装
- 陷入与中断：统一 trap 入口 (`trap_entry.S`) 与 Rust 处理框架（区分用户 / 内核态）
- 系统调用：基础号，可扩展宏包装
- 任务与调度：内核线程、用户任务骨架、简单 RR 调度器
- 内存管理：
  - 物理页帧分配器
  - 多级页表封装与地址空间切换
  - ELF 加载（用户程序段映射与入口设置）
  - 用户栈构建（argc/argv/envp 布局）
- 同步原语：自旋锁、睡眠锁、屏蔽中断守卫；
- 简易“内存文件系统”（嵌入 ELF，保证 8 字节对齐）
- 日志与调试：分级日志、GDB 可用符号、SBI 控制台输出
- [ ] IPC
- [ ] 虚拟文件系统
- [ ] 设备管理
- [ ] I/O 与驱动程序
- [ ] 安全性与权限
- [ ] 磁盘文件系统
- [ ] 网络协议栈

## 目录结构

```
os/                    内核源码主目录
  arch/                架构相关（当前重点：riscv）
  kernel/              核心：任务/调度/系统调用/CPU
  mm/                  内存管理（页表/分配器/地址类型）
  fs/                  简单内存文件系统（smfs）
  sync/                同步原语
  log/                 日志系统
  vfs/                 VFS 框架雏形
document/              设计与说明（待补充集成）
qemu-run.sh            启动脚本
Cargo.toml             构建定义
```

## 快速开始

### 依赖

1. 本地运行

- Rust nightly（`rust-toolchain.toml` 已固定）
- RISC-V 目标工具链：`rustup target add riscv64gc-unknown-none-elf`
- QEMU

2. 容器化

- Docker
- DevContainer相关插件

### 构建 & 运行

```bash
cd os
make run
```

若需调试：

```bash
# 启动（监听 gdb）
cd os
make debug

# 另一个终端
cd os
make gdb
```

## 近期路线

| 优先级 | 任务 |
|--------|------|
| P0 | 完成稳定的 ELF 加载 + 用户复制 API + trap_entry 重构 |
| P1 | 丰富系统调用（yield / getpid / read / open / close）与任务退出回收 |
| P1 | 定时器中断与时间片调度 |
| P2 | 文件描述符表 + VFS 打开流程 |
| P2 | fork + 基础 COW |
| P3 | 信号 / 管道 / 动态内存（brk / mmap） |

## 贡献

欢迎提交 PR / Issue：  
- 可以阅读[document](./document/README.md)中的文档快速了解项目
- 提交pr时参照.github中的 issue 与 pr 模板
- 仔细阅读[编码规范](./CONTRIBUTING.md)

## 许可证

[GPL3.0](./LICENSE)
