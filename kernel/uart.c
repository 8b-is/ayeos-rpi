// ayeos-rpi / kernel / uart.c — PL011 UART driver (Pi 3 PL011, Pi 4 PL011 + mini-UART)
// Pi 3: primary UART = PL011 at 0x3F201000
// Pi 4: primary UART = PL011 at 0xFE201000 (mini-UART at 0xFE215040)
// Detect board via CPUID midr and select base.

#include <stddef.h>

#define UART_DR  0x00
#define UART_FR  0x18
#define UART_IBRD 0x24
#define UART_FBRD 0x28
#define UART_LCR_H 0x2C
#define UART_CR   0x30
#define UART_IMSC 0x38

#define FR_TXFF  (1 << 5)
#define CR_UARTEN (1 << 0)
#define CR_TXE    (1 << 8)
#define CR_RXE    (1 << 9)

static volatile unsigned long *uart_base = (void *)0xFE201000; /* Pi 4 default */

static void uart_select_base(void) {
    /* Read MIDR_EL1 to distinguish Pi 3 (BCM2837, midr 0x410FD034)
     * from Pi 4 (BCM2711, midr 0x410FD083). Fallback to Pi 4 base. */
    unsigned long midr;
    __asm__ volatile("mrs %0, midr_el1" : "=r"(midr));
    unsigned long implementer = (midr >> 24) & 0xFF;
    unsigned long part = (midr >> 4) & 0xFFF;

    if (implementer == 0x41 && part == 0xD03) {
        /* Cortex-A53 (Pi 3) */
        uart_base = (void *)0x3F201000;
    } else {
        /* Cortex-A72 (Pi 4) or unknown -> Pi 4 PL011 base */
        uart_base = (void *)0xFE201000;
    }
}

void uart_init(void) {
    uart_select_base();
    volatile unsigned long *reg = uart_base;

    /* Disable UART during config */
    reg[CR / 4] = 0;

    /* Baud: 115200 @ 48MHz (Pi 3/4 UART clock) -> IBRD=26, FBRD=0x33 approx */
    reg[IBRD / 4] = 26;
    reg[FBRD / 4] = 0x33;
    reg[LCR_H / 4] = (1 << 5) | (1 << 6); /* 8-bit, no parity */
    reg[CR / 4] = CR_UARTEN | CR_TXE | CR_RXE;
}

void uart_putc(char c) {
    volatile unsigned long *reg = uart_base;
    while (reg[FR / 4] & FR_TXFF)
        ;
    reg[DR / 4] = (unsigned long)c;
}

void uart_puts(const char *s) {
    while (*s) {
        if (*s == '\n')
            uart_putc('\r');
        uart_putc(*s++);
    }
}
