//! Guest (vmapple XNU) boot handoff: the `boot_args` structure iBoot
//! normally builds and hands to XNU's arm64 entry point in x0, and the
//! `eret` that becomes the guest's first instruction.
//!
//! Field layout transcribed exactly from
//! `pexpert/pexpert/arm64/boot.h`'s `struct boot_args` in the
//! xnu-12377.121.6 tree at /Volumes/Dev/xnu (Revision/Version 1/2,
//! `kBootArgsRevision2` for the `bootFlags` field XNU actually reads).
//!
//! # Not yet implemented: the devicetree blob itself
//!
//! `deviceTreeP`/`deviceTreeLength` below point at a stub - a *correctly
//! shaped* Apple DeviceTree (Apple's own proprietary flattened
//! name/property-list binary format, unrelated to the FDT/DTB format
//! U-Boot and the native kernel's own `devicetree.rs` consume) with at
//! minimum a root node whose `compatible` matches what `pexpert`'s board
//! matching expects for VMA2, plus `chosen`/memory nodes, is real work
//! that hasn't been done yet. Booting past `PE_init_iokit` needs it;
//! reaching the HVC calls this hypervisor already answers does not - the
//! earliest hypercalls (PAC setup) happen in `start.s` before the
//! devicetree is ever consulted.

use core::mem::size_of;

/// Where the guest kernel image's Mach-O file bytes are expected to be
/// loaded (raw, unmodified, starting at the file's own offset 0) - must
/// match `HV_LOAD_ADDR` avoidance in boot/linker.ld's header comment.
/// `tools/boot_xnu.sh` writes them here over USB before starting `hv`.
///
/// The `0x4000`-odd tail is NOT arbitrary and must not be "cleaned up" to a
/// round address. XNU's `start.s` builds its bootstrap page tables out of
/// *block* descriptors, and `create_l2_block_entries` masks the physical
/// base it is handed with `ARM_TTE_BLOCK_L2_MASK` (osfmk/arm64/start.s) -
/// i.e. the mapping it installs is block-relative, so it is only an
/// identity/KVA map if the load address and `GUEST_VIRT_BASE` share the
/// same offset within a block. This kernel is `__ARM_16K_PG__`, making an
/// L2 block 32MiB (`ARM_TT_L2_SHIFT` 25), so the requirement is exactly:
///
///     GUEST_ENTRY_PA % 32MiB == GUEST_VIRT_BASE % 32MiB   (== 0x100_4000)
///
/// Real iBoot satisfies this by construction; we have to do it by hand.
/// Violating it does not fail loudly - the guest runs all of `start.s`,
/// enables its MMU and trampolines to KVA successfully, then fetches its C
/// entry point from an address off by the residue difference and dies on
/// whatever garbage it decodes there (observed: `_arm_init` faulting as an
/// undefined instruction while the file plainly holds a `pacibsp`).
/// `DRAM_BASE` is 32MiB-aligned on both boards, so the residue comes
/// entirely from the offset below.
pub const GUEST_ENTRY_PA: u64 = crate::board::DRAM_BASE + 0x0300_4000;

/// The vmapple KDK kernel's own base VA (its `__TEXT` segment's `vmaddr`).
/// The image is laid out in physical memory mirroring its virtual layout,
/// so every segment satisfies `PA == GUEST_ENTRY_PA + (vmaddr -
/// GUEST_VIRT_BASE)`.
///
/// That is NOT the same as copying the Mach-O file byte-for-byte, and this
/// comment previously claimed it was. `vmaddr - fileoff` is *not* constant
/// across segments: `__DATA` has `vmsize 0x178000` but `filesize 0x4c000`,
/// so the 0x12c000 bytes of zero-fill it occupies in memory but not in the
/// file shift every later segment. A flat copy lands `__BOOTDATA` -
/// precisely what XNU's early boot consumes - 0x12c000 bytes below where
/// the kernel looks for it, along with `__LINKINFO`/`__LINKEDIT`.
/// `tools/prepare_guest.py` performs the real segment-wise placement and
/// reports which segments a flat copy would misplace.
///
/// Kernel-version-specific: re-derive if the KDK kernel is ever updated.
/// `tools/prepare_guest.py` prints `virt_base`, the entry offset and the
/// image span it produced; `tools/boot_xnu.sh` cross-checks them against
/// the constants here and fails the boot on any mismatch.
const GUEST_VIRT_BASE: u64 = 0xffff_fe00_0700_4000;

/// Bytes of physical memory the laid-out image occupies: the span from
/// `__TEXT`'s `vmaddr` through the highest `vmaddr + vmsize` of any
/// memory-resident segment (`__LINKEDIT`'s end), as computed by
/// `tools/prepare_guest.py`. Larger than the Mach-O file's own loadable
/// extent because of the zero-fill described above.
const GUEST_IMAGE_LEN: u64 = 0x0175_4000;

/// Physical address one past everything the boot loader placed in DRAM -
/// `boot_args`' `topOfKernelData`, telling the guest where it may start
/// allocating. Must cover the `boot_args`/devicetree blobs too, not just the
/// kernel image, since those now sit immediately above it (see
/// `BOOT_ARGS_PA`) and the guest would otherwise be free to allocate over
/// the very structure it is still reading.
const GUEST_TOP_OF_KERNEL_DATA: u64 =
    (DEVICE_TREE_PA + DEVICE_TREE_STUB_LEN as u64).next_multiple_of(0x4000);

/// Physical memory the guest is told it owns: everything from the kernel
/// load address to the end of DRAM. See `mem_size` in `build_boot_args`.
const GUEST_MEM_SIZE: u64 = crate::board::DRAM_BASE + crate::board::DRAM_SIZE - GUEST_ENTRY_PA;

/// `LC_UNIXTHREAD`'s `pc` register, converted from the kernel's own VA to
/// the physical address `enter_guest` should set `ELR_EL2` to.
pub const GUEST_ENTRY_POINT_PA: u64 = GUEST_ENTRY_PA + (0xffff_fe00_073e_8480 - GUEST_VIRT_BASE);

/// First 4 bytes of a little-endian 64-bit Mach-O (`MH_MAGIC_64`) -
/// checked at `GUEST_ENTRY_PA` to tell a real loaded kernel image apart
/// from whatever DRAM happened to contain at cold boot, since this
/// hypervisor has no other way to know whether the host tool wrote one
/// before starting it (see `boot.rs`'s `hvMain`).
pub const MACHO_MAGIC_64: u32 = 0xfeed_facf;

const BOOT_LINE_LENGTH: usize = 1024;
const K_BOOT_ARGS_REVISION2: u16 = 2;
const K_BOOT_ARGS_VERSION2: u16 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct BootVideo {
    v_base_addr: u64,
    v_display: u64,
    v_row_bytes: u64,
    v_width: u64,
    v_height: u64,
    v_depth: u64,
}

#[repr(C)]
struct BootArgs {
    revision: u16,
    version: u16,
    virt_base: u64,
    phys_base: u64,
    mem_size: u64,
    top_of_kernel_data: u64,
    video: BootVideo,
    machine_type: u32,
    device_tree_p: u64, // `void *` in C - kept as a plain PA here
    device_tree_length: u32,
    command_line: [u8; BOOT_LINE_LENGTH],
    boot_flags: u64,
    mem_size_actual: u64,
}

const _BOOT_ARGS_LAYOUT_SANITY: () = assert!(size_of::<BootArgs>() > 0);

/// XNU's block-granular bootstrap mapping is only a correct identity/KVA map
/// when the load address and the kernel's own base VA sit at the same offset
/// within a 16KiB-granule L2 block (32MiB) - see `GUEST_ENTRY_PA`. Enforced
/// here because violating it produces a guest that boots deep into
/// `start.s`, enables its MMU, and only then dies on a garbage instruction
/// fetch, which is expensive to diagnose from the symptom.
const _GUEST_LOAD_BLOCK_RESIDUE_MATCHES: () = {
    const L2_BLOCK: u64 = 32 * 1024 * 1024;
    assert!(GUEST_ENTRY_PA % L2_BLOCK == GUEST_VIRT_BASE % L2_BLOCK);
};

/// Physical scratch location for the constructed `boot_args` + stub
/// devicetree.
///
/// These MUST live *above* `GUEST_ENTRY_PA`, not below it. XNU does not read
/// `boot_args` through the physical pointer it gets in x0 for long: very
/// early in `arm_init` it converts that pointer to a kernel virtual address
/// (`gVirtBase`/`gPhysBase`, i.e. `pa - phys_base + virt_base`) and
/// dereferences the KVA instead. The only translations that exist at that
/// moment are the bootstrap block mappings `start.s` built, and those start
/// at the kernel Mach-O header and run *upward* to the end of memory -
/// nothing below the header is mapped at all. `boot_args` previously sat at
/// `DRAM_BASE + 0x180_0000`, below the image, which cost a level-2
/// translation fault on the first KVA `boot_args` field read inside
/// `arm_init` (FAR was exactly `phystokv(BOOT_ARGS_PA)`).
///
/// Placing them just past the image also keeps them clear of the hv's own
/// image and stage-2 tables, which stay below `GUEST_ENTRY_PA` and are
/// deliberately excluded from the memory the guest is told it owns (see
/// `mem_size` in `build_boot_args`).
const BOOT_ARGS_PA: u64 = GUEST_ENTRY_PA + GUEST_IMAGE_LEN;
const DEVICE_TREE_PA: u64 = BOOT_ARGS_PA + 0x4000;
/// Placeholder size until the real Apple DeviceTree builder exists (see
/// module comment) - large enough that a future real blob has headroom
/// without relocating this constant immediately.
const DEVICE_TREE_STUB_LEN: u32 = 0x1000;

/// Writes a `boot_args` (and a zeroed devicetree-shaped stub) to
/// `BOOT_ARGS_PA`/`DEVICE_TREE_PA` describing the memory layout this
/// hypervisor's stage-2 map presents to the guest. Returns the physical
/// address to hand the guest in x0.
///
/// # Safety
/// Must only be called once, after stage-2 setup, before guest entry.
pub unsafe fn build_boot_args(command_line: &str) -> u64 {
    let mut args = BootArgs {
        revision: 1,
        version: K_BOOT_ARGS_VERSION2,
        virt_base: GUEST_VIRT_BASE,
        phys_base: GUEST_ENTRY_PA,
        // Memory the guest owns, measured from `phys_base` - NOT the full
        // DRAM size. `phys_base` is the kernel load address rather than the
        // base of DRAM, so everything below it (this hypervisor's own image
        // and its stage-2 tables) is deliberately invisible to the guest,
        // and counting it here would both overstate RAM and let the guest
        // allocate over us. It also decides how far `start.s` extends its
        // bootstrap block mappings (`memSize >> ARM_TT_L2_SHIFT` entries),
        // so overstating it maps blocks past the end of real DRAM.
        mem_size: GUEST_MEM_SIZE,
        top_of_kernel_data: GUEST_TOP_OF_KERNEL_DATA,
        // Framebuffer: populate from adopted display panel (Superbird) or dummy/ramfb (QEMU)
        video: {
            #[cfg(not(feature = "qemu"))]
            {
                if crate::display::is_present() {
                    let fb = crate::display::get_info();
                    let depth_code = match fb.format {
                        crate::display::PixelFormat::Rgb565 => 16,
                        crate::display::PixelFormat::Xrgb8888
                        | crate::display::PixelFormat::Argb8888 => 32,
                        _ => 32,
                    };
                    BootVideo {
                        v_base_addr: fb.addr as u64,
                        v_display: 1, // kPEGraphicsMode
                        v_row_bytes: fb.stride as u64,
                        v_width: fb.width as u64,
                        v_height: fb.height as u64,
                        v_depth: depth_code,
                    }
                } else {
                    BootVideo {
                        v_base_addr: 0,
                        v_display: 0,
                        v_row_bytes: 0,
                        v_width: 0,
                        v_height: 0,
                        v_depth: 0,
                    }
                }
            }
            #[cfg(feature = "qemu")]
            {
                BootVideo {
                    v_base_addr: 0,
                    v_display: 0,
                    v_row_bytes: 0,
                    v_width: 0,
                    v_height: 0,
                    v_depth: 0,
                }
            }
        },
        machine_type: 0,
        // `device_tree_p` must be a KVA (or translated through `phystokv`).
        // Passing `phystokv(DEVICE_TREE_PA)` ensures `PE_state.deviceTreeHead`
        // is a valid virtual address (`0xfffffe000875c000`), so `kvtophys(DTRootNode)`
        // evaluates to `DEVICE_TREE_PA >= GUEST_ENTRY_PA`, preventing `SecureDTIsLockedDown()`
        // from falsely misclassifying the DeviceTree as EXTRADATA and clobbering `segLOWEST`.
        device_tree_p: (DEVICE_TREE_PA - GUEST_ENTRY_PA) + GUEST_VIRT_BASE,
        device_tree_length: DEVICE_TREE_STUB_LEN,
        command_line: [0; BOOT_LINE_LENGTH],
        boot_flags: 0,
        mem_size_actual: GUEST_MEM_SIZE,
    };
    // kBootArgsRevision2 (bootFlags valid) requires Revision == 2, not just
    // Version - both fields gate different added struct members per
    // boot.h's comments.
    args.revision = K_BOOT_ARGS_REVISION2 as u16;

    let default_cmd = "-v serial=3 debug=0x14e";
    let eff_cmd = if command_line.is_empty() {
        default_cmd
    } else {
        command_line
    };
    let bytes = eff_cmd.as_bytes();
    let n = bytes.len().min(BOOT_LINE_LENGTH - 1);
    args.command_line[..n].copy_from_slice(&bytes[..n]);
    unsafe {
        let dt_slice = core::slice::from_raw_parts_mut(
            DEVICE_TREE_PA as *mut u8,
            DEVICE_TREE_STUB_LEN as usize,
        );
        // QEMU virt UART is at 0x0900_0000; on Superbird/Meson it would be its UART
        let pl011_base = 0x0900_0000;
        let dt_len = crate::adt::generate_vmapple_adt(
            dt_slice,
            crate::board::DRAM_BASE,
            crate::board::DRAM_SIZE,
            pl011_base,
        );
        args.device_tree_length = dt_len as u32;
        core::ptr::write(BOOT_ARGS_PA as *mut BootArgs, args);
    }
    BOOT_ARGS_PA
}

/// Invalidates (without cleaning) the data cache over a physical range, so
/// subsequent cacheable accesses miss to DRAM.
///
/// Deliberately `dc ivac`, never `dc civac`: every caller's range holds
/// data that reached DRAM through uncached writes, so any cache line still
/// covering it is stale garbage. Cleaning would write that garbage back
/// over the good data instead of discarding it.
///
/// # Safety
/// EL2 with the MMU off, so VA == PA and `start` is a physical address.
/// Discards any dirty lines in the range - the caller must be certain DRAM
/// holds the authoritative copy.
unsafe fn invalidate_dcache_range(start: u64, len: u64) {
    if len == 0 {
        return;
    }
    unsafe {
        // CTR_EL0.DminLine (bits 19:16) is log2 of the smallest data cache
        // line size in *words*, so the byte stride is 4 << DminLine. Using
        // the architectural minimum rather than assuming 64 bytes keeps the
        // loop correct if it ever runs on a core with narrower lines.
        let ctr: u64;
        core::arch::asm!("mrs {0}, ctr_el0", out(reg) ctr, options(nomem, nostack));
        let line = 4u64 << ((ctr >> 16) & 0xf);

        core::arch::asm!("dsb sy", options(nomem, nostack));
        let mut addr = start & !(line - 1);
        let end = start + len;
        while addr < end {
            core::arch::asm!("dc ivac, {0}", in(reg) addr, options(nomem, nostack));
            addr += line;
        }
        core::arch::asm!("dsb sy", "isb", options(nomem, nostack));
    }
}

/// Configures the EL1 guest context and `eret`s into it. Never returns.
///
/// HCR_EL2 here (as opposed to kernel-lib's `cpu::drop_to_el1`, which
/// this deliberately does NOT reuse) keeps stage-2 translation active
/// (VM=1, already pointed at the stage2.rs tables) and routes physical
/// IRQ/FIQ to EL2 (IMO=1, FMO=1) instead of letting them go straight to
/// the guest - both required for this to function as a hypervisor rather
/// than kernel-lib's own EL2->EL1 passthrough boot path.
///
/// # Safety
/// `entry_pa` must be a valid AArch64 instruction stream already present
/// at that physical address; `boot_args_pa` must point at a fully
/// constructed `boot_args`; stage-2 (`stage2::init`) and EL2 vectors
/// (`exceptions::init`) must already be set up.
pub unsafe fn enter_guest(entry_pa: u64, boot_args_pa: u64) -> ! {
    unsafe {
        // RW=1 (EL1 AArch64), VM=1 (stage-2 enabled), IMO=1/FMO=1 (route
        // physical IRQ/FIQ to EL2), HCD=0 (HVC from EL1 traps to EL2 - the
        // architectural default, spelled out here since hvc.rs's entire
        // premise depends on it).
        let hcr_el2: u64 = (1 << 31) | (1 << 0) | (1 << 4) | (1 << 3);
        core::arch::asm!("msr hcr_el2, {0}", in(reg) hcr_el2, options(nomem, nostack));
        core::arch::asm!("msr cptr_el2, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr cnthctl_el2, {0}", in(reg) 3u64, options(nomem, nostack));
        core::arch::asm!("msr cntvoff_el2, {0}", in(reg) 0u64, options(nomem, nostack));

        // Point the guest's VBAR_EL1 at the catch vectors in this image
        // (asm.rs) so exceptions the guest takes to EL1 are reported
        // instead of silently vanishing into the boot ROM's VBAR_EL1.
        // Legal because the guest runs MMU-off under an identity stage-2
        // map and so can execute this image at its physical address; the
        // guest overwrites this with its own table as soon as it installs
        // one.
        unsafe extern "C" {
            static el1_catch_vectors: u64;
        }
        let el1_vectors = core::ptr::addr_of!(el1_catch_vectors) as u64;
        core::arch::asm!("msr vbar_el1, {0}", in(reg) el1_vectors, options(nomem, nostack));
        crate::hv_println!("hv: guest VBAR_EL1 -> {:#x} (catch vectors)", el1_vectors);
        // Initialize EL1 control state to known-good values before the
        // `eret`. This is NOT a CPU reset - the Amlogic MaskROM/BL2 ran at
        // EL3 before us and leaves EL1 registers in an arbitrary state, so
        // none of them can be assumed zero. The one that matters is
        // SCTLR_EL1.M: if it is left set, the guest's very first
        // instruction fetch is stage-1 translated through a stale
        // TTBR0_EL1 that describes no valid tables, faults, and - because
        // HCR_EL2.TGE is 0 - is delivered to the guest's equally stale
        // VBAR_EL1 rather than to EL2. The result is a silent hang with no
        // diagnostic from this hypervisor at all, identical for every
        // guest image, which is exactly the failure this replaced.
        let (sctlr_el1, ttbr0_el1, tcr_el1, vbar_el1): (u64, u64, u64, u64);
        core::arch::asm!(
            "mrs {0}, sctlr_el1",
            "mrs {1}, ttbr0_el1",
            "mrs {2}, tcr_el1",
            "mrs {3}, vbar_el1",
            out(reg) sctlr_el1,
            out(reg) ttbr0_el1,
            out(reg) tcr_el1,
            out(reg) vbar_el1,
            options(nomem, nostack)
        );
        crate::hv_println!(
            "hv: inherited EL1 state sctlr={:#x} ttbr0={:#x} tcr={:#x} vbar={:#x}",
            sctlr_el1,
            ttbr0_el1,
            tcr_el1,
            vbar_el1
        );

        // SCTLR_EL1 with only the ARMv8.0 RES1 bits (11, 20, 22, 23, 28,
        // 29) set: MMU off (M=0), caches off (C=0/I=0), little-endian.
        // This is the same value Linux and kernel-lib use for an
        // MMU-disabled EL1. The guest turns its own MMU on later.
        const SCTLR_EL1_MMU_OFF: u64 = 0x30D0_0800;
        core::arch::asm!("msr sctlr_el1, {0}", in(reg) SCTLR_EL1_MMU_OFF, options(nomem, nostack));
        core::arch::asm!("msr ttbr0_el1, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr ttbr1_el1, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr tcr_el1, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr mair_el1, {0}", in(reg) 0u64, options(nomem, nostack));
        // Let EL1/EL0 use FP/SIMD without trapping (CPACR_EL1.FPEN=0b11);
        // XNU's early boot uses NEON long before it programs this itself.
        core::arch::asm!("msr cpacr_el1, {0}", in(reg) (3u64 << 20), options(nomem, nostack));
        // Boot-time instruction tracer (temporary diagnostic, see
        // exceptions.rs's SoftwareStepLowerEl arm): MDCR_EL2.TDE routes
        // the guest's debug exceptions - including the Software Step
        // exception armed below - to EL2 instead of the guest's own
        // VBAR_EL1, and MDSCR_EL1.SS arms single-step. Combined with
        // PSTATE.SS in spsr_el2 below, this traps back to EL2 once per
        // *retired guest instruction*, reporting its PC, until the trace
        // handler's step limit disarms it. Exists because this board has
        // no other way to see what a hung guest is actually doing: no
        // UART, no framebuffer wired into boot_args yet, and every prior
        // silent hang turned out to have a real cause once made visible.
        //
        // Left disarmed by default (`TRACE_SINGLE_STEP = false`): it did
        // its job (proved the guest executes real code - disassembly
        // confirmed XNU's own `start.s`, including a real `msr
        // OSLAR_EL1, xzr` OS-Lock-clear at the exact PC tracing appeared
        // to "stick" on) and found the real bug (a missing `elr_el2`
        // write elsewhere in this function, now fixed). Continued tracing
        // past the OS Lock unlock produced a PC that stopped advancing -
        // most likely the tracer interacting with the guest's own
        // debug-unlock sequence (OS Lock architecturally gates debug
        // exceptions), not a real guest hang. Flip this back to `true` to
        // resume tracing if a future hang needs the same treatment.
        const TRACE_SINGLE_STEP: bool = false;
        core::arch::asm!(
            "msr mdcr_el2, {0}",
            in(reg) if TRACE_SINGLE_STEP { 1u64 << 8 } else { 0 },
            options(nomem, nostack)
        ); // TDE
        core::arch::asm!(
            "msr mdscr_el1, {0}",
            in(reg) if TRACE_SINGLE_STEP { 1u64 } else { 0 },
            options(nomem, nostack)
        ); // SS

        // Drop any stale cached copies of the regions the guest is about to
        // read as Normal cacheable memory. Both were produced by uncached
        // writes: the image by the MaskROM's USB DRAM writes, the
        // boot_args/devicetree by this hypervisor's own stores with the MMU
        // off (Device-nGnRnE, straight to DRAM). Neither allocated a cache
        // line, so DRAM holds the only correct copy - while the caches may
        // still hold lines for these addresses left by a previous run under
        // `keep_power` (no CPU reset between launches).
        //
        // This must INVALIDATE and not clean-and-invalidate: `dc civac`
        // would write those stale lines back out over the freshly loaded
        // image, destroying it. `dc ivac` discards them so the guest's
        // first fetch/read misses to DRAM.
        invalidate_dcache_range(GUEST_ENTRY_PA, GUEST_IMAGE_LEN);
        invalidate_dcache_range(BOOT_ARGS_PA, core::mem::size_of::<BootArgs>() as u64);
        invalidate_dcache_range(DEVICE_TREE_PA, DEVICE_TREE_STUB_LEN as u64);

        // ...and any stale instruction-cache lines for the image, for the
        // same reason: the guest's entry point may still be cached from an
        // earlier attempt at this address.
        core::arch::asm!(
            "dsb sy",
            "ic ialluis",
            "dsb sy",
            "isb",
            options(nomem, nostack)
        );

        // EL1h, all exceptions masked until the guest's own early boot
        // code unmasks what it needs - mirrors kernel-lib's own
        // drop_to_el1 SPSR_EL2 choice, plus PSTATE.SS (bit 21) when the
        // instruction tracer above is armed for this initial entry.
        let spsr_el2: u64 = 0x3c5 | if TRACE_SINGLE_STEP { 1 << 21 } else { 0 };
        core::arch::asm!("msr spsr_el2, {0}", in(reg) spsr_el2, options(nomem, nostack));
        core::arch::asm!("msr elr_el2, {0}", in(reg) entry_pa, options(nomem, nostack));
        core::arch::asm!("dsb sy", "isb", options(nomem, nostack));
        core::arch::asm!(
            "mov x0, {boot_args}",
            "eret",
            boot_args = in(reg) boot_args_pa,
            options(nomem, nostack, noreturn)
        );
    }
}
