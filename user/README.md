# 用户态程序源码目录 (user)

该目录包含可选的 RISC-V 64 `no_std` 示例程序和一层很薄的系统调用库。评测 rootfs 不从这里生成，`make build` / `make all` 也不会编译这里的 crate。

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

仓库根执行 `make build-user` 会在本目录运行 `make`，并把可执行文件移到 `user/bin/`。`make pack-simple-fs` 再用 `scripts/make_init_simple_fs.py` 生成 `os/simple_fs.img`。

内核构建本身不读取 `user/bin`。`os/build.rs` 明确跳过用户程序编译，并嵌入一份空的 SimpleFS 占位镜像。`os/src/fs/smfs.rs` 里的 `STATIC_FILES` 也是空表。

添加示例程序：在本目录新增 crate，然后重新执行 `make build-user`。若要放进 SimpleFS 调试镜像，再执行 `make pack-simple-fs`。这不会把它放进 ext4 rootfs。

## 约定 / ABI

- 入口符号 `_start` 由编译产物生成。内核把 `sepc` 设为 ELF 入口。
- RISC-V `execve` 遵循 Linux 约定：`argc`、`argv`、`envp` 在用户栈上，进入用户态时 `a0`/`a1`/`a2` 为 0。`a0` 留给静态 glibc 当作 `rtld_fini`。
- 栈 16 字节对齐，`argv[]` / `envp[]` 以 NULL 终止，字符串以 `\0` 结束。
- 本目录的 `Makefile` 只构建 `riscv64gc-unknown-none-elf`。

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
