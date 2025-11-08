# 用户态程序源码目录 (user)

该目录包含可被内核嵌入并通过 `execve` 运行的 RISC-V 64 用户程序与支持库。

## 目录结构
- `lib/`：通用用户态支持库（系统调用封装等）
  - `src/lib.rs`：导出接口
  - `src/syscall.S`：RISC-V 汇编实现基础 syscall 调用入口（`ecall`）
- `hello/`：示例程序
  - `src/main.rs`：简单输出示例

## 构建
默认目标：`riscv64gc-unknown-none-elf`  
示例：
```bash
cd ../hello
cargo build --release --target riscv64gc-unknown-none-elf
```
产物：`target/riscv64gc-unknown-none-elf/release/hello` (ELF)

## 与内核集成
目前内核通过 `include_bytes!` 方式嵌入生成的 ELF（已使用 `repr(align(8))` 保证 64 位 ELF 头 8 字节对齐）。添加新程序步骤：
1. 在此目录新增 `<prog>/` crate（`cargo new --bin <prog>`）
2. 构建生成 ELF
3. 在内核 `smfs.rs` 中新增静态对齐包装：
   ```rust
   #[repr(align(8))]
   struct Align8<const N: usize>([u8; N]);
   static PROG: Align8<{ include_bytes!("../../../user/<prog>/target/riscv64gc-unknown-none-elf/release/<prog>").len() }> =
       Align8(*include_bytes!("../../../user/<prog>/target/riscv64gc-unknown-none-elf/release/<prog>"));
   ```
4. 加入文件表 `STATIC_FILES`：
   ```rust
   FileEntry { name: "<prog>", data: &PROG.0, size: PROG.0.len() }
   ```
5. 内核中调用：`kernel_execve("<prog>", &["<prog>"], &[])`

## 约定 / ABI
- 入口符号 `_start` 由编译产物生成（Rust 默认运行时已裁剪），内核设置 `sepc` 至 ELF 入口。
- `main(argc, argv, envp)` 参数寄存器：
  - a0 = argc
  - a1 = argv 指针数组基址
  - a2 = envp 指针数组基址
- 栈 16 字节对齐，`argv[]` / `envp[]` 以 NULL 终止，字符串以 `\0` 结束。

## 系统调用
库中汇编通过：
- a7：系统调用号
- a0..a6：参数
- `ecall` 进入内核
示例（Rust 侧包装）：
```rust
pub fn write(fd: usize, buf: *const u8, len: usize) -> isize {
    unsafe { syscall3(SYS_WRITE, fd, buf as usize, len) }
}
```

## 添加更多 syscall
1. 在用户库定义号（与内核保持同步）
2. 新增封装函数
3. 内核 `syscall` 分发表实现对应处理

## 调试
- 使用 `objdump -d <elf>` 确认入口与指令
- GDB 连接内核后可在用户入口地址设置断点（见 ELF 反汇编）

## 注意事项
- 不使用标准库：确认 `Cargo.toml` 中设置 `#![no_std]`。
- 避免使用会引入运行时初始化的特性。
- 禁止在用户程序中假设虚拟地址常量（由内核决定映射布局）。

## 后续扩展建议
- 抽象 syscall 封装为安全 API（返回 Result）
- 增加基础运行时：panic → exit
- 提供轻量字符串 / I/O 辅助库
