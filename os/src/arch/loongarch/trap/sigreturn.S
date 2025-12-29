.text
.globl __sigreturn_trampoline
.type __sigreturn_trampoline, @function
.align 4

# LoongArch64 用户态信号返回跳板：发起 rt_sigreturn 系统调用
__sigreturn_trampoline:
    li.w   $a7, 139          # __NR_rt_sigreturn
    syscall 0
