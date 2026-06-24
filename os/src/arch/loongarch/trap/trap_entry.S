# LoongArch64 trap entry and context restore
.text
.globl trap_entry
.globl tlb_refill_entry
.globl boot_trap_entry
.globl __restore
.globl trap_handler

.equ TF_FREGS, 304
.equ TF_FCSR, 560
.equ TF_FCC, 568

.macro save_fp_state base, tmp0, tmp1
    fst.d   $f0,  \base, TF_FREGS + 0 * 8
    fst.d   $f1,  \base, TF_FREGS + 1 * 8
    fst.d   $f2,  \base, TF_FREGS + 2 * 8
    fst.d   $f3,  \base, TF_FREGS + 3 * 8
    fst.d   $f4,  \base, TF_FREGS + 4 * 8
    fst.d   $f5,  \base, TF_FREGS + 5 * 8
    fst.d   $f6,  \base, TF_FREGS + 6 * 8
    fst.d   $f7,  \base, TF_FREGS + 7 * 8
    fst.d   $f8,  \base, TF_FREGS + 8 * 8
    fst.d   $f9,  \base, TF_FREGS + 9 * 8
    fst.d   $f10, \base, TF_FREGS + 10 * 8
    fst.d   $f11, \base, TF_FREGS + 11 * 8
    fst.d   $f12, \base, TF_FREGS + 12 * 8
    fst.d   $f13, \base, TF_FREGS + 13 * 8
    fst.d   $f14, \base, TF_FREGS + 14 * 8
    fst.d   $f15, \base, TF_FREGS + 15 * 8
    fst.d   $f16, \base, TF_FREGS + 16 * 8
    fst.d   $f17, \base, TF_FREGS + 17 * 8
    fst.d   $f18, \base, TF_FREGS + 18 * 8
    fst.d   $f19, \base, TF_FREGS + 19 * 8
    fst.d   $f20, \base, TF_FREGS + 20 * 8
    fst.d   $f21, \base, TF_FREGS + 21 * 8
    fst.d   $f22, \base, TF_FREGS + 22 * 8
    fst.d   $f23, \base, TF_FREGS + 23 * 8
    fst.d   $f24, \base, TF_FREGS + 24 * 8
    fst.d   $f25, \base, TF_FREGS + 25 * 8
    fst.d   $f26, \base, TF_FREGS + 26 * 8
    fst.d   $f27, \base, TF_FREGS + 27 * 8
    fst.d   $f28, \base, TF_FREGS + 28 * 8
    fst.d   $f29, \base, TF_FREGS + 29 * 8
    fst.d   $f30, \base, TF_FREGS + 30 * 8
    fst.d   $f31, \base, TF_FREGS + 31 * 8

    movfcsr2gr \tmp0, $fcsr0
    st.d    \tmp0, \base, TF_FCSR

    movcf2gr \tmp0, $fcc0
    move    \tmp1, \tmp0
    movcf2gr \tmp0, $fcc1
    bstrins.d \tmp1, \tmp0, 15, 8
    movcf2gr \tmp0, $fcc2
    bstrins.d \tmp1, \tmp0, 23, 16
    movcf2gr \tmp0, $fcc3
    bstrins.d \tmp1, \tmp0, 31, 24
    movcf2gr \tmp0, $fcc4
    bstrins.d \tmp1, \tmp0, 39, 32
    movcf2gr \tmp0, $fcc5
    bstrins.d \tmp1, \tmp0, 47, 40
    movcf2gr \tmp0, $fcc6
    bstrins.d \tmp1, \tmp0, 55, 48
    movcf2gr \tmp0, $fcc7
    bstrins.d \tmp1, \tmp0, 63, 56
    st.d    \tmp1, \base, TF_FCC
.endm

.macro restore_fp_state base, tmp0, tmp1
    fld.d   $f0,  \base, TF_FREGS + 0 * 8
    fld.d   $f1,  \base, TF_FREGS + 1 * 8
    fld.d   $f2,  \base, TF_FREGS + 2 * 8
    fld.d   $f3,  \base, TF_FREGS + 3 * 8
    fld.d   $f4,  \base, TF_FREGS + 4 * 8
    fld.d   $f5,  \base, TF_FREGS + 5 * 8
    fld.d   $f6,  \base, TF_FREGS + 6 * 8
    fld.d   $f7,  \base, TF_FREGS + 7 * 8
    fld.d   $f8,  \base, TF_FREGS + 8 * 8
    fld.d   $f9,  \base, TF_FREGS + 9 * 8
    fld.d   $f10, \base, TF_FREGS + 10 * 8
    fld.d   $f11, \base, TF_FREGS + 11 * 8
    fld.d   $f12, \base, TF_FREGS + 12 * 8
    fld.d   $f13, \base, TF_FREGS + 13 * 8
    fld.d   $f14, \base, TF_FREGS + 14 * 8
    fld.d   $f15, \base, TF_FREGS + 15 * 8
    fld.d   $f16, \base, TF_FREGS + 16 * 8
    fld.d   $f17, \base, TF_FREGS + 17 * 8
    fld.d   $f18, \base, TF_FREGS + 18 * 8
    fld.d   $f19, \base, TF_FREGS + 19 * 8
    fld.d   $f20, \base, TF_FREGS + 20 * 8
    fld.d   $f21, \base, TF_FREGS + 21 * 8
    fld.d   $f22, \base, TF_FREGS + 22 * 8
    fld.d   $f23, \base, TF_FREGS + 23 * 8
    fld.d   $f24, \base, TF_FREGS + 24 * 8
    fld.d   $f25, \base, TF_FREGS + 25 * 8
    fld.d   $f26, \base, TF_FREGS + 26 * 8
    fld.d   $f27, \base, TF_FREGS + 27 * 8
    fld.d   $f28, \base, TF_FREGS + 28 * 8
    fld.d   $f29, \base, TF_FREGS + 29 * 8
    fld.d   $f30, \base, TF_FREGS + 30 * 8
    fld.d   $f31, \base, TF_FREGS + 31 * 8

    ld.d    \tmp0, \base, TF_FCSR
    movgr2fcsr $fcsr0, \tmp0

    ld.d    \tmp0, \base, TF_FCC
    bstrpick.d \tmp1, \tmp0, 7, 0
    movgr2cf $fcc0, \tmp1
    bstrpick.d \tmp1, \tmp0, 15, 8
    movgr2cf $fcc1, \tmp1
    bstrpick.d \tmp1, \tmp0, 23, 16
    movgr2cf $fcc2, \tmp1
    bstrpick.d \tmp1, \tmp0, 31, 24
    movgr2cf $fcc3, \tmp1
    bstrpick.d \tmp1, \tmp0, 39, 32
    movgr2cf $fcc4, \tmp1
    bstrpick.d \tmp1, \tmp0, 47, 40
    movgr2cf $fcc5, \tmp1
    bstrpick.d \tmp1, \tmp0, 55, 48
    movgr2cf $fcc6, \tmp1
    bstrpick.d \tmp1, \tmp0, 63, 56
    movgr2cf $fcc7, \tmp1
.endm

trap_entry:
boot_trap_entry:
    # 保留原始寄存器，避免覆盖 KScratch0（其长期保存 TrapFrame 指针）
    csrwr   $a0, 0x31          # KScratch1 <- 原始 a0
    csrwr   $t1, 0x32          # KScratch2 <- 原始 t1

    # 读取 KScratch0 中的 TrapFrame 指针
    csrrd   $t1, 0x30          # t1 <- tf_ptr (保存于 KScratch0)
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
    # r13/t1 has been clobbered to hold tf_ptr; restore original t1 from KScratch2.
    csrrd   $t1, 0x32
    st.d    $t1,  $a0, 104     # r13 (t1)
    st.d    $t2,  $a0, 112     # r14 (t2)
    st.d    $t3,  $a0, 120     # r15
    st.d    $t4,  $a0, 128     # r16
    st.d    $t5,  $a0, 136     # r17
    st.d    $t6,  $a0, 144     # r18
    st.d    $t7,  $a0, 152     # r19
    st.d    $t8,  $a0, 160     # r20
    st.d    $r21, $a0, 168     # r21
    st.d    $fp,  $a0, 176     # r22 / fp
    st.d    $s0,  $a0, 184     # r23 / s0
    st.d    $s1,  $a0, 192     # r24 / s1
    st.d    $s2,  $a0, 200     # r25 / s2
    st.d    $s3,  $a0, 208     # r26 / s3
    st.d    $s4,  $a0, 216     # r27 / s4
    st.d    $s5,  $a0, 224     # r28 / s5
    st.d    $s6,  $a0, 232     # r29 / s6
    st.d    $s7,  $a0, 240     # r30 / s7
    st.d    $s8,  $a0, 248     # r31 / s8

    # 保存原始 a0（保存在 KScratch1 中）
    csrrd   $t1, 0x31          # t1 <- old a0
    st.d    $t1, $a0, 32       # r4 (a0)

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
    save_fp_state $a0, $t2, $t3
    ld.d    $sp, $a0, 288      # kernel_sp
1:
    # 调用 Rust trap_handler(trap_frame)
    bl      trap_handler

# 注意：trap_handler 不应返回；若返回则直接跳转到恢复逻辑

__restore:
    # 使用 r21 作为基址寄存器，避免提前被覆盖
    move    $r21, $a0          # r21 <- TrapFrame 指针
    csrwr   $r21, 0x30         # 更新 KScratch0，供下一次陷阱使用

    # 恢复 CSR
    ld.d    $t0, $r21, 256     # ERA
    csrwr   $t0, 0x6
    ld.d    $t0, $r21, 280     # PRMD
    andi    $t1, $t0, 0x3      # 目标 PRMD.PLV 位；csrwr 会把旧 PRMD 写回 $t0
    csrwr   $t0, 0x1

    beqz    $t1, 1f
    restore_fp_state $r21, $t0, $t1
1:

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
    ld.d    $s0,  $r21, 184
    ld.d    $s1,  $r21, 192
    ld.d    $s2,  $r21, 200
    ld.d    $s3,  $r21, 208
    ld.d    $s4,  $r21, 216
    ld.d    $s5,  $r21, 224
    ld.d    $s6,  $r21, 232
    ld.d    $s7,  $r21, 240
    ld.d    $s8,  $r21, 248
    # 先恢复 a0（返回值），再恢复 r21 本身
    ld.d    $a0,  $r21, 32
    ld.d    $r21, $r21, 168

    # 结束陷阱，返回异常前上下文
    ertn

# TLB refill entry (hardware-assisted walk with LDDIR/LDPTE).
# Use CSR.TLBRSAVE (0x8b) to preserve $t0 only, and avoid clobbering KSAVE/KSCRATCH
# CSRs that trap_entry relies on.
	    .align 12
	tlb_refill_entry:
	    csrwr   $t0, 0x8b          # TLBRSAVE <- t0
	    csrrd   $t0, 0x1b          # CSR.PGD (auto-selects PGDL/PGDH)
	    # PWCH enables Dir3 as the top-level directory; Dir4 is disabled.
	    # Walk Dir3 -> Dir2 -> Dir1 before loading the even/odd PTE pair.
	    lddir   $t0, $t0, 3
	    lddir   $t0, $t0, 2
	    lddir   $t0, $t0, 1
	    ldpte   $t0, 0
	    ldpte   $t0, 1
	    tlbfill
	    csrrd   $t0, 0x8b          # restore t0
	    ertn
