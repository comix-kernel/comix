# LoongArch64 trap entry and context restore
.text
.globl trap_entry
.globl tlb_refill_entry
.globl boot_trap_entry
.globl __restore
.globl trap_handler

trap_entry:
boot_trap_entry:
    # 保留原始 t2，使用 KScratch1 作为临时保存区
    csrwr   $t2, 0x49          # KScratch1 <- 原始 t2

    # 读取 KScratch0 中的 TrapFrame 指针，顺便将原始 a0 写回 KScratch0
    csrrd   $t1, 0x48          # t1 <- tf_ptr (保存于 KScratch0)
    csrwr   $a0, 0x48          # KScratch0 <- 原始 a0
    move    $a0, $t1           # a0 <- tf_ptr

    # 保存通用寄存器到 TrapFrame.regs[0..31]
    st.d    $r0,  $a0, 0       # r0 (zero)
    st.d    $ra,  $a0, 8       # r1
    st.d    $tp,  $a0, 16      # r2
    ld.d    $tp,  $a0, 296     # tp <- cpu_ptr (kernel per-CPU 指针)
    st.d    $sp,  $a0, 24      # r3
    st.d    $a1,  $a0, 40      # r5
    st.d    $a2,  $a0, 48      # r6
    st.d    $a3,  $a0, 56      # r7
    st.d    $a4,  $a0, 64      # r8
    st.d    $a5,  $a0, 72      # r9
    st.d    $a6,  $a0, 80      # r10
    st.d    $a7,  $a0, 88      # r11
    st.d    $t0,  $a0, 96      # r12
    st.d    $t1,  $a0, 104     # r13
    csrrd   $t2, 0x49          # 取回原始 t2
    st.d    $t2,  $a0, 112     # r14
    st.d    $t3,  $a0, 120     # r15
    st.d    $t4,  $a0, 128     # r16
    st.d    $t5,  $a0, 136     # r17
    st.d    $t6,  $a0, 144     # r18
    st.d    $t7,  $a0, 152     # r19
    st.d    $t8,  $a0, 160     # r20
    st.d    $r21, $a0, 168     # r21
    st.d    $fp,  $a0, 176     # r22 / s0
    st.d    $s1,  $a0, 184     # r23 / s1
    st.d    $s2,  $a0, 192     # r24 / s2
    st.d    $s3,  $a0, 200     # r25 / s3
    st.d    $s4,  $a0, 208     # r26 / s4
    st.d    $s5,  $a0, 216     # r27 / s5
    st.d    $s6,  $a0, 224     # r28 / s6
    st.d    $s7,  $a0, 232     # r29 / s7
    st.d    $s8,  $a0, 240     # r30 / s8
    st.d    $r31, $a0, 248     # r31

    # 保存原始 a0（现存于 KScratch0 中）
    csrrd   $t1, 0x48          # t1 <- old a0
    st.d    $t1, $a0, 32       # r4
    csrwr   $a0, 0x48          # 恢复 KScratch0 为 TrapFrame 指针

    # 保存异常相关 CSR
    csrrd   $t1, 0x6           # ERA
    st.d    $t1, $a0, 256
    csrrd   $t1, 0x5           # ESTAT
    st.d    $t1, $a0, 264
    csrrd   $t1, 0x0           # CRMD（当前模式/中断状态）
    st.d    $t1, $a0, 272
    csrrd   $t1, 0x1           # PRMD（异常前模式/中断状态）
    st.d    $t1, $a0, 280

    # 若来自用户态，则切换到保存的内核栈
    andi    $t2, $t1, 0x3      # PRMD.PLV 位
    beqz    $t2, 1f
    ld.d    $sp, $a0, 288      # kernel_sp
1:
    # 调用 Rust trap_handler(trap_frame)
    bl      trap_handler

# 注意：trap_handler 不应返回；若返回则直接跳转到恢复逻辑

__restore:
    # 使用 r21 作为基址寄存器，避免提前被覆盖
    move    $r21, $a0          # r21 <- TrapFrame 指针
    csrwr   $r21, 0x48         # 更新 KScratch0，供下一次陷阱使用

    # 恢复 CSR
    ld.d    $t0, $r21, 256     # ERA
    csrwr   $t0, 0x6
    ld.d    $t0, $r21, 280     # PRMD
    csrwr   $t0, 0x1
    ld.d    $t0, $r21, 272     # CRMD
    csrwr   $t0, 0x0

    # 恢复通用寄存器（保持 r21 基址到最后）
    ld.d    $ra,  $r21, 8
    ld.d    $tp,  $r21, 16
    ld.d    $sp,  $r21, 24
    ld.d    $a1,  $r21, 40
    ld.d    $a2,  $r21, 48
    ld.d    $a3,  $r21, 56
    ld.d    $a4,  $r21, 64
    ld.d    $a5,  $r21, 72
    ld.d    $a6,  $r21, 80
    ld.d    $a7,  $r21, 88
    ld.d    $t0,  $r21, 96
    ld.d    $t1,  $r21, 104
    ld.d    $t2,  $r21, 112
    ld.d    $t3,  $r21, 120
    ld.d    $t4,  $r21, 128
    ld.d    $t5,  $r21, 136
    ld.d    $t6,  $r21, 144
    ld.d    $t7,  $r21, 152
    ld.d    $t8,  $r21, 160
    ld.d    $fp,  $r21, 176
    ld.d    $s1,  $r21, 184
    ld.d    $s2,  $r21, 192
    ld.d    $s3,  $r21, 200
    ld.d    $s4,  $r21, 208
    ld.d    $s5,  $r21, 216
    ld.d    $s6,  $r21, 224
    ld.d    $s7,  $r21, 232
    ld.d    $s8,  $r21, 240
    ld.d    $r31, $r21, 248
    # 先恢复 a0（返回值），再恢复 r21 本身
    ld.d    $a0,  $r21, 32
    ld.d    $r21, $r21, 168

    # 结束陷阱，返回异常前上下文
    ertn

# TLB refill entry (software walk + tlbfill)
    .align 12
tlb_refill_entry:
    # Save temporary registers (t0-t2) without touching KScratch0 (trap frame ptr)
    csrwr   $t0, 0x49          # KScratch1 <- t0
    csrwr   $t1, 0x4a          # KScratch2 <- t1
    csrwr   $t2, 0x4b          # KScratch3 <- t2

    # t0 = badv from TLBRBADV
    csrrd   $t0, 0x89

    # Select PGD base: low half -> PGDL, high half -> PGDH
    csrrd   $t1, 0x19          # PGDL
    srai.d  $t2, $t0, 63
    beqz    $t2, 1f
    csrrd   $t1, 0x1a          # PGDH
1:
    # Level 2 index: (va >> 30) & 0x1ff
    srai.d  $t2, $t0, 30
    andi    $t2, $t2, 0x1ff
    slli.d  $t2, $t2, 3
    add.d   $t2, $t1, $t2
    ld.d    $t1, $t2, 0
    andi    $t2, $t1, 0x1
    beqz    $t2, tlb_refill_invalid
    li.d    $t2, -4096
    and     $t1, $t1, $t2     # next table base

    # Level 1 index: (va >> 21) & 0x1ff
    srai.d  $t2, $t0, 21
    andi    $t2, $t2, 0x1ff
    slli.d  $t2, $t2, 3
    add.d   $t2, $t1, $t2
    ld.d    $t1, $t2, 0
    andi    $t2, $t1, 0x1
    beqz    $t2, tlb_refill_invalid
    li.d    $t2, -4096
    and     $t1, $t1, $t2     # next table base

    # Level 0 index: (va >> 12) & 0x1ff
    srai.d  $t2, $t0, 12
    andi    $t2, $t2, 0x1ff
    slli.d  $t2, $t2, 3
    add.d   $t2, $t1, $t2
    ld.d    $t1, $t2, 0        # t1 = PTE
    andi    $t2, $t1, 0x1
    beqz    $t2, tlb_refill_invalid

tlb_refill_fill:
    # TLBREHI: set VPPN and PS=12 (4KB)
    srai.d  $t2, $t0, 13
    slli.d  $t2, $t2, 13
    ori     $t2, $t2, 12
    csrwr   $t2, 0x8e

    # Decide even/odd page by VA bit 12
    srai.d  $t2, $t0, 12
    andi    $t2, $t2, 0x1
    beqz    $t2, 2f
    # odd page -> TLBRELO1
    csrwr   $zero, 0x8c
    csrwr   $t1, 0x8d
    b       3f
2:
    # even page -> TLBRELO0
    csrwr   $t1, 0x8c
    csrwr   $zero, 0x8d
3:
    tlbfill

    # Restore temporaries
    csrrd   $t0, 0x49
    csrrd   $t1, 0x4a
    csrrd   $t2, 0x4b
    ertn

tlb_refill_invalid:
    # Fill invalid entry to trigger a normal page exception later
    li.d    $t1, 0
    b       tlb_refill_fill
