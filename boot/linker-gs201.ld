/* Pixel 7a / gs201 (Tensor G2) link layout for the EL2 hypervisor stub -
 * the board::DRAM_BASE-shifted twin of linker.ld's Superbird layout. See
 * board.rs's module comment.
 *
 * DRAM base 0x8000_0000, confirmed live via /proc/device-tree/memory@80000000
 * on real hardware (see this crate's port-investigation notes) - unlike
 * Superbird's/QEMU's single contiguous bank, gs201 has further
 * discontiguous banks above 32GiB that this hv doesn't map or use yet
 * (display-only bring-up only needs the first bank). Same relative
 * offsets as Superbird/QEMU otherwise: hv at DRAM_BASE+0x0100_0000, guest
 * kernel image at DRAM_BASE+0x0200_0000 (GUEST_ENTRY_PA in src/guest.rs)
 * - though no guest is loaded yet for this board; this scaffolding exists
 * to get display init running standalone first (see boot.rs's hvMain).
 */

ENTRY(_start)

SECTIONS
{
    . = 0x81000000;
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

    /* Stage-2 translation tables - see linker.ld's identical comment and
     * stage2.rs's module comment for the full rationale. Same three
     * tables regardless of board; only the addresses stage2.rs computes
     * from board::DRAM_BASE differ. */
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
