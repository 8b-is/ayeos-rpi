// ayeos-rpi / kernel / timer.c — ARM arch timer (CNTFRQ/CNTPCT)
#include <stddef.h>

void timer_init(void) {
    /* Read counter frequency (just to verify it's running) */
    unsigned long freq;
    __asm__ volatile("mrs %0, cntfrq_el0" : "=r"(freq));
    (void)freq;
}

unsigned long timer_read(void) {
    unsigned long val;
    __asm__ volatile("mrs %0, cntpct_el0" : "=r"(val));
    return val;
}

void timer_delay(unsigned long cycles) {
    unsigned long start = timer_read();
    while ((timer_read() - start) < cycles)
        ;
}
