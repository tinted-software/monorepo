/* QEMU `virt` machine link layout for the EL2 hypervisor stub - the
 * board::DRAM_BASE-shifted twin of linker.ld's Superbird layout, used to
 * validate guest-entry mechanics (stage-2, EL1 init, single-step tracer,
 * eret) against a fast, deterministic, GDB-inspectable target before
 * trusting a result observed only on physical hardware over USB with no
 * reset between attempts. See board.rs's module comment.
 *
 * QEMU's `virt` machine always places DRAM at physical 0x4000_0000,
 * regardless of `-m`/`-cpu`/devicetree contents. Same relative offsets as
 * Superbird otherwise: hv at DRAM_BASE+0x0100_0000, guest kernel image at
 * DRAM_BASE+0x0200_0000 (GUEST_ENTRY_PA in src/guest.rs).
 */

ENTRY(_start)

SECTIONS
{
    . = 0x41000000;
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
