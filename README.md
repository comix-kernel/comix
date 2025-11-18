# Comix

Comix 是一个用 Rust 编写的教学/实验型 RISC-V 64 位内核

## 目标

1. 循序实现从最小可运行内核到具备基础进程 / 虚拟内存 / 系统调用 / 文件系统的原型，并保持结构清晰与可重构性。
2. 支持musl
3. 实现自举
4. 兼容Linux ABI

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
- IPC
- 虚拟文件系统
- [ ] 设备管理
- [ ] I/O 与驱动程序
- [ ] 安全性与权限
- [ ] 磁盘文件系统
- [ ] 网络协议栈

## 目录结构

```
/                         项目根
  document/               设计文档与开发指南
    README.md             文档维护说明
    SUMMARY.md            mdBook 目录
    ...                   各子系统文档（arch/kernel/mm/ipc/log/sync/...）

  os/                     内核源码与构建脚本（Rust crate）
    Cargo.toml            内核 crate 定义
    build.rs              构建脚本
    Makefile              构建/运行辅助
    qemu-run.sh           在 QEMU 上运行内核的脚本
    rust-toolchain.toml   工具链版本钉死
    rustfmt.toml          Rust 格式化配置
    .cargo/
      config.toml         构建/目标配置
    src/
      arch/               架构相关抽象
        riscv/            RISC-V 支持（常量/启动/中断/陷阱/内核/内存/平台/系统调用/定时器）
        loongarch/        LoongArch（占位）
      kernel/             核心：CPU/任务/调度器/系统调用入口
      mm/                 内存管理：地址/页帧/全局堆/内存空间/页表
      ipc/                进程间通信：pipe/message/shared_memory/signal
      fs/                 简单文件系统实现（simple_fs/smfs）与测试
      vfs/                虚拟文件系统框架与实现（管道/stdio/磁盘文件）
      devices/            设备抽象与内存盘
      sync/               同步原语：自旋锁/互斥/中断保护
      log/                日志系统
      tool/               工具与通用数据结构（环形缓冲/用户缓冲/字符串）
      test/               内核测试支撑
      main.rs             入口
      linker.ld           链接脚本
    target/               构建产物（可忽略）
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
- 仔细阅读[编码规范](./CONTRIBUTING.md)

## 许可证

[GPL3.0](./LICENSE)
