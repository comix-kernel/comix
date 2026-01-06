    .section .text.entry
    .globl _start

#
# LoongArch64 内核入口点
#
# QEMU virt 机器加载 ELF 后，CPU 从入口点开始执行。
# 假设 QEMU 固件已配置 DMW，或我们首先配置 DMW。
#
# DMW 映射：
# - DMW0: VSEG=0x8, VA 0x8xxx -> PA 0x0xxx (Uncached, MMIO)
# - DMW1: VSEG=0x9, VA 0x9xxx -> PA 0x0xxx (Cached, 内存)
#
_start:
    # ==========================================
    # 1. 配置 DMW (Direct Mapping Window)
    # ==========================================
    # 即使 QEMU 可能已配置，我们也显式设置以确保正确

    # DMW0: VSEG=0x8, PLV0, MAT=0 (SUC, Uncached)
    li.d        $t0, (0x8 << 60) | (0 << 4) | 0x1
    csrwr       $t0, 0x180          # CSR_DMW0

    # DMW1: VSEG=0x9, PLV0, MAT=1 (CC, Cached)
    li.d        $t0, (0x9 << 60) | (1 << 4) | 0x1
    csrwr       $t0, 0x181          # CSR_DMW1

    # ==========================================
    # 2. 保存设备树地址
    # LoongArch virt: a0=UEFI 标志(1), a2=EFI system table
    # FDT 固定放在 0x0010_0000
    # ==========================================
    la.global   $t1, DTP
    beqz        $a0, 1f
    li.d        $t0, 0x00100000
    st.d        $t0, $t1, 0
    b           2f
1:
    st.d        $a0, $t1, 0
2:

    # ==========================================
    # 3. 设置栈指针
    # ==========================================
    la.global   $sp, boot_stack_top

    # ==========================================
    # 4. 跳转到 Rust 入口
    # ==========================================
    bl          rust_main

    # 不应到达这里
1:  b           1b

    # ==========================================
    # 启动栈
    # ==========================================
    .section .bss.stack
    .align 12
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 48                # 48 页 = 192KB 启动栈
    .globl boot_stack_top
boot_stack_top:
