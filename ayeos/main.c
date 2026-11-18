// ayeos-rpi / ayeos / main.c — ayeOS entry (jumped to from kernel8 at 0x100000)
#include <stddef.h>

extern void uart_init(void);
extern void uart_puts(const char *s);
extern void uart_putc(char c);

void ayeos_entry(void) {
    uart_init();
    uart_puts("ayeOS v0.1\n");
    uart_puts("pi 3 / 4 userspace OS\n");
    uart_puts("init: syscalls + fs shell\n");

    /* Minimal shell loop placeholder */
    uart_puts("> ");
    while (1) {
        /* In a real build: read char, parse command, execute */
        for (volatile long i = 0; i < 1000000; i++) __asm__ volatile("" ::: "memory");
    }
}
