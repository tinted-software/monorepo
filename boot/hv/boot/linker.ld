/* Meson G12A (Superbird) link layout for the EL2 hypervisor stub.
 *
 * Shares the board's memory map with //src/kernel:kernel-meson
 * (ref/src/kernel/boot/linker-meson.ld) - DRAM 0x0..0x2000_0000, secmon
 * reserved 0x0500_0000..0x0730_0000, U-Boot's own relocated copy near
 * ~0x1700_0000 - but loads *below* the native kernel's 0x0200_0000 so the
 * two never collide: the hv owns 0x0100_0000..0x0180_0000 (8 MiB, image +
 * stacks + stage-2 tables), and hands off into the vmapple guest kernel
 * image loaded separately at 0x0200_0000 (GUEST_ENTRY_PA in src/guest.rs -
 * keep that constant and this base in sync).
 */

ENTRY(_start)

SECTIONS
{
    . = 0x01000000;
    __image_start = .;

    .text : {
        KEEP(*(.text.boot))
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .got : {
        *(.got .got.*)
    }

    . = ALIGN(16);
    __bss_start = .;
    .bss (NOLOAD) : {
        *(.bss .bss.*)
        *(COMMON)
    }
    . = ALIGN(16);
    __bss_end = .;

    . = ALIGN(4096);
    __boot_stack_bottom = .;
    . += 0x10000; /* 64KiB EL2 boot stack - core 0 (primary) only */
    __boot_stack_top = .;

    /* One 64KiB EL2 stack per secondary core, indexed by core ID exactly
     * like the native kernel's __secondary_stacks_bottom. */
    . = ALIGN(4096);
    __secondary_stacks_bottom = .;
    . += 0x10000 * 4;
    __secondary_stacks_top = .;

    /* Stage-2 translation tables, 4KiB granule, starting at level 1 (VTCR_EL2
     * SL0=1): one L1 table (4 of 512 entries used - IPA space is capped at
     * 4GiB, one 1GiB block per entry); L1[0] (IPA 0x0..0x3fff_ffff, which
     * contains all of DRAM plus some low Amlogic MMIO) descends to its own
     * L2 table so DRAM can be mapped Normal-WB at 2MiB granularity instead
     * of Device for the whole GiB (executing guest code from a stage-2
     * Device mapping is UNPREDICTABLE per the ARM ARM, not merely slow -
     * see src/stage2.rs's module comment); L1[3] similarly descends to a
     * second L2 table for the single 1GiB slot containing the real
     * GIC-400, which is trapped/software-emulated rather than identity
     * mapped - see that same module comment for why. */
    . = ALIGN(4096);
    __stage2_l1_table = .;
    . += 0x1000;
    __stage2_dram_l2_table = .;
    . += 0x1000;
    __stage2_trap_l2_table = .;
    . += 0x1000;

    . = ALIGN(4096);
    __kernel_end = .;
    __image_end = .;
    __image_size = __image_end - __image_start;

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame .eh_frame_hdr)
        *(.note .note.*)
        *(.debug_*)
    }
}
