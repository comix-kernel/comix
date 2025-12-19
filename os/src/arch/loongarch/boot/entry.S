    .section .text.entry
    .globl _start

# LoongArch64 内核入口点
# 此时 MMU 关闭，使用物理地址
_start:
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
