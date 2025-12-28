    # trap_entry.S - RISC-V Trap Entry and Restore Routines
    # 保存和恢复上下文的汇编代码
    .text
    .globl trap_entry
    .globl trap_handler
    .globl __restore
    .align 4
trap_entry:
        # 保存上下文到 trap_frame 结构体中
        # 约束: sscratch 指向 trap_frame 结构体

        # 交换 sscratch 和 a0
        csrrw a0, sscratch, a0

        # 保存用户 tp (此时 tp 还是用户值)
        sd tp, 32(a0)

        # 从 TrapFrame 读取 cpu_ptr,设置到 tp
        ld tp, 272(a0)

        # 保存所有通用寄存器
        sd ra, 8(a0)
        sd sp, 16(a0)
        sd gp, 24(a0)
        # 注意: tp 已经在前面保存了

        sd t0, 40(a0)
        sd t1, 48(a0)
        sd t2, 56(a0)
        sd s0, 64(a0)
        sd s1, 72(a0)
        sd a1, 88(a0)
        sd a2, 96(a0)
        sd a3, 104(a0)
        sd a4, 112(a0)
        sd a5, 120(a0)
        sd a6, 128(a0)
        sd a7, 136(a0)
        sd s2, 144(a0)
        sd s3, 152(a0)
        sd s4, 160(a0)
        sd s5, 168(a0)
        sd s6, 176(a0)
        sd s7, 184(a0)
        sd s8, 192(a0)
        sd s9, 200(a0)
        sd s10, 208(a0)
        sd s11, 216(a0)
        sd t3, 224(a0)
        sd t4, 232(a0)
        sd t5, 240(a0)
        sd t6, 248(a0)

        # 保存 a0
        csrr t0, sscratch
        sd t0, 80(a0)

        # 保存 sepc
        csrr t0, sepc
        sd t0, 0(a0)
        # 保存 sstatus
        csrr t0, sstatus
        sd t0, 256(a0)

        # 转到内核栈
        # 内核进程不切换栈，用户进程必须切换栈  
        
        # 检查是否是内核态中断

        # 如果是用户态中断，切换到内核栈
        csrr t0, sstatus
        li t1, 0x100    # SPP 位掩码
        and t0, t0, t1
        bnez t0, .L1    # 如果 SPP=1，说明是内核态中断，跳过切换栈
        ld sp, 264(a0)
.L1:
        # 保存正确的内核 sp 到 TrapFrame.kernel_sp
        # 必须在调用 trap_handler 之前保存，否则 sp 会因为函数调用而下降
        sd sp, 264(a0)
        
        # 使用寄存器间接跳转以支持大代码模型（避免 JAL 范围限制）
        la t0, trap_handler
        jr t0

__restore:
        # 约束: a0 = trapframe 指针
        # 注意: kernel_sp 已在 trap_entry 中保存，这里不再保存
        # 恢复 sepc
        ld t0, 0(a0)
        csrw sepc, t0
        # 恢复 sstatus
        ld t0, 256(a0)
        csrw sstatus, t0
        # 恢复 sscratch
        csrw sscratch, a0

        # 从 trapframe 结构体中恢复所有通用寄存器
        ld ra, 8(a0)
        ld sp, 16(a0)
        ld gp, 24(a0)
        # 恢复用户 tp
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
        ld a0, 80(a0)
        
        # 中断返回:PC <- sepc，SIE <- SPIE，特权级 <- SPP
        sret