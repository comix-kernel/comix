    .section .text.entry
    .globl _start
_start:
    # 1. 设置早期页表（恒等映射 + 高地址映射）
    la t0, boot_pagetable
    srli t0, t0, 12          # 转换为 PPN
    li t1, 8 << 60           # SV39 模式
    or t0, t0, t1
    csrw satp, t0            # 启用分页
    sfence.vma               # 刷新 TLB

    # 2. 跳转到高地址
    la t0, _start_high
    jr t0

_start_high:
    # 3. 现在 PC 在高地址，可以安全使用虚拟地址
    la sp, boot_stack_top
    call rust_main

    .section .data
    .align 12
boot_pagetable:
    # 早期页表：只映射内核段（恒等 + 高地址）
    # PTE[2]: 0x00000000_80000000 -> 0x80000000 (恒等映射，用于启动)
    .8byte (0x80000 << 10) | 0xef  # PPN=0x80000, V|R|W|X|A|D|G

    # PTE[?]: 0xffffffffc0000000 -> 0x80000000 (高地址映射，内核使用)
    # 计算 VPN[2] = 0xffffffffc0000000 >> 30 & 0x1ff = 0x100
    .zero 8 * (0x100 - 1)          # 填充到索引 0x100
    .8byte (0x80000 << 10) | 0xef  # 同样映射到物理地址 0x80000000

    .zero 8 * (512 - 0x100 - 2)    # 填充剩余项

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top: