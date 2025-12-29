    .section .text.entry
    .globl _start
_start:
    # 将a1的值存储到DTP位置
    # a1: 指向设备树的物理地址,由引导加载程序传递
    la t0, DTP
    sd a1, 0(t0)

    # ========== 主核路径（hartid == 0）==========
.primary_hart:
    # 1. 设置早期页表（恒等映射 + 高地址映射）
    la t0, boot_pagetable
    srli t0, t0, 12          # 转换为 PPN
    li t1, 8 << 60           # SV39 模式
    or t0, t0, t1
    csrw satp, t0            # 启用分页
    sfence.vma               # 刷新 TLB

    # 2. 跳转到高地址
    # 参考其他OS的做法: la加载物理地址,然后or上虚拟地址偏移
    # VADDR_START = 0xffff_ffc0_0000_0000 (arch/riscv/mm/mod.rs)
    # 或者 VIRTUAL_BASE = 0xffffffffc0200000 (linker.ld, 符号扩展形式)
    # 实际上两者的高位部分相同,都是将bit38-63设置为1
    la t0, _start_high       # t0 = _start_high的物理地址
    li t1, -1                # t1 = 0xffffffff_ffffffff
    slli t1, t1, 38          # t1 = 0xffffffc0_00000000 (清除低38位,设置高位)
    or t0, t0, t1            # 将物理地址转换为虚拟地址
    jr t0

_start_high:
    # 3. 现在 PC 在高地址，可以安全使用虚拟地址
    # 加载栈指针,需要确保是虚拟地址
    la sp, boot_stack_top
    li t0, -1
    slli t0, t0, 38          # t0 = 0xffffffc0_00000000
    or sp, sp, t0            # 将栈地址转换为虚拟地址

    # 4. 跳转到 Rust 主入口
    call rust_main

    # 不应该返回
.loop_forever:
    wfi
    j .loop_forever

    # ========== 数据段 ==========
    .section .data
    .align 12
boot_pagetable:
    # 早期页表：只映射内核段（恒等 + 高地址）
    # PTE[0-1]: 未使用
    .8byte 0
    .8byte 0
    # PTE[2]: 0x80000000 -> 0x80000000 (恒等映射，用于启动)
    .8byte (0x80000 << 10) | 0xef  # PPN=0x80000, V|R|W|X|A|D|G

    # PTE[3-255]: 填充
    .zero 8 * (0x100 - 3)          # 填充从索引3到索引255

    # PTE[256-259]: 高地址映射，映射4个1GB页面以覆盖整个内核空间
    # PTE[256]: 0xffff_ffc0_0000_0000 -> 0x00000000
    .8byte (0x00000 << 10) | 0xcf
    # PTE[257]: 0xffff_ffc0_4000_0000 -> 0x40000000
    .8byte (0x40000 << 10) | 0xcf
    # PTE[258]: 0xffff_ffc0_8000_0000 -> 0x80000000 (内核所在)
    .8byte (0x80000 << 10) | 0xcf
    # PTE[259]: 0xffff_ffc0_c000_0000 -> 0xc0000000
    .8byte (0xc0000 << 10) | 0xcf

    # PTE[260-511]: 填充剩余项
    .zero 8 * (512 - 0x100 - 4)    # 填充从索引260到511

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 48
    .globl boot_stack_top
boot_stack_top:

    # ========== SBI HSM 从核入口 ==========
    # 当使用 SBI HSM 启动从核时，OpenSBI 会将从核跳转到这里（物理地址）
    # 此时 MMU 关闭，a0 = hartid, a1 = opaque
    .section .text.entry
    .globl secondary_sbi_entry
secondary_sbi_entry:
    mv s0, a0               # 保存 hartid 到 s0

    # 1. 启用分页（使用主核已设置的页表）
    la t0, boot_pagetable
    li t1, 0x3FFFFFFFFF
    and t0, t0, t1          # 转换为物理地址
    srli t0, t0, 12
    li t1, 8 << 60
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    # 2. 跳转到高地址
    la t0, .secondary_sbi_high
    li t1, -1
    slli t1, t1, 38
    or t0, t0, t1
    jr t0

.secondary_sbi_high:
    # 3. 设置栈（虚拟地址）
    la sp, secondary_stacks_top
    li t1, -1
    slli t1, t1, 38
    or sp, sp, t1            # 转换为虚拟地址
    slli t1, s0, 16          # hartid × 64KB
    sub sp, sp, t1

    # 4. 跳转到 Rust 入口
    mv a0, s0
    call secondary_start

    # 不应该返回
.secondary_sbi_loop:
    wfi
    j .secondary_sbi_loop

    # ========== 从核栈空间 ==========
    .section .bss.secondary_stack
    .align 12
secondary_stacks_bottom:
    .space 65536 * 8        # 8 核 × 64KB
    .globl secondary_stacks_top
secondary_stacks_top: