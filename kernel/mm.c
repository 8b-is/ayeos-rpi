// ayeos-rpi / kernel / mm.c — minimal MMU setup stub
// Full stage-1 translation tables are board-specific; this stub keeps identity mapping
// and enables nothing yet. A real ayeOS would program TCR_EL1, TTBR0_EL1, MAIR_EL1, SCTLR_EL1.

#include <stddef.h>

void mm_init(void) {
    /* Placeholder: read current SCTLR_EL1 to confirm EL1 */
    unsigned long sctlr;
    __asm__ volatile("mrs %0, sctlr_el1" : "=r"(sctlr));
    (void)sctlr;
}
