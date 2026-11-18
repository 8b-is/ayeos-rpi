// ayeos-rpi / ayeos / syscalls.c — minimal syscall layer for nullclaw
#include <stddef.h>

/* Syscall numbers (ayeOS-specific) */
#define AYEOS_SYSCALL_WRITE   1
#define AYEOS_SYSCALL_READ    2
#define AYEOS_SYSCALL_EXIT   60

long ayeos_syscall(long n, long a1, long a2, long a3) {
    long ret;
    __asm__ volatile (
        "svc #0"
        : "=r"(ret)
        : "r"(n), "r"(a1), "r"(a2), "r"(a3)
        : "memory"
    );
    return ret;
}

long ayeos_write(int fd, const void *buf, size_t count) {
    return ayeos_syscall(AYEOS_SYSCALL_WRITE, fd, (long)buf, (long)count);
}
