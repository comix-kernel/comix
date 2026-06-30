# 地址抽象层

地址抽象层把裸 `usize` 拆成带语义的类型, 让 MM 代码在接口边界上区分物理地址, 内核虚拟地址, 用户地址和页号.

## 当前状态

- `PA`, `VA`, `UA` 来自架构地址模块, 在 `os/src/mm/address/types.rs` 中实现统一 trait.
- `Ppn` 和 `Vpn` 是 MM 层定义的页号类型.
- `AddressRange<T>` 和 `PageNumRange<T>` 统一使用 `[start, end)`.
- `ConvertablePA` 和 `ConvertableVA` 只表达直接映射地址转换能力, 不替代页表翻译.

## 目标

- 防止把物理地址和虚拟地址作为同一种整数随意传递.
- 统一页对齐, 页号转换和范围遍历.
- 把架构地址格式保留在 `arch` 层, 让 MM 上层只依赖少量 trait.

## 非目标

- 不负责检查用户指针合法性.用户地址最终仍要经过当前进程页表或专门的用户拷贝路径验证.
- 不负责页表权限判断.`VA::to_pa()` 只适用于架构直接映射窗口, 普通用户地址必须走 `MemorySpace::translate()`.
- 不承诺所有算术都做溢出保护.调用方仍需在外部边界做长度和地址空间检查.

## 模块边界

- `types.rs` 定义 `Address`, `AddressRange`, `PA/VA/UA` 的统一行为以及直接映射转换 trait.
- `page_num.rs` 定义 `PageNum`, `Ppn`, `Vpn`, `PpnRange`, `VpnRange`.
- `operations.rs` 定义 `UsizeConvert`, `CalcOps`, `AlignOps` 以及地址/页号算术宏.
- `mod.rs` 只负责重导出, 让其他 MM 模块通过 `crate::mm::address::*` 使用这些类型.

## 关键流程

### 地址到页号

页号转换分为 floor 和 ceil:

- floor 用于查找地址所在页, 例如页表翻译非页对齐地址.
- ceil 用于把字节区间尾部扩展到完整页, 例如映射 `[start, start + len)`.

这两个方向不能混用.`translate()` 一类路径必须用 floor, 否则页内地址会错误落到下一页.

### 物理地址到内核虚拟地址

`ConvertablePA::to_va()` 调用架构直接映射函数.它适用于已经确认位于内核直接映射窗口内的物理页, 例如清零物理帧或读写页表页.

### 虚拟地址到物理地址

`ConvertableVA::to_pa()` 只面向架构直接映射地址.用户地址和一般 VMA 地址必须通过当前 `MemorySpace` 的页表翻译.

## 生命周期与安全约束

- `Ppn` 本身不拥有物理帧.帧所有权由 `FrameTracker`, `FrameRangeTracker` 或 `MappingArea.frames` 管理.
- `VA` 本身不说明地址可访问.它只是一种数值语义, 访问前仍需页表映射和权限保证.
- Range 迭代只表达页号/地址序列, 不表达内存已经映射.

## 已知限制

- 地址算术主要服务内核内部路径, 对恶意输入的完整溢出防护在系统调用层或调用方完成.
- `AddressRange::from_slices()` 这类辅助构造不应作为 VMA 合法性判断依据.

## 源码索引

- `os/src/mm/address/mod.rs:1` - 模块说明和重导出.
- `os/src/mm/address/types.rs:12` - `Address` trait 与 `AddressRange`.
- `os/src/mm/address/types.rs:103` - `ConvertablePA` 和 `ConvertableVA`.
- `os/src/mm/address/page_num.rs:13` - `PageNum`, `Ppn`, `Vpn` 和页号范围.
- `os/src/mm/address/operations.rs:12` - `UsizeConvert`, 算术和对齐 trait.
