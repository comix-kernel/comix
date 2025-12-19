    .section .text.entry
    .globl _start

# LoongArch64 内核入口点
_start:
    # 配置 DMW0: VSEG = 0x8, PLV0, Strongly ordered uncached (用于 MMIO/UART)
    # DMW0 使虚拟地址 0x8000_xxxx_xxxx_xxxx 映射到物理地址 0x0000_xxxx_xxxx_xxxx
    li.d        $t0, (0x8000000000000000 | 1)
    csrwr       $t0, 0x180      # CSR_DMW0

    # 配置 DMW1: VSEG = 0x9, PLV0, Coherent cached (用于内存访问)
    # DMW1 使虚拟地址 0x9000_xxxx_xxxx_xxxx 映射到物理地址 0x0000_xxxx_xxxx_xxxx
    li.d        $t0, (0x9000000000000000 | 1 | (1 << 4))
    csrwr       $t0, 0x181      # CSR_DMW1

    # 保存设备树地址 (QEMU 通过 a0 传递)
    la.global   $t0, DTP
    st.d        $a0, $t0, 0

    # 设置栈指针
    la.global   $sp, boot_stack_top

    # 跳转到 Rust 入口
    bl          rust_main

    # 不应到达这里
1:  b           1b

    # 注意: DTP 由 device/device_tree.rs 定义，此处仅引用

    .section .bss.stack
    .align 12
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:
