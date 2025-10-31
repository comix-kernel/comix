	#
        # RISC-V User Trap Handling Trampoline
        #
	.section trampsec
.globl trampoline
trampoline:
.align 4
.globl uservec
uservec:    
        # 交换 sscratch 和 a0 的值，a0 现在指向当前进程的 trapframe 结构体。
        csrrw a0, sscratch, a0
        # 保存所有通用寄存器到 trapframe 结构体中
        sd ra, 8(a0)
        sd sp, 16(a0)
        sd gp, 24(a0)
        sd tp, 32(a0)
        sd t0, 40(a0)
        sd t1, 48(a0)
        sd t2, 56(a0)
        sd s0, 64(a0)
        sd s1, 72(a0)
        # 保存 sepc 的值
        csrr t0, sepc
        sd t0, 0(a0)
        # 保存 a0 的值
        csrr t0, sscratch
        sd t0, 80(a0)
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

        # 加载内核栈指针
        ld sp, 256(a0)
        # 加载陷阱处理函数地址
        ld t0, 264(a0)
        # 跳转到陷阱处理函数
        jr t0


