# 上下文切换
#
#   fn switch(struct context *old, struct context *new);
#
# 保存当前任务的寄存器状态到 old 指向的 context 结构体中，
# 然后从 new 指向的 context 结构体中恢复寄存器状态，
# 实现任务切换。

.globl switch
switch:
        st.d    $r1,  $a0, 0
        st.d    $r3,  $a0, 8
        st.d    $r23, $a0, 16
        st.d    $r24, $a0, 24
        st.d    $r25, $a0, 32
        st.d    $r26, $a0, 40
        st.d    $r27, $a0, 48
        st.d    $r28, $a0, 56
        st.d    $r29, $a0, 64
        st.d    $r30, $a0, 72
        st.d    $r31, $a0, 80

        ld.d    $r1,  $a1, 0
        ld.d    $r3,  $a1, 8
        ld.d    $r23, $a1, 16
        ld.d    $r24, $a1, 24
        ld.d    $r25, $a1, 32
        ld.d    $r26, $a1, 40
        ld.d    $r27, $a1, 48
        ld.d    $r28, $a1, 56
        ld.d    $r29, $a1, 64
        ld.d    $r30, $a1, 72
        ld.d    $r31, $a1, 80

        jirl    $r0, $r1, 0
