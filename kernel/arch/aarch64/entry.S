/* ayeos-rpi / kernel / arch / aarch64 / entry.S — ARM64 boot block (Pi 3 / Pi 4)
 *
 * VideoCore loads kernel8.img at 0x80000 and jumps here.
 * Sets up stack, clears BSS, programs VBAR, enables timer, then calls kernel_main.
 */

    .section .boot, "ax", @progbits
    .globl _start
    .align 4
_start:
    /* Initialize stack pointer (top of a 64K region above load address) */
    adr    x0, _start
    add    sp, x0, #(64 * 1024)

    /* Clear BSS */
    adr    x0, __bss_start
    adr    x1, _end
    sub    x1, x1, x0
    mov    x2, #0
1:  cbz    x1, 2f
    str    x2, [x0], #8
    sub    x1, x1, #8
    b      1b
2:

    /* Set up exception vector base (VBAR_EL1 -> vectors) */
    adr    x0, _vectors
    msr    vbar_el1, x0
    isb

    /* Enable timer (CNTFRQ read) */
    mrs    x0, cntfrq_el0

    /* Jump to C kernel */
    bl     kernel_main

    /* Should not return */
3:  wfe
    b      3b

/* Minimal exception vector table (all exceptions -> panic loop) */
    .align 4
_vectors:
    .quad 1f, 1f, 1f, 1f   /* EL1 Sync, IRQ, FIQ, SError */
    .quad 1f, 1f, 1f, 1f   /* EL0 64-bit, ... */
    .quad 1f, 1f, 1f, 1f   /* ... */
    .quad 1f, 1f, 1f, 1f
1:  wfe
    b      1b

    .section .bss, "aw", @nobits
    .align 4
__bss_start:
    .skip 4096
_end:
