# API 文档

正式设计文档只保留模块边界和关键流程，公共 API 细节以 rustdoc 为准。

## 在线入口

- [RISC-V64 rustdoc](https://comix-kernel.github.io/comix/api/os/)

## 本地生成

```bash
cd os
cargo doc --no-deps --target riscv64gc-unknown-none-elf
```

LoongArch64 代码仍在演进中。面向架构差异的设计说明见 `document/arch/`，具体 API 以当前目标架构的 rustdoc 和源码为准。
