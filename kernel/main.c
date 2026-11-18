// ayeos-rpi / kernel / main.c — kernel entry (C) for Pi 3 / Pi 4
#include <stddef.h>

extern void uart_init(void);
extern void uart_putc(char c);
extern void uart_puts(const char *s);
extern void timer_init(void);
extern void mm_init(void);

static void put_hex(unsigned long v) {
    char hex[] = "0123456789abcdef";
    uart_puts("0x");
    for (int i = 60; i >= 0; i -= 4) {
        uart_putc(hex[(v >> i) & 0xF]);
    }
    uart_putc('\n');
}

void kernel_main(void) {
    uart_init();
    uart_puts("ayeos-rpi kernel8\n");
    uart_puts("board: raspberry pi 3 / 4 (armv8-a)\n");

    mm_init();
    timer_init();

    uart_puts("MMU + timer ready\n");

    /* Load ayeOS payload from SD/FAT and jump to 0x100000 */
    uart_puts("loading ayeOS...\n");

    /* Hang: in a real build, this loads ayeos.img and jumps. */
    while (1) {
        uart_putc('.');
        for (volatile long i = 0; i < 500000; i++) __asm__ volatile("" ::: "memory");
    }
}
