OUTPUT_ARCH(loongarch)
ENTRY(_start)

/* LoongArch QEMU virt 平台内存布局 */
/* 参考 RISC-V linker.ld 的设计 */
/* 
 * QEMU virt 机器将内核加载到物理地址 0x200000 附近
 * 通过 DMW1 可以使用虚拟地址 0x9000_0000_xxxx_xxxx 访问
 * 当前简化版本：直接使用物理地址，未启用虚实分离
 * TODO: 未来可以参考 bakaos 实现完整的虚实地址分离
 */

/* 物理加载地址 - QEMU virt 平台 RAM 起始附近 */
PHYSICAL_BASE = 0x200000;

/* 虚拟地址基址 - 通过 DMW1 映射 (暂时与物理地址相同) */
/* 注意: 当前 entry.S 未实现地址跳转，所以使用物理地址 */
VIRTUAL_BASE = PHYSICAL_BASE;

SECTIONS
{
    . = VIRTUAL_BASE;

    skernel = .;
    stext = .;

    .text : AT(PHYSICAL_BASE) {
        *(.text.entry)
        . = ALIGN(4K);
        *(.text .text.*)
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
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
