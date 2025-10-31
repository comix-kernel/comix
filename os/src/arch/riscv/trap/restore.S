    .text
    .globl __restore
__restore:
        # 恢复sepc
        ld t0, 0(a0)
        csrw sepc, t0
        # 恢复 sscratch
        ld t0, 80(a0)
        csrw sscratch, t0
        # 从 trapframe 结构体中恢复所有通用寄存器
        ld ra, 8(a0)
        ld sp, 16(a0)
        ld gp, 24(a0)
        ld tp, 32(a0)
        ld t0, 40(a0)
        ld t1, 48(a0)
        ld t2, 56(a0)
        ld s0, 64(a0)
        ld s1, 72(a0)

        ld a1, 88(a0)
        ld a2, 96(a0)
        ld a3, 104(a0)
        ld a4, 112(a0)
        ld a5, 120(a0)
        ld a6, 128(a0)
        ld a7, 136(a0)
        ld s2, 144(a0)
        ld s3, 152(a0)
        ld s4, 160(a0)
        ld s5, 168(a0)
        ld s6, 176(a0)
        ld s7, 184(a0)
        ld s8, 192(a0)
        ld s9, 200(a0)
        ld s10, 208(a0)
        ld s11, 216(a0)
        ld t3, 224(a0)
        ld t4, 232(a0)
        ld t5, 240(a0)
        ld t6, 248(a0)

        # 恢复 sstatus
        ld t0, 256(a0)
        csrw sstatus, t0

        csrrw a0, sscratch, a0

        # 中断返回:PC <- sepc，SIE <- SPIE，特权级 <- SPP
        sret
