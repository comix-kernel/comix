	    #
        # 简化的陷阱处理程序入口点。
        # 用于启动极早期的内核陷阱处理。
        #
        # 保存，调用处理程序，和恢复所有通用寄存器。
        #
.globl boot_trap_entry
.globl trap_handler
.align 4
boot_trap_entry:
        addi sp, sp, -264

        // 保存寄存器
        sd ra, 8(sp)
        sd sp, 16(sp)
        sd gp, 24(sp)
        sd tp, 32(sp)
        sd t0, 40(sp)
        sd t1, 48(sp)
        sd t2, 56(sp)
        sd s0, 64(sp)
        sd s1, 72(sp)
        sd a0, 80(sp)
        sd a1, 88(sp)
        sd a2, 96(sp)
        sd a3, 104(sp)
        sd a4, 112(sp)
        sd a5, 120(sp)
        sd a6, 128(sp)
        sd a7, 136(sp)
        sd s2, 144(sp)
        sd s3, 152(sp)
        sd s4, 160(sp)
        sd s5, 168(sp)
        sd s6, 176(sp)
        sd s7, 184(sp)
        sd s8, 192(sp)
        sd s9, 200(sp)
        sd s10, 208(sp)
        sd s11, 216(sp)
        sd t3, 224(sp)
        sd t4, 232(sp)
        sd t5, 240(sp)
        sd t6, 248(sp)

        csrr a0, sepc       // 保存陷阱发生时的程序计数器
        sd a0, 0(sp)      // 将 sepc 保存到栈上
        csrr a0, sstatus    // 保存陷阱发生时的状态寄存器
        sd a0, 256(sp)      // 将 sstatus 保存到栈上

    	// call the trap handler in kerneltrap.rs
        call trap_handler

        // 恢复寄存器
        ld a0, 0(sp)
        csrw sepc, a0       // 恢复 sepc
        ld a0, 256(sp)
        csrw sstatus, a0    // 恢复 sstatus

        ld ra, 8(sp)
        ld sp, 16(sp)
        ld gp, 24(sp)
        // 不要恢复 tp，以防我们移动了CPU.
        //
        // 在多核系统（多 Hart）中，线程指针寄存器 (tp 或 x4) 
        // 通常用于存储当前 Hart/CPU 独有的数据（如指向该 Hart 的上下文或本地存储）。
        // 如果在内核陷阱处理（kerneltrap 的 C 代码）过程中，操作系统执行了进程/线程
        // 迁移，将当前的执行上下文调度到了另一个 Hart 上，那么恢复旧的 tp 值将是不正确的。
        // ld tp, 32(sp)
        ld t0, 40(sp)
        ld t1, 48(sp)
        ld t2, 56(sp)
        ld s0, 64(sp)
        ld s1, 72(sp)
        ld a0, 80(sp)
        ld a1, 88(sp)
        ld a2, 96(sp)
        ld a3, 104(sp)
        ld a4, 112(sp)
        ld a5, 120(sp)
        ld a6, 128(sp)
        ld a7, 136(sp)
        ld s2, 144(sp)
        ld s3, 152(sp)
        ld s4, 160(sp)
        ld s5, 168(sp)
        ld s6, 176(sp)
        ld s7, 184(sp)
        ld s8, 192(sp)
        ld s9, 200(sp)
        ld s10, 208(sp)
        ld s11, 216(sp)
        ld t3, 224(sp)
        ld t4, 232(sp)
        ld t5, 240(sp)
        ld t6, 248(sp)

        addi sp, sp, 264

        // return to whatever we were doing in the kernel.
        sret