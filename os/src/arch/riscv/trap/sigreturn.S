// 系统调用号的值
.equ __NR_rt_sigreturn, 139

    .text
    .globl __sigreturn_trampoline
    .type __sigreturn_trampoline, @function  // 指定符号类型
    .align 4
    
__sigreturn_trampoline:
    // 将 rt_sigreturn 的系统调用号 (139) 载入 a7 寄存器
    li   a7, __NR_rt_sigreturn  
    
    // 执行环境调用，陷入内核
    ecall
