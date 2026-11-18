# ayeos-rpi / kernel / linker.ld — ARM64 bare-metal linker script
ENTRY(_start)

SECTIONS
{
    . = 0x80000;

    .boot : {
        *(.boot)
    }

    .text : {
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
        *(COMMON)
    }

    . = ALIGN(4);
    _end = .;
}
