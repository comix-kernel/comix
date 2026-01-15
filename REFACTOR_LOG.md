# Comix 内核重构日志

## 重构概览

| 项目 | 内容 |
|------|------|
| 开始日期 | 2026-01-15 |
| 当前阶段 | Phase 1: 地址类型安全化 |
| 重构方案 | REFACTOR_PLAN.md v2.0 (精简版) |

---

## Phase 1: 地址类型安全化

### 目标
增强现有 `os/src/mm/address/`，使其更类型安全，**不需要重写所有代码**

### 验收标准
- ✅ `make build` 编译通过
- ✅ 新代码使用 `PA`/`VA`/`UA` 类型
- ✅ 旧代码保持兼容（无破坏性更改）
- ✅ `make test` 通过所有测试

---

## 修改记录

### 待执行任务

- [ ] Step 1.1: 增强地址类型（1-2 天）
- [ ] Step 1.2: 逐步迁移（3-4 天）
- [ ] Step 1.3: 编译器辅助（1 天）

---

## 变更历史

### 2026-01-15 - 开始 Phase 1

**状态**: 🟡 进行中

**任务**: 开始执行 Step 1.1 - 增强地址类型

**计划**:
1. 检查当前 `os/src/mm/address/` 的实现
2. 添加 `PA`（物理地址）类型
3. 添加 `UA`（用户地址）类型
4. 增强 `VA`（虚拟地址）类型别名
5. 实现类型转换方法
6. 运行编译测试验证

---

### 2026-01-15 - Step 1.1 完成 ✅

**状态**: ✅ 完成

**任务**: 增强地址类型，添加 UA/PA/VA 类型别名

**修改内容**:

#### 1. 添加 `Uaddr` 类型（用户地址）
- 文件：`os/src/mm/address/address.rs`
- 新增 `Uaddr` 结构体，语义上表示用户虚拟地址
- 实现 `from_vaddr()` 和 `to_vaddr()` 转换方法
- 实现指针转换方法：`from_ref()`, `from_ptr()`, `as_ref()`, `as_mut()`, `as_ptr()`, `as_mut_ptr()`
- 添加 `UaddrRange` 类型别名

#### 2. 添加类型别名
- 文件：`os/src/mm/address/mod.rs`
- `PA = Paddr` (物理地址)
- `VA = Vaddr` (虚拟地址)
- `UA = Uaddr` (用户地址)
- `PARange = PaddrRange`
- `VARange = VaddrRange`
- `UARange = UaddrRange`

#### 3. 增强 `Vaddr` 类型
- 文件：`os/src/mm/address/address.rs`
- 添加 `from_uaddr()` 方法（从用户地址创建虚拟地址）
- 添加 `to_uaddr()` 方法（将虚拟地址转换为用户地址）

**验证结果**:
- ✅ 编译成功：`make build` 通过
- ✅ 无编译错误
- ⚠️  有一些警告（与本次修改无关，为已存在的代码风格警告）

**代码统计**:
- 新增文件：0
- 修改文件：2
  - `os/src/mm/address/address.rs`: +92 行
  - `os/src/mm/address/mod.rs`: +7 行
- 新增类型：`Uaddr`, `UaddrRange`
- 新增类型别名：`PA`, `VA`, `UA`, `PARange`, `VARange`, `UARange`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：`#[repr(transparent)]` 确保与 `usize` 相同的内存布局
- ✅ 类型安全增强：`UA` 提供了用户地址的语义区分

**下一步**: Step 1.2 - 逐步迁移关键模块使用新类型

---

### 2026-01-15 - 创建地址类型使用指南

**状态**: ✅ 完成

**任务**: 创建文档说明如何在代码中使用新的地址类型

**修改内容**:

#### 新增文档
- 文件：`document/mm/address_types_guide.md`
- 内容：
  - PA/VA/UA 类型概述
  - 使用场景说明
  - 转换规则
  - 迁移指南
  - 示例代码

**验证结果**:
- ✅ 测试编译成功：`cargo build --target riscv64gc-unknown-none-elf --tests`
- ✅ 普通编译成功：`make build`

**影响分析**:
- ✅ 纯文档添加，不影响代码功能
- ✅ 为后续迁移提供参考指南

---

### Phase 1 进展总结

**当前状态**: 🟡 Step 1.1 完成，Step 1.2 进行中

**已完成**:
1. ✅ 添加 `Uaddr` (UA) 类型
2. ✅ 添加 PA/VA/UA 类型别名
3. ✅ 实现 VA <-> UA 转换方法
4. ✅ 创建地址类型使用指南
5. ✅ 验证编译通过

**待完成**:
- [ ] Step 1.2: 在关键模块中逐步使用新类型
  - [ ] os/src/mm/memory_space.rs - 地址空间操作
  - [ ] os/src/mm/page_table/ - 页表操作
  - [ ] os/src/arch/riscv64/mm/ - 架构相关内存操作
  - [ ] os/src/kernel/task/ - 进程内存布局
- [ ] Step 1.3: 添加编译器辅助检查

**当前风险**: 低
- 所有修改向后兼容
- 编译通过
- 测试框架正常工作

---

### 2026-01-15 - Step 1.1 提交 ✅

**Commit**: `49e7b60`

**Commit 信息**:
```
refactor(mm): 添加 UA 用户地址类型和 PA/VA/UA 类型别名

- 新增 Uaddr (UA) 类型用于语义区分用户虚拟地址
- 添加 PA/VA/UA 类型别名简化使用
- 实现 VA <-> UA 双向转换方法
- 添加地址范围类型 UARange
- 创建地址类型使用指南文档
- 所有类型零运行时开销 (#[repr(transparent)])
- 向后兼容，现有代码无需修改

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改统计**:
- 4 个文件修改
- +502 行，-1 行
- 新增文档：REFACTOR_LOG.md, document/mm/address_types_guide.md

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

---

### 2026-01-15 - Step 1.2 完成 ✅

**状态**: ✅ 完成

**任务**: 在关键模块中逐步使用新类型（memory_space.rs）

**修改内容**:

#### 1. 添加类型安全的 brk 函数
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 新增 `current_brk_ua()` 函数：返回 `Option<UA>` 而不是 `Option<usize>`
- 新增 `brk_ua()` 函数：接受和返回 `UA` 类型而不是 `usize`
- 保留原有的 `current_brk()` 和 `brk()` 函数以保持向后兼容

**代码示例**:
```rust
// 新增的类型安全函数
pub fn current_brk_ua(&self) -> Option<UA> {
    self.current_brk().map(UA::from_usize)
}

pub fn brk_ua(&mut self, new_brk: UA) -> Result<UA, PagingError> {
    self.brk(new_brk.as_usize()).map(UA::from_usize)
}
```

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/mm/memory_space/memory_space.rs`: +13 行
- 新增函数：2 个类型安全版本

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新函数仅是类型转换包装
- ✅ 类型安全增强：新代码可以使用 UA 类型标记用户地址

**迁移策略**:
- 采用渐进式迁移：添加新函数而不是修改现有函数
- 新代码优先使用类型安全版本（`*_ua()` 后缀）
- 旧代码保持不变，逐步迁移

**下一步**: 继续迁移其他模块或提交当前进度

---

### 2026-01-15 - Step 1.2 提交 ✅

**Commit**: `d9432fb`

**Commit 信息**:
```
refactor(mm): 添加类型安全的 brk 函数（UA 类型）

- 新增 current_brk_ua() 返回 Option<UA>
- 新增 brk_ua() 接受和返回 UA 类型
- 保留原有函数以保持向后兼容
- 采用渐进式迁移策略
- 零性能开销，仅类型转换包装

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改统计**:
- 2 个文件修改
- +72 行

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

---

### 2026-01-15 - Step 1.2 继续：添加 ELF 加载类型安全版本 ✅

**状态**: ✅ 完成

**任务**: 添加 from_elf 的类型安全版本

**Commit**: `87fb6ab`

**Commit 信息**:
```
refactor(mm): 添加 ELF 加载的类型安全版本 (from_elf_ua)

- 新增 ElfLoadResult 结构体封装 from_elf 返回值
- 新增 from_elf_ua() 方法返回类型安全的 UA 地址
- 新增 user_stack_top_ua() 辅助函数
- 保留原有 from_elf() 函数以保持向后兼容
- 零性能开销,仅类型转换包装

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 1. 添加 `ElfLoadResult` 结构体
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 封装 from_elf 的返回值，使用类型安全的 UA 地址
- 字段：
  - `space: MemorySpace` - 内存空间
  - `entry_point: UA` - 程序入口地址
  - `user_stack_top: UA` - 用户栈顶地址
  - `phdr_addr: UA` - 程序头地址
  - `ph_num: usize` - 程序头数量
  - `ph_ent: usize` - 程序头条目大小

#### 2. 添加 `from_elf_ua()` 方法
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 类型安全版本的 ELF 加载函数
- 返回 `Result<ElfLoadResult, PagingError>`
- 内部调用原有的 `from_elf()` 并转换返回值

#### 3. 添加 `user_stack_top_ua()` 辅助函数
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 返回类型安全的用户栈顶地址 `UA`

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/mm/memory_space/memory_space.rs`: +48 行
- 新增结构体：`ElfLoadResult`
- 新增函数：`from_elf_ua()`, `user_stack_top_ua()`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新函数仅是类型转换包装
- ✅ 类型安全增强：新代码可以使用 UA 类型标记用户地址

**迁移策略**:
- 采用渐进式迁移：添加新函数而不是修改现有函数
- 新代码优先使用类型安全版本（`from_elf_ua()`）
- 旧代码保持不变，逐步迁移

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

**下一步**: 继续 Step 1.2 - 迁移其他关键模块

---

### 2026-01-15 - Step 1.2 继续：添加 mmap/munmap 类型安全版本 ✅

**状态**: ✅ 完成

**任务**: 添加 mmap 和 munmap 的类型安全版本

**Commit**: `b7e4ac8`

**Commit 信息**:
```
refactor(mm): 添加 mmap/munmap 的类型安全版本

- 新增 mmap_ua() 接受 Option<UA> hint，返回 UA
- 新增 munmap_ua() 接受 UA 起始地址
- 保留原有函数以保持向后兼容
- 零性能开销，仅类型转换包装

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 1. 添加 `mmap_ua()` 方法
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 类型安全版本的 mmap 函数
- 参数：
  - `hint: Option<UA>` - 建议的起始地址（None = 由内核选择）
  - `len: usize` - 长度（字节）
  - `pte_flags: UniversalPTEFlag` - 页表项标志
- 返回：`Result<UA, PagingError>` - 映射的起始地址

#### 2. 添加 `munmap_ua()` 方法
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 类型安全版本的 munmap 函数
- 参数：
  - `start: UA` - 起始地址
  - `len: usize` - 长度（字节）
- 返回：`Result<(), PagingError>`

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/mm/memory_space/memory_space.rs`: +34 行
- 新增函数：`mmap_ua()`, `munmap_ua()`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新函数仅是类型转换包装
- ✅ 类型安全增强：新代码可以使用 UA 类型标记用户地址
- ✅ API 改进：mmap_ua 使用 Option<UA> 更符合 Rust 习惯

**迁移策略**:
- 采用渐进式迁移：添加新函数而不是修改现有函数
- 新代码优先使用类型安全版本（`mmap_ua()`, `munmap_ua()`）
- 旧代码保持不变，逐步迁移

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

**下一步**: 继续 Step 1.2 - 迁移其他关键模块（page_table, arch/mm, kernel/task）

---

### 2026-01-15 - Step 1.2 继续：添加 mprotect 类型安全版本 ✅

**状态**: ✅ 完成

**任务**: 添加 mprotect 的类型安全版本

**Commit**: `33cba2b`

**Commit 信息**:
```
refactor(mm): 添加 mprotect 的类型安全版本

- 新增 mprotect_ua() 接受 UA 起始地址
- 保留原有函数以保持向后兼容
- 零性能开销，仅类型转换包装

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 添加 `mprotect_ua()` 方法
- 文件：`os/src/mm/memory_space/memory_space.rs`
- 类型安全版本的 mprotect 函数
- 参数：
  - `start: UA` - 起始地址（必须页对齐）
  - `len: usize` - 长度（字节）
  - `prot: UniversalPTEFlag` - 新的保护标志
- 返回：`Result<(), PagingError>`

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/mm/memory_space/memory_space.rs`: +19 行
- 新增函数：`mprotect_ua()`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新函数仅是类型转换包装
- ✅ 类型安全增强：新代码可以使用 UA 类型标记用户地址

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

---

### 2026-01-15 - Step 1.2 memory_space.rs 迁移总结 ✅

**状态**: ✅ 完成

**任务**: 完成 memory_space.rs 中主要用户地址函数的类型安全迁移

**已完成的类型安全函数**:

1. **堆管理**:
   - `current_brk_ua()` - 返回 `Option<UA>`
   - `brk_ua()` - 接受和返回 `UA`

2. **栈管理**:
   - `user_stack_top_ua()` - 返回 `UA`

3. **ELF 加载**:
   - `ElfLoadResult` 结构体 - 封装所有用户地址为 `UA`
   - `from_elf_ua()` - 返回 `ElfLoadResult`

4. **内存映射**:
   - `mmap_ua()` - 接受 `Option<UA>` hint，返回 `UA`
   - `munmap_ua()` - 接受 `UA` 起始地址
   - `mprotect_ua()` - 接受 `UA` 起始地址

**总计**:
- 新增结构体：1 个（`ElfLoadResult`）
- 新增函数：7 个（所有带 `_ua` 后缀）
- 修改行数：约 +155 行
- 提交次数：4 次

**迁移策略验证**:
- ✅ 渐进式迁移成功：所有新函数与旧函数并存
- ✅ 零破坏性：现有代码无需修改
- ✅ 零性能开销：所有类型转换在编译时完成
- ✅ 类型安全增强：用户地址现在有明确的类型标记

**下一步**: 继续 Step 1.2 - 迁移其他模块
- [ ] os/src/mm/page_table/ - 页表操作
- [ ] os/src/arch/riscv64/mm/ - 架构相关内存操作
- [ ] os/src/kernel/task/ - 进程内存布局

---

### 2026-01-15 - Step 1.2 继续：添加架构层地址转换类型安全版本 ✅

**状态**: ✅ 完成

**任务**: 添加 RISC-V 架构层地址转换函数的类型安全版本

**Commit**: `32e7d17`

**Commit 信息**:
```
refactor(arch/riscv/mm): 添加地址转换的类型安全版本

- 新增 vaddr_to_paddr_typed() 接受 VA，返回 PA
- 新增 paddr_to_vaddr_typed() 接受 PA，返回 VA
- 保留原有函数以保持向后兼容
- 零性能开销，仅类型转换包装

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 1. 添加 `vaddr_to_paddr_typed()` 函数
- 文件：`os/src/arch/riscv/mm/mod.rs`
- 类型安全版本的虚拟地址到物理地址转换
- 参数：`vaddr: VA` - 虚拟地址
- 返回：`PA` - 物理地址
- 标记为 `unsafe`（与原函数一致）

#### 2. 添加 `paddr_to_vaddr_typed()` 函数
- 文件：`os/src/arch/riscv/mm/mod.rs`
- 类型安全版本的物理地址到虚拟地址转换
- 参数：`paddr: PA` - 物理地址
- 返回：`VA` - 虚拟地址

#### 3. 导入 `UsizeConvert` trait
- 文件：`os/src/arch/riscv/mm/mod.rs`
- 添加 `use crate::mm::address::UsizeConvert;`
- 使 `as_usize()` 和 `from_usize()` 方法可用

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/arch/riscv/mm/mod.rs`: +28 行
- 新增函数：`vaddr_to_paddr_typed()`, `paddr_to_vaddr_typed()`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新函数仅是类型转换包装
- ✅ 类型安全增强：地址转换现在有明确的类型标记
- ✅ 架构层基础：为上层代码提供类型安全的地址转换

**技术说明**:
- 原函数是 `const fn`，但类型安全版本不是 `const`
- 原因：`UsizeConvert` trait 的方法不是 `const`
- 影响：类型安全版本不能在编译时常量中使用，但这不影响运行时使用

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

**下一步**: 继续 Step 1.2 - 迁移其他模块
- [x] os/src/arch/riscv64/mm/ - 架构相关内存操作 ✅
- [ ] os/src/mm/page_table/ - 页表操作
- [ ] os/src/kernel/task/ - 进程内存布局

---

### 2026-01-15 - Step 1.2 继续：添加 PreparedExecImage 类型安全版本 ✅

**状态**: ✅ 完成

**任务**: 添加 PreparedExecImage 结构体的类型安全版本

**Commit**: `a2a9ed9`

**Commit 信息**:
```
refactor(task): 添加 PreparedExecImage 的类型安全版本

- 添加 PreparedExecImageUA 结构体，使用 UA 类型表示用户地址
- 添加 PreparedExecImage::to_ua() 转换方法
- 保持原有 PreparedExecImage 结构体不变，确保向后兼容

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 1. 添加 `PreparedExecImageUA` 结构体
- 文件：`os/src/kernel/task/exec_loader.rs`
- 类型安全版本的 ELF 加载结果
- 字段：
  - `space: MemorySpace` - 内存空间
  - `initial_pc: UA` - 初始 PC（程序入口或动态链接器入口）
  - `user_sp_high: UA` - 用户栈顶地址
  - `at_base: UA` - auxv AT_BASE（动态链接器 load bias）
  - `at_entry: UA` - auxv AT_ENTRY（主程序入口）
  - `phdr_addr: UA` - 程序头地址
  - `phnum: usize` - 程序头数量（非地址）
  - `phent: usize` - 程序头条目大小（非地址）

#### 2. 添加 `PreparedExecImage::to_ua()` 方法
- 文件：`os/src/kernel/task/exec_loader.rs`
- 将原始结构体转换为类型安全版本
- 消耗原结构体（`self`），返回 `PreparedExecImageUA`

**代码示例**:
```rust
pub struct PreparedExecImageUA {
    pub space: MemorySpace,
    pub initial_pc: crate::mm::address::UA,
    pub user_sp_high: crate::mm::address::UA,
    pub at_base: crate::mm::address::UA,
    pub at_entry: crate::mm::address::UA,
    pub phdr_addr: crate::mm::address::UA,
    pub phnum: usize,
    pub phent: usize,
}

impl PreparedExecImage {
    pub fn to_ua(self) -> PreparedExecImageUA {
        use crate::mm::address::UA;
        PreparedExecImageUA {
            space: self.space,
            initial_pc: UA::from_usize(self.initial_pc),
            user_sp_high: UA::from_usize(self.user_sp_high),
            at_base: UA::from_usize(self.at_base),
            at_entry: UA::from_usize(self.at_entry),
            phdr_addr: UA::from_usize(self.phdr_addr),
            phnum: self.phnum,
            phent: self.phent,
        }
    }
}
```

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/kernel/task/exec_loader.rs`: +32 行
- 新增结构体：`PreparedExecImageUA`
- 新增方法：`PreparedExecImage::to_ua()`

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新结构体仅是类型转换包装
- ✅ 类型安全增强：ELF 加载结果中的用户地址现在有明确的类型标记
- ✅ 语义清晰：区分地址字段（UA）和非地址字段（usize）

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

**下一步**: 继续 Step 1.2 - 迁移其他模块
- [x] os/src/arch/riscv64/mm/ - 架构相关内存操作 ✅
- [x] os/src/kernel/task/exec_loader.rs - ELF 加载结果 ✅
- [ ] os/src/mm/page_table/ - 页表操作
- [ ] os/src/kernel/task/ - 其他进程内存布局相关代码

---

### 2026-01-15 - Step 1.2 继续：添加 TaskStruct 线程 ID 地址类型安全访问方法 ✅

**状态**: ✅ 完成

**任务**: 添加 TaskStruct 中线程 ID 地址字段的类型安全访问方法

**Commit**: `890a596`

**Commit 信息**:
```
refactor(task): 添加 TaskStruct 线程 ID 地址的类型安全访问方法

- 添加 set_child_tid_ua() 获取 set_child_tid 地址
- 添加 set_set_child_tid_ua() 设置 set_child_tid 地址
- 添加 clear_child_tid_ua() 获取 clear_child_tid 地址
- 添加 set_clear_child_tid_ua() 设置 clear_child_tid 地址
- 所有方法使用 UA 类型表示用户地址
- 保持原有字段不变，确保向后兼容

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**修改内容**:

#### 1. 添加 `set_child_tid_ua()` 方法
- 文件：`os/src/kernel/task/task_struct.rs`
- 获取 set_child_tid 地址的类型安全版本
- 返回：`UA` - 用户地址类型

#### 2. 添加 `set_set_child_tid_ua()` 方法
- 文件：`os/src/kernel/task/task_struct.rs`
- 设置 set_child_tid 地址的类型安全版本
- 参数：`addr: UA` - 用户地址类型

#### 3. 添加 `clear_child_tid_ua()` 方法
- 文件：`os/src/kernel/task/task_struct.rs`
- 获取 clear_child_tid 地址的类型安全版本
- 返回：`UA` - 用户地址类型

#### 4. 添加 `set_clear_child_tid_ua()` 方法
- 文件：`os/src/kernel/task/task_struct.rs`
- 设置 clear_child_tid 地址的类型安全版本
- 参数：`addr: UA` - 用户地址类型

**代码示例**:
```rust
/// 获取 set_child_tid 地址（类型安全版本）
pub fn set_child_tid_ua(&self) -> crate::mm::address::UA {
    crate::mm::address::UA::from_usize(self.set_child_tid)
}

/// 设置 set_child_tid 地址（类型安全版本）
pub fn set_set_child_tid_ua(&mut self, addr: crate::mm::address::UA) {
    self.set_child_tid = addr.as_usize();
}

/// 获取 clear_child_tid 地址（类型安全版本）
pub fn clear_child_tid_ua(&self) -> crate::mm::address::UA {
    crate::mm::address::UA::from_usize(self.clear_child_tid)
}

/// 设置 clear_child_tid 地址（类型安全版本）
pub fn set_clear_child_tid_ua(&mut self, addr: crate::mm::address::UA) {
    self.clear_child_tid = addr.as_usize();
}
```

**验证结果**:
- ✅ 编译成功：`cargo build --target riscv64gc-unknown-none-elf`
- ✅ 无编译错误
- ✅ 向后兼容：旧代码继续工作

**代码统计**:
- 修改文件：1
  - `os/src/kernel/task/task_struct.rs`: +20 行
- 新增方法：4 个（getter 和 setter 各 2 个）

**影响分析**:
- ✅ 向后兼容：所有现有代码继续工作
- ✅ 零性能开销：新方法仅是类型转换包装
- ✅ 类型安全增强：线程 ID 地址现在有明确的类型标记
- ✅ 线程同步支持：为 clone/futex 等系统调用提供类型安全接口

**技术说明**:
- `set_child_tid` 和 `clear_child_tid` 用于线程创建和退出时的同步
- 这些地址指向用户空间的 tid 变量
- 类型安全版本确保不会混淆用户地址和其他 usize 值

**Git 状态**:
- ✅ 已提交到 branch `refactor/momix`
- ✅ 所有修改已保存

**下一步**: 继续 Step 1.2 - 迁移其他模块
- [x] os/src/arch/riscv64/mm/ - 架构相关内存操作 ✅
- [x] os/src/kernel/task/exec_loader.rs - ELF 加载结果 ✅
- [x] os/src/kernel/task/task_struct.rs - 线程 ID 地址 ✅
- [ ] os/src/mm/page_table/ - 页表操作
- [ ] os/src/kernel/task/ - 其他进程内存布局相关代码

---
