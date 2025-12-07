OUTPUT_ARCH(riscv)
ENTRY(_start)
PHYSICAL_BASE = 0x80200000;           /* 物理地址 */
VIRTUAL_BASE = 0xffffffc080200000;    /* 虚拟地址（高半核），与arch/mm保持一致 */

SECTIONS
{
    . = VIRTUAL_BASE;                 /* 虚拟地址计数器 */

    skernel = .;
    stext = .;

    .text : AT(PHYSICAL_BASE) {       /* AT() 指定物理地址 */
        *(.text.entry)
        . = ALIGN(4K);
        strampoline = .;         /* 标记 trampoline 虚拟地址 */
        *(.text.trampoline);     /* trampoline 代码段 */
        . = ALIGN(4K);           /* 对齐到页边界 */
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .early_test_entry :
        {
            __early_test_start = .; /* 定义段起始符号 */
            KEEP(*(.early_test_entry)) /* KEEP 防止链接器优化掉未使用的段 */
            __early_test_end = .;   /* 定义段结束符号 */
        }

    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : AT(PHYSICAL_BASE + (srodata - VIRTUAL_BASE)) {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : AT(PHYSICAL_BASE + (sdata - VIRTUAL_BASE)) {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    .bss : AT(PHYSICAL_BASE + (edata - VIRTUAL_BASE)) {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    sheap = .;
    . = . + 32M;
    eheap = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}