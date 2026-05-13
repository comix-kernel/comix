# Moss Kernel HAL 设计指南

本文档详尽介绍 moss-kernel 的硬件抽象层 (HAL) 设计与实现，适合将其设计模式移植到自己的内核项目中。

---

## 1. 整体架构概览

moss-kernel 的 HAL 分为两个 crate:

```
libkernel/       ← 架构无关的核心抽象层  (可在 x86_64 宿主编译测试)
src/arch/        ← 架构相关实现层       (仅 aarch64)
```

```
┌──────────────────────────────────────────────────────────┐
│                   src/ (内核主体)                          │
│  use crate::arch::ArchImpl;  // 编译时选择具体架构           │
│  ArchImpl::some_method();    // 调用 Arch trait 方法         │
├──────────────────────────────────────────────────────────┤
│              src/arch/mod.rs                               │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  trait Arch: CpuOps + VirtualMemory { ... }          │ │
│  │  #[cfg(aarch64)] pub use arm64::Aarch64 as ArchImpl; │ │
│  └──────────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────┤
│            src/arch/arm64/mod.rs                           │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  struct Aarch64;                                     │ │
│  │  impl CpuOps for Aarch64 { ... }                     │ │
│  │  impl VirtualMemory for Aarch64 { ... }              │ │
│  │  impl Arch for Aarch64 { ... }                       │ │
│  └──────────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────┤
│           libkernel (架构无关库)                             │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  trait CpuOps { id(), halt(), disable_interrupts() } │ │
│  │  trait VirtualMemory: CpuOps { ... }                 │ │
│  │  trait UserAddressSpace { map_page(), activate()  }  │ │
│  │  struct SpinLockIrq<T, CPU: CpuOps> { ... }         │ │
│  │  struct Mutex<T, CPU: CpuOps> { ... }               │ │
│  │  type VA = Address<Virtual, ()>;                     │ │
│  └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

---

## 2. 核心抽象: CpuOps — HAL 的第一层

**文件:** `libkernel/src/lib.rs:96`

这是 HAL 最底层的 trait, 所有同层和上层抽象都依赖它。

```rust
pub trait CpuOps: 'static {
    fn id() -> usize;                        // 当前 CPU 核心 ID
    fn halt() -> !;                          // 停止 CPU, 永不返回
    fn disable_interrupts() -> usize;        // 关中断, 返回旧状态
    fn restore_interrupt_state(flags: usize); // 恢复中断状态
    fn enable_interrupts();                  // 显式开中断
}
```

**设计意图:** 将架构相关操作缩小到最少 5 个方法, 使得 sync/memory 等模块完全可移植。

**关键用法 — 泛型约束:** 所有同步原语都是 `CpuOps` 的泛型, 例如:

```rust
// libkernel/src/sync/spinlock.rs
pub struct SpinLockIrq<T: ?Sized, CPU: CpuOps> { ... }
impl<T, CPU: CpuOps> SpinLockIrq<T, CPU> {
    pub fn lock_save_irq(&self) -> SpinLockIrqGuard<'_, T, CPU> {
        let saved_irq_flags = CPU::disable_interrupts();  // <-- 通过 CpuOps 关中断
        // ... 自旋等待 ...
        SpinLockIrqGuard { lock: self, irq_flags: saved_irq_flags, ... }
    }
}
impl Drop for SpinLockIrqGuard<..., CPU: CpuOps> {
    fn drop(&mut self) {
        // unlock ...
        CPU::restore_interrupt_state(self.irq_flags);  // <-- 通过 CpuOps 恢复
    }
}
```

**同理** `Mutex<T, CPU>`, `RwLock<T, CPU>`, `PerCpu<T, CPU>` 等原语都采用相同的泛型模式。

**可测试性:** 因为 `CpuOps` 是 trait, 可以在 x86_64 宿主编译时提供 Mock 实现:
```rust
// libkernel/src/lib.rs:116 (仅 test 编译时)
impl CpuOps for MockCpuOps {
    fn id() -> usize { 0 }
    fn halt() -> ! { loop { core::hint::spin_loop() } }
    fn disable_interrupts() -> usize { 0 }
    fn restore_interrupt_state(_flags: usize) {}
    fn enable_interrupts() {}
}
```
配合 feature gates, `libkernel` 的 230+ 测试全部在宿主 x86_64 上运行。

> **移植要点:** CpuOps 是你要做的第一个 trait 实现。它只有 5 个方法, 实现完它就能编译同步原语和内存分配器等核心模块。

---

## 3. 类型安全的地址抽象

**文件:** `libkernel/src/memory/address.rs`

### 3.1 地址类型体系

```rust
// Sealed trait — 外部无法实现 (安全边界)
pub trait MemKind: sealed::Sealed + Ord + Clone + Copy + PartialEq + Eq {}
pub struct Virtual;   // 虚拟地址
pub struct Physical;  // 物理地址
pub struct User;      // 用户空间地址

// 泛型地址类型 — 带 Kind 和数据类型标记
pub struct Address<K: MemKind, T> {
    inner: usize,
    _phantom: PhantomData<K>,
    _phantom_type: PhantomData<T>,
}

// 类型别名
pub type PA = Address<Physical, ()>;   // 无类型标记的物理地址
pub type VA = Address<Virtual, ()>;    // 无类型标记的虚拟地址
pub type UA = Address<User, ()>;       // 无类型标记的用户地址
pub type TPA<T> = Address<Physical, T>; // 带类型标记的物理地址指针
pub type TVA<T> = Address<Virtual, T>;  // 带类型标记的虚拟地址指针
```

### 3.2 设计优势

**编译期防止地址空间混用:**
```rust
fn map_page(va: VA, pa: PA, perms: PtePermissions)  // 不会传错参数
```

**地址转换需要显式 Translator:**
```rust
pub trait AddressTranslator<T>: 'static + Send + Sync {
    fn virt_to_phys(va: TVA<T>) -> TPA<T>;
    fn phys_to_virt(pa: TPA<T>) -> TVA<T>;
}
// 只有通过 Translator 才能跨地址空间转换
let pa: PA = va.to_pa::<IdentityTranslator>();
```

**分页计算上下文无关:**
```rust
impl<K: MemKind, T> Address<K, T> {
    pub fn is_page_aligned(self) -> bool { ... }
    pub fn add_pages(self, count: usize) -> Self { ... }
    pub fn align_up(self, align: usize) -> Self { ... }
    pub fn page_aligned(self) -> Self { ... }
}
```

**安全访问控制:**
- `PA::as_ptr()` / `PA::as_ptr_mut()` 是 `unsafe` — 裸物理地址访问需要显式承诺
- `VA::as_ptr()` / `VA::as_ptr_mut()` 不是 unsafe — 虚拟地址已通过 MMU 映射
- `UA` 不能直接转指针 — 必须通过 `copy_from_user` 等安全机制

> **移植要点:** 地址类型体系在 `libkernel` 中定义, 与任何特定 CPU 解耦, 可以直接复用。

---

## 4. VirtualMemory trait — 内存子系统抽象

**文件:** `libkernel/src/memory/address_space.rs:200`

```rust
pub trait VirtualMemory: CpuOps + Sized {
    type PageTableRoot;                              // 顶层页表类型
    type ProcessAddressSpace: UserAddressSpace;       // 进程地址空间
    type KernelAddressSpace: KernAddressSpace;        // 内核地址空间
    const PAGE_OFFSET: usize;                        // 物理→虚拟映射偏移
    fn kern_address_space() -> &'static SpinLockIrq<Self::KernelAddressSpace, Self>;
}
```

### 4.1 UserAddressSpace trait — 进程地址空间

**文件:** `libkernel/src/memory/address_space.rs:32`

```rust
pub trait UserAddressSpace: Send + Sync {
    fn new() -> Result<Self>;                          // 新建空页表
    fn activate(&self);                                // 激活(写入 TTBR0/CR3)
    fn deactivate(&self);                              // 反激活
    fn map_page(&mut self, page: PageFrame, va: VA, perms: PtePermissions) -> Result<()>;
    fn unmap(&mut self, va: VA) -> Result<PageFrame>;
    fn remap(&mut self, va: VA, new_page: PageFrame, perms: PtePermissions) -> Result<PageFrame>;
    fn protect_range(&mut self, va_range: VirtMemoryRegion, perms: PtePermissions) -> Result<()>;
    fn unmap_range(&mut self, va_range: VirtMemoryRegion) -> Result<Vec<PageFrame>>;
    fn translate(&self, va: VA) -> Option<PageInfo>;
    fn protect_and_clone_region(&mut self, region: VirtMemoryRegion, other: &mut Self, perms: PtePermissions) -> Result<()>;
}
```

这个 trait 完全解耦了进程地址空间的操作。架构相关实现只需填充方法。

### 4.2 KernAddressSpace trait — 内核地址空间

```rust
pub trait KernAddressSpace: Send {
    fn map_mmio(&mut self, region: PhysMemoryRegion) -> Result<VA>;
    fn map_normal(&mut self, phys_range: PhysMemoryRegion, virt_range: VirtMemoryRegion, perms: PtePermissions) -> Result<()>;
}
```

---

## 5. Arch trait — 顶层架构抽象

**文件:** `src/arch/mod.rs:28`

```rust
pub trait Arch: CpuOps + VirtualMemory {
    type UserContext: Sized + Send + Sync + Clone;
    type PTraceGpRegs: UserCopyable + for<'a> From<&'a Self::UserContext>;

    // 进程/上下文
    fn new_user_context(entry_point: VA, stack_top: VA) -> Self::UserContext;
    fn context_switch(new: Arc<Task>);
    fn create_idle_task() -> OwnedTask;

    // 信号处理
    fn do_signal(ctx: ProcessCtx, sig: SigId, action: UserspaceSigAction) -> impl Future<Output = Result<Self::UserContext>>;
    fn do_signal_return(ctx: ProcessCtx) -> impl Future<Output = Result<Self::UserContext>>;

    // 用户/内核内存复制 (安全边界)
    unsafe fn copy_from_user(src: UA, dst: *mut (), len: usize) -> impl Future<Output = Result<()>>;
    unsafe fn try_copy_from_user(src: UA, dst: *mut (), len: usize) -> Result<()>;
    unsafe fn copy_to_user(src: *const (), dst: UA, len: usize) -> impl Future<Output = Result<()>>;
    unsafe fn copy_strn_from_user(src: UA, dst: *mut u8, len: usize) -> impl Future<Output = Result<usize>>;

    // 系统信息
    fn name() -> &'static str;
    fn cpu_count() -> usize;
    fn get_cmdline() -> Option<String>;

    // 电源管理
    fn power_off() -> !;
    fn restart() -> !;
}
```

**await 感知设计:** `copy_from_user` 等返回 `impl Future`, 允许在页缺失处理过程中异步等待磁盘 I/O。

---

## 6. 架构选择机制

**文件:** `src/arch/mod.rs:204-208`

```rust
#[cfg(target_arch = "aarch64")]
mod arm64;

#[cfg(target_arch = "aarch64")]
pub use self::arm64::Aarch64 as ArchImpl;
```

内核其余部分只使用 `ArchImpl`:

```rust
// src/main.rs
use crate::arch::{Arch, ArchImpl};

ArchImpl::disable_interrupts();
ArchImpl::kern_address_space().lock_save_irq();
ArchImpl::power_off();
```

**如何添加新架构 (如 riscv64):**
1. 创建 `src/arch/riscv64/` 目录
2. 实现 `CpuOps`, `VirtualMemory`, `Arch` 三个 trait
3. 在 `src/arch/mod.rs` 中添加:
```rust
#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use self::riscv64::Riscv64 as ArchImpl;
```

---

## 7. Feature Gates — 模块化编译

**文件:** `libkernel/Cargo.toml`

```
                sync
                  ↓
                alloc  →  paging  →  proc_vm
                         proc
                         fs (依赖 proc + sync)
                         kbuf (依赖 sync)
```

| Feature  | 启用的模块                                | 依赖          |
|----------|------------------------------------------|--------------|
| `sync`   | spinlock, mutex, rwlock, per_cpu, mpsc  | —            |
| `alloc`  | buddy 分配器, slab 分配器                | `sync`       |
| `paging` | 页表操作, 地址空间管理, PTE 辅助类型      | `alloc`      |
| `proc`   | UID/GID, capabilities                   | —            |
| `fs`     | VFS traits, 块设备, ext4+               | `proc`,`sync`|
| `proc_vm`| mmap, brk, CoW                          | `paging`,`fs`|
| `kbuf`   | 环形内核缓冲区                           | `sync`       |

> **移植要点:** 可以先只启用 `sync`, 逐步向上启用, 以此验证每一层。

---

## 8. Aarch64 实现文件对照

**入口:** `src/arch/arm64/mod.rs` — 定义 `Aarch64` 结构体, 实现 `CpuOps` + `VirtualMemory` + `Arch` 三个 trait。

### 8.1 trait 实现位置

| Trait | 实现类型 | 文件 |
|-------|---------|------|
| `CpuOps` | `Aarch64` | `src/arch/arm64/mod.rs:46` |
| `VirtualMemory` | `Aarch64` | `src/arch/arm64/mod.rs:70` |
| `Arch` | `Aarch64` | `src/arch/arm64/mod.rs:82` |
| `UserAddressSpace` | `Arm64ProcessAddressSpace` | `src/arch/arm64/memory/address_space.rs:40` |
| `KernAddressSpace` | `Arm64KernelAddressSpace` | `src/arch/arm64/memory/mmu.rs:61` |
| `PgTable` | `L0Table`, `L1Table`, `L2Table`, `L3Table` | `libkernel/src/arch/arm64/memory/pg_tables.rs` |
| `PageTableEntry` | `L0Descriptor`, ..., `L3Descriptor` | `libkernel/src/arch/arm64/memory/pg_descriptors.rs` |

### 8.2 源码组织

```
src/arch/arm64/
├── mod.rs                ← Arch / CpuOps / VirtualMemory 实现
├── cpu_ops.rs            ← local_irq_save / local_irq_restore (内联汇编)
├── boot/
│   ├── mod.rs
│   ├── exception_level.rs  ← EL 切换
│   ├── logical_map.rs      ← 启动时逻辑映射
│   ├── memory.rs           ← 启动时物理内存检测
│   ├── paging_bootstrap.rs ← 早期页表建立
│   └── secondary.rs        ← 次核启动
├── memory/
│   ├── mod.rs
│   ├── address_space.rs    ← Arm64ProcessAddressSpace (UserAddressSpace impl)
│   ├── fault.rs            ← 缺页异常处理
│   ├── fixmap.rs           ← fixmap 临时映射
│   ├── heap.rs             ← 内核堆初始化
│   ├── mmu.rs              ← Arm64KernelAddressSpace (KernAddressSpace impl)
│   ├── mmu/
│   │   ├── page_allocator.rs
│   │   ├── page_mapper.rs
│   │   └── smalloc_page_allocator.rs
│   ├── tlb.rs              ← TLB 刷新
│   └── uaccess.rs          ← copy_from_user / copy_to_user 实现
├── exceptions/
│   ├── mod.rs              ← 异常向量表
│   ├── esr.rs              ← 异常综合寄存器解析
│   └── syscall.rs          ← 系统调用分发
├── proc/
│   ├── mod.rs              ← context_switch 实现
│   ├── idle.rs             ← 创建 idle 任务
│   ├── signal.rs           ← do_signal / do_signal_return
│   └── vdso.rs             ← vDSO
├── fdt.rs                  ← 设备树解析 (启动参数)
├── psci.rs                 ← PSCI 电源管理接口
└── ptrace.rs               ← ptrace 寄存器类型
```

---

## 9. 阅读路线建议

### 第一圈: 理解核心抽象 (约 30 分钟)

按以下顺序阅读:

1. **`libkernel/src/lib.rs`** — `CpuOps` trait 定义 + MockCpuOps 实现
2. **`libkernel/src/memory/address.rs`** — `VA`, `PA`, `UA` 类型系统
3. **`libkernel/src/sync/spinlock.rs`** — 理解 `CPU: CpuOps` 泛型模式
4. **`src/arch/mod.rs`** — `Arch` trait 完整定义
5. **`src/arch/arm64/mod.rs`** — 三个 trait 的 arm64 实现 (约 180 行)

### 第二圈: 内存子系统 (约 45 分钟)

6. **`libkernel/src/memory/address_space.rs`** — `UserAddressSpace`, `KernAddressSpace`, `VirtualMemory` 三个 trait
7. **`libkernel/src/memory/region.rs`** — `MemoryRegion<T>` 通用内存区域
8. **`libkernel/src/arch/arm64/memory/pg_tables.rs`** — arm64 页表层级的 libkernel 侧定义 (`PgTable` trait, PTE 结构)
9. **`src/arch/arm64/memory/mmu.rs`** — `Arm64KernelAddressSpace` 实现
10. **`src/arch/arm64/memory/address_space.rs`** — `Arm64ProcessAddressSpace` 实现

### 第三圈: 进程与异常 (约 30 分钟)

11. **`src/arch/arm64/cpu_ops.rs`** — 内联汇编中断开关
12. **`src/arch/arm64/proc/mod.rs`** — `context_switch` 实现
13. **`src/arch/arm64/exceptions/mod.rs`** — 异常向量表
14. **`src/arch/arm64/exceptions/syscall.rs`** — 系统调用分发

### 第四圈: 了解完整用法 (约 20 分钟)

15. **`src/main.rs`** — 内核初始化如何调用 ArchImpl
16. **`libkernel/Cargo.toml`** — feature gates 的层次结构
17. 随机抽查 `src/drivers/` 中的几个文件, 看 `ArchImpl` 如何被驱动使用

---

## 10. 移植检查清单

将这个 HAL 模式移植到你的内核时, 建议按以下顺序实现:

- [ ] **Phase 1: CpuOps** — 实现 CPU ID、中断控制、halt (编译 0 依赖)
- [ ] **Phase 2: 地址类型** — 直接复用 `PA`, `VA`, `UA` 类型系统 (架构无关)
- [ ] **Phase 3: CpuOps 完成** — sync 原语全部可用 (SpinLockIrq, Mutex)
- [ ] **Phase 4: 页表抽象** — 实现 `PgTable` trait 和各级页表描述符
- [ ] **Phase 5: VirtualMemory** — 实现 `UserAddressSpace` 和 `KernAddressSpace`
- [ ] **Phase 6: Arch trait** — 实现上下文切换、用户/内核内存复制
- [ ] **Phase 7: 测试** — 编写 mock 实现, 在宿主编译运行单元测试

---

## 11. 关键设计模式总结

| 模式 | 说明 |
|------|------|
| **最小编译依赖** | `CpuOps` 只有 5 个方法, 不延展架构细节 |
| **泛型注入** | `SpinLockIrq<T, CPU>` 通过泛型参数注入 CPU 操作, 调用方在类型系统中可见依赖 |
| **关联类型** | `Arch` 和 `VirtualMemory` 用关联类型约束用户上下文、页表根等架构特定类型 |
| **Feature Gates** | 按模块粒度 feature gating, 消费者按需启用 |
| **Type Alias 统一入口** | `ArchImpl` 编译时展开为具体架构类型, 其余代码无感知 |
| **trait 继承** | `Arch: CpuOps + VirtualMemory` 建立清晰的层级 |
| **Mock + 宿主编译测试** | `#[cfg(test)]` 提供 mock impl, 架构无关代码全部在宿主测试 |
| **async/await 贯穿** | 所有可能阻塞的操作都用 `async fn`/`impl Future`, 编译器保证 spinlock 不跨 await 持有 |
