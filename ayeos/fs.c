// ayeos-rpi / ayeos / fs.c — minimal FAT stub
#include <stddef.h>

int ayeos_fs_mount(void) {
    /* Placeholder: FAT16/FAT32 mount from SD/MMC */
    return 0;
}

int ayeos_fs_read(const char *path, void *buf, size_t len) {
    (void)path; (void)buf; (void)len;
    return -1;
}
