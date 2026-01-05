    .section .text.trap
    .globl trap_entry
    .globl trap_handler
    .globl __restore
    .align 12

#
# LoongArch64 trap entry
# - CSR.SAVE0 (0x30) holds current TrapFrame pointer
#
trap_entry:
    csrrd   $t0, 0x30

    # Save general registers r0-r31
    st.d    $r0,  $t0, 0
    st.d    $r1,  $t0, 8
    st.d    $r2,  $t0, 16
    st.d    $r3,  $t0, 24
    st.d    $r4,  $t0, 32
    st.d    $r5,  $t0, 40
    st.d    $r6,  $t0, 48
    st.d    $r7,  $t0, 56
    st.d    $r8,  $t0, 64
    st.d    $r9,  $t0, 72
    st.d    $r10, $t0, 80
    st.d    $r11, $t0, 88
    st.d    $r12, $t0, 96
    st.d    $r13, $t0, 104
    st.d    $r14, $t0, 112
    st.d    $r15, $t0, 120
    st.d    $r16, $t0, 128
    st.d    $r17, $t0, 136
    st.d    $r18, $t0, 144
    st.d    $r19, $t0, 152
    st.d    $r20, $t0, 160
    st.d    $r21, $t0, 168
    st.d    $r22, $t0, 176
    st.d    $r23, $t0, 184
    st.d    $r24, $t0, 192
    st.d    $r25, $t0, 200
    st.d    $r26, $t0, 208
    st.d    $r27, $t0, 216
    st.d    $r28, $t0, 224
    st.d    $r29, $t0, 232
    st.d    $r30, $t0, 240
    st.d    $r31, $t0, 248

    # Save ERA/ESTAT/CRMD/PRMD
    csrrd   $t1, 0x6
    st.d    $t1, $t0, 256
    csrrd   $t1, 0x5
    st.d    $t1, $t0, 264
    csrrd   $t1, 0x0
    st.d    $t1, $t0, 272
    csrrd   $t1, 0x1
    st.d    $t1, $t0, 280

    # Save current SP as kernel_sp (may be user SP if no stack switch)
    st.d    $sp, $t0, 288

    move    $a0, $t0
    bl      trap_handler

__restore:
    # a0 = TrapFrame*
    move    $t0, $a0

    # Restore ERA/PRMD before returning
    ld.d    $t1, $t0, 256
    csrwr   $t1, 0x6
    ld.d    $t1, $t0, 280
    csrwr   $t1, 0x1

    # Restore general registers (leave r12/t0 last)
    ld.d    $r1,  $t0, 8
    ld.d    $r2,  $t0, 16
    ld.d    $r3,  $t0, 24
    ld.d    $r4,  $t0, 32
    ld.d    $r5,  $t0, 40
    ld.d    $r6,  $t0, 48
    ld.d    $r7,  $t0, 56
    ld.d    $r8,  $t0, 64
    ld.d    $r9,  $t0, 72
    ld.d    $r10, $t0, 80
    ld.d    $r11, $t0, 88
    ld.d    $r13, $t0, 104
    ld.d    $r14, $t0, 112
    ld.d    $r15, $t0, 120
    ld.d    $r16, $t0, 128
    ld.d    $r17, $t0, 136
    ld.d    $r18, $t0, 144
    ld.d    $r19, $t0, 152
    ld.d    $r20, $t0, 160
    ld.d    $r21, $t0, 168
    ld.d    $r22, $t0, 176
    ld.d    $r23, $t0, 184
    ld.d    $r24, $t0, 192
    ld.d    $r25, $t0, 200
    ld.d    $r26, $t0, 208
    ld.d    $r27, $t0, 216
    ld.d    $r28, $t0, 224
    ld.d    $r29, $t0, 232
    ld.d    $r30, $t0, 240
    ld.d    $r31, $t0, 248
    ld.d    $r12, $t0, 96

    ertn
