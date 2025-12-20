OUTPUT_ARCH(loongarch)
ENTRY(_start)

/* LoongArch QEMU virt 平台内存布局 */
/* 物理 RAM 起始于 0x0，内核加载到 0x90000000 */
PHYSICAL_BASE = 0x90000000;

SECTIONS
{
    . = PHYSICAL_BASE;

    skernel = .;
    stext = .;

    .text : {
        *(.text.entry)
        . = ALIGN(4K);
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    srodata = .;

    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;

    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;

    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
