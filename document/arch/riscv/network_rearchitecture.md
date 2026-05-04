# RISC-V 网络重构说明

RISC-V 网络重构不需要在网络核心中引入架构差异。架构层只负责 syscall 分发、trap 上下文和用户指针访问前置条件。

## 规则

- syscall 号和参数寄存器差异只放在 `os/src/arch/riscv/`。
- `os/src/kernel/syscall/network.rs` 接收架构无关的参数值。
- 网络核心不得根据 RISC-V 条件编译分叉。
