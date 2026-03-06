/* STM32H750IBK6 Memory Layout — Daisy Seed */
MEMORY
{
    /* Internal Flash — 128 KB (single bank, execute-in-place) */
    FLASH  (rx)  : ORIGIN = 0x08000000, LENGTH = 128K

    /* DTCM RAM — 128 KB, 0-wait state, data only (no execute) */
    /* Used for: audio DMA buffers, stack, hot DSP state */
    DTCMRAM (rw) : ORIGIN = 0x20000000, LENGTH = 128K

    /* AXI SRAM — 512 KB, 0-1 wait state */
    /* Used for: heap, delay lines, reverb buffers */
    RAM    (rwx) : ORIGIN = 0x24000000, LENGTH = 512K

    /* D2 SRAM1 — 128 KB, 1-2 wait state */
    /* Used for: DMA-accessible buffers (SAI audio) */
    SRAM1  (rw)  : ORIGIN = 0x30000000, LENGTH = 128K

    /* D2 SRAM2 — 128 KB, 1-2 wait state */
    SRAM2  (rw)  : ORIGIN = 0x30020000, LENGTH = 128K

    /* D2 SRAM3 — 32 KB, 1-2 wait state */
    SRAM3  (rw)  : ORIGIN = 0x30040000, LENGTH = 32K

    /* D3 SRAM4 — 64 KB, low-power domain */
    SRAM4  (rw)  : ORIGIN = 0x38000000, LENGTH = 64K

    /* External SDRAM — 64 MB, 4-8 wait state */
    /* Used for: long delay lines (>500ms), sampler buffers */
    SDRAM  (rwx) : ORIGIN = 0xC0000000, LENGTH = 64M
}

/* Stack in DTCM for zero-wait-state access */
_stack_start = ORIGIN(DTCMRAM) + LENGTH(DTCMRAM);
