# 用户程序栈布局（Stack Layout）

本文档说明内核在为用户程序构造初始用户栈（execve / spawn）时的内存布局、不变量和实现注意点。将原来散落在代码中的长注释集中到文档，便于维护与校验。

## 概览（栈增长方向）
- RISC‑V / 本项目栈向下增长：栈顶为低地址，栈底为高地址。
- 内核在构造用户栈时从高地址向低地址压入字符串、指针数组和 argc，然后把最终对齐的栈指针写入 TrapFrame.x2_sp（用户 sp）。
- main 的调用约定（由内核在 TrapFrame 中设置）：
  - a0 = argc
  - a1 = argv（指向指针数组的首地址）
  - a2 = envp（指向指针数组的首地址）

## 典型布局（高地址 → 低地址）
（注：示例中 argv.len() == 4, envp.len() == 3）

（高地址 — 栈底）
+-----------------------+
| ...                   |
+-----------------------+
| "USER=john"           |  <-- envp[2] 指向这里
+-----------------------+
| "HOME=/home/john"     |  <-- envp[1] 指向这里
+-----------------------+
| "SHELL=/bin/bash"     |  <-- envp[0] 指向这里
+-----------------------+
| "hello world"         |  <-- argv[3] 指向这里
+-----------------------+
| "arg2"                |  <-- argv[2] 指向这里
+-----------------------+
| "arg1"                |  <-- argv[1] 指向这里
+-----------------------+
| "./prog"              |  <-- argv[0] 指向这里
+-----------------------+  <-- 字符串存储区域开始
| ...                   |
+-----------------------+  <-- 进入 main 时的栈指针 (sp) 附近
| char* envp[3] (NULL)  |
+-----------------------+
| char* envp[2]         | --> 指向上面的 "USER=john"
| char* envp[1]         | --> 指向上面的 "HOME=/home/john"
| char* envp[0]         | --> 指向上面的 "SHELL=/bin/bash"
+-----------------------+
| char* argv[4] (NULL)  |
+-----------------------+
| char* argv[3]         | --> 指向上面的 "hello world"
| char* argv[2]         | --> 指向上面的 "arg2"
| char* argv[1]         | --> 指向上面的 "arg1"
| char* argv[0]         | --> 指向上面的 "./prog"
+-----------------------+
| int argc              |  // 在本内核实现中通常通过寄存器 a0 传递
+-----------------------+
| Return Address        |
+-----------------------+  <-- main 的栈帧开始
（低地址 — 栈顶）

## 要求与不变量
- 指针与整数按机器字（usize）对齐；最终用户 sp 必须满足 ABI 要求（本项目要求 16 字节对齐）。
- argv 与 envp 指针数组必须以 NULL 结尾：`argv[argc] == NULL`，`envp[n] == NULL`。
- 所有字符串必须以 NUL (0) 结尾。
- 内核写入用户栈时须确保用户地址可写：
  - 要么在写之前已激活用户页表并临时允许 SUM（sstatus.SUM = 1），
  - 要么通过封装的 copy_to_user 接口（推荐），将页面错误映射为 -EFAULT。
- 在写字符串或指针前，必须保证目标页已被映射（MemorySpace::from_elf 应已完成映射），否则会触发页故障（Load/Store Page Fault）。

## 实现顺序建议（从高地址向低地址）
1. 将 current_sp 设为用户栈的“高地址”（stack_top）。
2. 先按 reverse order 将 env 字符串写入（每个字符串后写 NUL），记录字符串地址到 env_ptrs。
3. 再按 reverse order 将 argv 字符串写入，记录地址到 arg_ptrs。
4. 按机器字对 current_sp 做对齐（word 对齐）。
5. 写入 envp 的 NULL 终止器（写入 0）。
6. 逆序写入 env_ptrs，使 envp[0] 在最低地址；记录 envp_vec_ptr。
7. 写入 argv 的 NULL 终止器（写入 0）。
8. 逆序写入 arg_ptrs，使 argv[0] 在最低地址；记录 argv_vec_ptr。
9. 写入 argc（如果非寄存器传递）。
10. 对最终 current_sp 做 ABI 对齐（16 字节），并将其作为用户 sp 写入 TrapFrame.x2_sp。
11. 在 TrapFrame 中设置 sepc（入口 PC）、sstatus（SPP=U, SPIE=1）、kernel_sp、a0/a1/a2 等寄存器，并清零 ra（避免从用户态返回到内核）。

## 常见陷阱
- 未对齐 pointer 数组或最终 sp：会导致 libc / 程序行为异常或非法指令错误。
- 在尚未切换到用户页表或没有开启 SUM 的情况下直接向用户地址写入，会在 trap_entry 或 execve 过程中触发页面错误（Store/Load Page Fault）。解决办法：先 activate(new_space.root_ppn())，再 write；或使用 copy_to_user。
- 将字符串或指针写到错误的地址（off-by-one）会破坏栈布局并难以调试。建议在测试中验证 argv/envp 指针能正确 deref。
- 在构造堆栈时务必记录并使用写入时的实际虚拟地址（不要使用临时计算出的物理地址）。

## 安全建议与封装
- 不要在多个位置散写 SUM 的设置/清除；把用户内存访问集中到 `user_mem::copy_to_user` / `copy_from_user`：
  - 该函数负责开启 SUM、逐页写入并在失败时返回 Err(UserCopyError::Fault)。
- 在 execve 路径中：
  - 先构造 MemorySpace 并完成段映射（包含用户栈页）。
  - activate(new_space.root_ppn()) 切换页表（使内核可以通过 SUM 访问 U 页）。
  - 再执行栈构造与 TrapFrame 写入流程。

## 验证与测试
- 单元测试中可提供 helper：`new_dummy_memory_space_with_stack()` 返回一个可写的 MemorySpace，便于验证栈布局写入后的读取正确性。
- 在集成/仿真测试中：
  - 验证用户入口处的指令字节非零；
  - 在用户程序中读取 argv/envp 并打印，确认内核构造无误。

## 参考示例（伪代码）
```rust
// 假设 new_space 已激活
let mut sp = stack_top;
for s in envp.iter().rev() { sp -= s.len()+1; write_user(sp, s); env_ptrs.push(sp); }
for s in argv.iter().rev() { sp -= s.len()+1; write_user(sp, s); arg_ptrs.push(sp); }
sp &= !(usize::BITS as usize/8 - 1); // word-align
// 写 envp NULL 与指针数组...
// 最终 sp 对齐到 16 字节后写入 TrapFrame.x2_sp
```
