    .text
    .globl __sigreturn_trampoline
    .align 4

__sigreturn_trampoline:
    li.w    $a7, 139
    syscall 0
