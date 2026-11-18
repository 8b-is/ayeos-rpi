/* ayeos-rpi / ayeos / linker.ld — ayeOS payload linker script */
ENTRY(ayeos_entry)

SECTIONS
{
    . = 0x100000;

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
