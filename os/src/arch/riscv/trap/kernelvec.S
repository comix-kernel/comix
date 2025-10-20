	    #
        # S特权级别的陷阱处理程序入口点。
        #
        # 保存，调用处理程序，和恢复所有通用寄存器。
        #
.globl kerneltrap
.globl kernelvec
.align 4
kernelvec:
        addi sp, sp, -256

        // 保存寄存器
        sd ra, 0(sp)
        sd sp, 8(sp)
        sd gp, 16(sp)
        sd tp, 24(sp)
        sd t0, 32(sp)
        sd t1, 40(sp)
        sd t2, 48(sp)
        sd s0, 56(sp)
        sd s1, 64(sp)
        sd a0, 72(sp)
        sd a1, 80(sp)
        sd a2, 88(sp)
        sd a3, 96(sp)
        sd a4, 104(sp)
        sd a5, 112(sp)
        sd a6, 120(sp)
        sd a7, 128(sp)
        sd s2, 136(sp)
        sd s3, 144(sp)
        sd s4, 152(sp)
        sd s5, 160(sp)
        sd s6, 168(sp)
        sd s7, 176(sp)
        sd s8, 184(sp)
        sd s9, 192(sp)
        sd s10, 200(sp)
        sd s11, 208(sp)
        sd t3, 216(sp)
        sd t4, 224(sp)
        sd t5, 232(sp)
        sd t6, 240(sp)

    	// call the trap handler in kerneltrap.rs
        call kerneltrap

        // 恢复寄存器
        ld ra, 0(sp)
        ld sp, 8(sp)
        ld gp, 16(sp)
        // 不要恢复 tp，以防我们移动了CPU.
        //
        // 在多核系统（多 Hart）中，线程指针寄存器 (tp 或 x4) 
        // 通常用于存储当前 Hart/CPU 独有的数据（如指向该 Hart 的上下文或本地存储）。
        // 如果在内核陷阱处理（kerneltrap 的 C 代码）过程中，操作系统执行了进程/线程
        // 迁移，将当前的执行上下文调度到了另一个 Hart 上，那么恢复旧的 tp 值将是不正确的。
        // ld tp, 24(sp)
        ld t0, 32(sp)
        ld t1, 40(sp)
        ld t2, 48(sp)
        ld s0, 56(sp)
        ld s1, 64(sp)
        ld a0, 72(sp)
        ld a1, 80(sp)
        ld a2, 88(sp)
        ld a3, 96(sp)
        ld a4, 104(sp)
        ld a5, 112(sp)
        ld a6, 120(sp)
        ld a7, 128(sp)
        ld s2, 136(sp)
        ld s3, 144(sp)
        ld s4, 152(sp)
        ld s5, 160(sp)
        ld s6, 168(sp)
        ld s7, 176(sp)
        ld s8, 184(sp)
        ld s9, 192(sp)
        ld s10, 200(sp)
        ld s11, 208(sp)
        ld t3, 216(sp)
        ld t4, 224(sp)
        ld t5, 232(sp)
        ld t6, 240(sp)

        addi sp, sp, 256

        // return to whatever we were doing in the kernel.
        sret