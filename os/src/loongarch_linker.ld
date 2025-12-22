OUTPUT_ARCH(loongarch)
ENTRY(_start)

/*
 * LoongArch64 内核链接器脚本
 *
 * 策略：所有段链接到虚拟地址，使用 AT() 指定物理加载地址。
 * QEMU 加载 ELF 时会使用物理地址，运行时通过 DMW 访问虚拟地址。
 *
 * DMW1 映射：VA 0x9000_xxxx_xxxx_xxxx -> PA 0x0000_xxxx_xxxx_xxxx
 */

/* 物理加载地址 - QEMU virt 平台 RAM */
PHYSICAL_BASE = 0x200000;

/* 虚拟地址基址 - 通过 DMW1 映射 (VSEG = 0x9) */
VIRTUAL_BASE = 0x9000000000200000;

SECTIONS
{
    . = VIRTUAL_BASE;                 /* 虚拟地址计数器 */

    skernel = .;
    stext = .;

    .text : AT(PHYSICAL_BASE) {       /* AT() 指定物理加载地址 */
        *(.text.entry)
        . = ALIGN(4K);
        strampoline = .;              /* 标记 trampoline 虚拟地址 */
        *(.text.trampoline);          /* trampoline 代码段 */
        . = ALIGN(4K);
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .early_test_entry : {
        __early_test_start = .;       /* 定义段起始符号 */
        KEEP(*(.early_test_entry))    /* KEEP 防止链接器优化掉未使用的段 */
        __early_test_end = .;         /* 定义段结束符号 */
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
    . = . + 32M;                      /* 堆空间 */
    eheap = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
