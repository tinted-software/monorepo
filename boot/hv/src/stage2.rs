//! Stage-2 translation: identity-maps guest IPA -> real PA for DRAM and
//! most Amlogic MMIO, except the 2MiB region containing the real GIC-400
//! (GICD @ 0xffc0_1000 / GICC @ 0xffc0_2000 - see kernel-lib's
//! `drivers::gic` module doc comment), which is left unmapped so every
//! guest access to it takes a stage-2 data-abort trap to EL2 and gets
//! software-emulated instead (see `handle_fault` below).
//!
//! # Why the whole GIC block is trapped, not just the GICv3-only bits
//!
//! vmapple XNU's pexpert board header (`HAS_GIC_V3 = 1` in
//! pexpert/pexpert/arm64/VMAPPLE.h) drives it to program the interrupt
//! controller as GICv3: GICD's low register block (CTLR/IGROUPR/
//! ISENABLER/IPRIORITYR - offsets 0x000..0x7FC) is binary-compatible
//! between GICv2 and GICv3, but GICD_IROUTER (v3-only, offset 0x6000,
//! replacing GICv2's GICD_ITARGETSR) and the entire per-core GICR
//! redistributor region (no GICv2 equivalent at all - SGI/PPI enable
//! moves from GICD into GICR_ISENABLER0 in v3) are not. Since the
//! compatible and incompatible registers live in the same 2MiB-aligned
//! block as far as stage-2 block-granularity mapping is concerned, this
//! first cut traps the *entire* block uniformly and re-derives every
//! access in software against the real GIC-400 registers, rather than
//! passing through the compatible subset and only trapping the rest -
//! simpler to get right, at the cost of a trap per access instead of
//! only the ones that need one.
//!
//! # Open risk this does NOT cover
//!
//! GICD/GICR are architecturally plain load-store MMIO (confirmed against
//! xnu's own `pexpert/arm/pe_fiq.c`, which accesses them through
//! `volatile` pointers), so stage-2 data-abort trapping is the
//! architecturally-guaranteed mechanism (ARM ARM D8.7) regardless of
//! what CPU features the physical core implements - that part is solid.
//! Interrupt acknowledge/EOI (`ICC_IAR1_EL1`/`ICC_EOIR1_EL1`), by
//! contrast, are AArch64 *system registers*, not MMIO, and G12A's
//! Cortex-A73/A55 implement no GICv3 CPU-interface system registers at
//! all (`ID_AA64PFR0_EL1.GIC == 0`). Whether MSR/MRS to those
//! unallocated encodings traps to EL2 (routable via `Ec::SysReg` in
//! exceptions.rs, symmetric with this file) or is simply UNDEFINED at
//! the guest's own EL1 is genuinely unverified - it depends on ARM ARM
//! exception-routing rules this hypervisor hasn't been tested against
//! real hardware for yet. Do not assume `context.rs`'s `Ec::SysReg` case
//! is reachable until that's confirmed.

use crate::context::DataAbortIss;
use crate::gic;

unsafe extern "C" {
    static __stage2_l1_table: u64;
    static __stage2_dram_l2_table: u64;
    static __stage2_trap_l2_table: u64;
}

const GIB: u64 = 1 << 30;
const MIB2: u64 = 2 << 20;

/// Real GIC-400 register bases on Meson G12A (kernel-lib's
/// `drivers::gic::AMLOGIC_{DIST,CPU}_BASE`, restated here as the
/// authoritative addresses the fault handler below reads/writes).
const REAL_GICD_BASE: u64 = gic::AMLOGIC_DIST_BASE;
const REAL_GICC_BASE: u64 = gic::AMLOGIC_CPU_BASE;

/// Guest-visible (IPA) base of the trapped GIC block. Chosen equal to the
/// real physical address for now (identity IPA=PA everywhere else in this
/// map), so the synthetic devicetree (guest.rs) can just describe the
/// real address - the trap exists to translate *register semantics*
/// (v3 -> v2), not relocate the device.
const TRAPPED_GIC_BLOCK_IPA: u64 = REAL_GICD_BASE & !(MIB2 - 1);

// --- Stage-2 descriptor bits (ARM ARM D8.5, 4KiB granule) ---
const S2_VALID: u64 = 1 << 0;
const S2_TABLE: u64 = 1 << 1; // level 1: table vs block
const S2AP_RW: u64 = 0b11 << 6;
const S2_SH_INNER: u64 = 0b11 << 8;
const S2_AF: u64 = 1 << 10;
// MemAttr[3:0] for stage-2 (ARM ARM D8.5.4): 0b1111 = Normal, Inner/Outer
// Write-Back Cacheable; 0b0000 = Device-nGnRnE.
const S2_MEMATTR_NORMAL_WB: u64 = 0b1111 << 2;
const S2_MEMATTR_DEVICE: u64 = 0b0000 << 2;

#[inline]
fn block_desc(pa: u64, device: bool) -> u64 {
    let memattr = if device {
        S2_MEMATTR_DEVICE
    } else {
        S2_MEMATTR_NORMAL_WB
    };
    (pa & !((1u64 << 12) - 1)) | memattr | S2AP_RW | S2_SH_INNER | S2_AF | S2_VALID
}

/// Builds the stage-2 tables and points VTTBR_EL2 at them. Must run once,
/// before the first `eret` into the guest (boot.rs's `hvMain`) - the
/// guest's very first instruction fetch is already stage-2 translated.
///
/// # Safety
/// Must only be called once per boot, before stage-2 translation is
/// enabled (HCR_EL2.VM), and only from the primary core.
pub unsafe fn init() {
    let l1 = core::ptr::addr_of!(__stage2_l1_table) as *mut u64;
    let dram_l2 = core::ptr::addr_of!(__stage2_dram_l2_table) as *mut u64;
    let gic_l2 = core::ptr::addr_of!(__stage2_trap_l2_table) as *mut u64;

    unsafe {
        core::ptr::write_bytes(l1, 0, 512);
        core::ptr::write_bytes(dram_l2, 0, 512);
        core::ptr::write_bytes(gic_l2, 0, 512);

        // L1[0..4]: identity-map the whole 4GiB IPA space as 1GiB device
        // blocks by default (safe default for MMIO-heavy address space),
        // except the two slots that need finer-grained handling: whichever
        // slot contains `board::DRAM_BASE` (all of DRAM plus, on
        // Superbird, some low Amlogic MMIO sharing its gigabyte) must
        // descend to 2MiB blocks so DRAM can be Normal-WB - executing
        // guest code from a stage-2 Device mapping is UNPREDICTABLE per
        // the ARM ARM (not merely slow: this was an actual silent-hang
        // bug, not a performance TODO - the guest's first instruction
        // fetch at its Device-mapped entry point never completed) - and
        // whichever slot contains the trapped GIC block (see below).
        let dram_gib_index = crate::board::DRAM_BASE / GIB;
        let dram_gib_base = dram_gib_index * GIB;
        let dram_top = crate::board::DRAM_BASE + crate::board::DRAM_SIZE;
        for i in 0..4u64 {
            let ipa_base = i * GIB;
            if i == dram_gib_index {
                *l1.add(i as usize) = (dram_l2 as u64 & !((1u64 << 12) - 1)) | S2_TABLE | S2_VALID;
            } else if !cfg!(feature = "qemu") && ipa_base == TRAPPED_GIC_BLOCK_IPA & !(GIB - 1) {
                // On real Superbird, this 1GiB region contains the trapped GIC block - install
                // a table descriptor pointing at the L2 table instead of a block.
                // Under QEMU, GICv3 is natively provided by QEMU, so no trap table is installed.
                *l1.add(i as usize) = (gic_l2 as u64 & !((1u64 << 12) - 1)) | S2_TABLE | S2_VALID;
            } else {
                *l1.add(i as usize) = block_desc(ipa_base, /* device */ true);
            }
        }

        // L2 table for DRAM's containing gigabyte: identity 2MiB blocks,
        // Normal-WB within [DRAM_BASE, DRAM_BASE+DRAM_SIZE), Device for
        // any remainder of that gigabyte (on Superbird, low Amlogic MMIO
        // below DRAM_BASE's gigabyte boundary is impossible since
        // DRAM_BASE==0; kept general rather than assuming that).
        for i in 0..512u64 {
            let ipa = dram_gib_base + i * MIB2;
            let is_dram = ipa >= crate::board::DRAM_BASE && ipa < dram_top;
            *dram_l2.add(i as usize) = block_desc(ipa, /* device */ !is_dram);
        }

        // L2 table for the GIC-containing 1GiB region: identity 2MiB
        // device blocks everywhere except the one slot containing GICD/
        // GICC, which is left all-zero (invalid) so it faults.
        let gic_l1_base = TRAPPED_GIC_BLOCK_IPA & !(GIB - 1);
        for i in 0..512u64 {
            let ipa = gic_l1_base + i * MIB2;
            if ipa == TRAPPED_GIC_BLOCK_IPA {
                continue; // leave invalid: this is the trapped block
            }
            *gic_l2.add(i as usize) = block_desc(ipa, /* device */ true);
        }

        // VTCR_EL2: 4KiB granule (TG0=0b00), start at level 1 (SL0=1),
        // T0SZ=32 -> 4GiB IPA space, PS matches ID_AA64MMFR0_EL1.PARange.
        let parange: u64;
        core::arch::asm!("mrs {0}, id_aa64mmfr0_el1", out(reg) parange, options(nomem, nostack));
        let ps = parange & 0xf;
        let vtcr: u64 = (32u64) // T0SZ = 32 -> 4GiB IPA space
            | (0b01 << 6)  // SL0 = 1 (start walk at level 1, per ARM ARM D8.4.4)
            | (0b01 << 8)  // IRGN0 = WBWA
            | (0b01 << 10) // ORGN0 = WBWA
            | (0b11 << 12) // SH0 = inner shareable
            | (0b00 << 14) // TG0 = 4KiB
            | (ps << 16);
        core::arch::asm!("msr vtcr_el2, {0}", in(reg) vtcr, options(nomem, nostack));

        core::arch::asm!("msr vttbr_el2, {0}", in(reg) (l1 as u64), options(nomem, nostack));

        // Invalidate every stage-1 and stage-2 TLB entry for the EL1&0
        // regime before any of the above can be used. Required, not
        // defensive: the tables and VTCR/VTTBR just changed, and this
        // hypervisor is routinely re-launched by the MaskROM with
        // `keep_power` set, i.e. with no intervening CPU reset - so the TLB
        // can still hold translations built by a *previous* run of this
        // same code with a different memory-type layout. Without this, new
        // tables are silently shadowed by stale entries.
        core::arch::asm!(
            "dsb ishst",
            "tlbi vmalls12e1is",
            "dsb ish",
            "isb",
            options(nomem, nostack)
        );
    }
}

/// Emulates one trapped GICD/GICC access. `ipa` is the faulting IPA
/// (`Frame::fault_ipa`); `iss` decodes the syndrome (register/size/
/// direction). Reads/writes the real GIC-400 at the equivalent offset,
/// translating v3 register semantics to v2 where they differ.
///
/// Register value flows through `frame.x[srt]` by reference in
/// exceptions.rs's caller - this function only computes what belongs
/// there.
pub fn handle_fault(ipa: u64, iss: DataAbortIss, x_srt: &mut u64) {
    debug_assert!(
        iss.isv,
        "GIC MMIO trap without a valid syndrome - unexpected instruction form"
    );

    let off = ipa - TRAPPED_GIC_BLOCK_IPA;
    // GICD offsets 0x000..0x7FC: binary-compatible with GICv2, passthrough
    // 1:1 onto the real GICD.
    if off < 0x800 {
        mmio_passthrough(REAL_GICD_BASE + off, iss, x_srt);
        return;
    }

    // GICD_IROUTER (v3-only, offset 0x6000+): guest is telling the
    // controller which core should receive a given SPI. GIC-400 has no
    // per-core routing register - GICD_ITARGETSR (a 1-byte-per-IRQ CPU
    // target mask, offset 0x800 in both v2 and v3-compat layouts) is the
    // nearest equivalent. Route everything to core 0 for now (single-core
    // guest is this project's actual near-term target) rather than
    // attempting a real IROUTER<->ITARGETSR affinity translation.
    if (0x6000..0x8000).contains(&off) {
        if iss.write {
            let itargetsr_off = 0x800 + (off - 0x6000) / 8;
            unsafe { write_gicd_byte(itargetsr_off, 0b0001) }; // core 0
        } else {
            *x_srt = 0;
        }
        return;
    }

    // Everything from 0x1_0000 up is the per-core GICR redistributor
    // region (RD_base + SGI_base, GICR_PE_SIZE = 0x2_0000 apart per core
    // per VMAPPLE.h) - has no GIC-400 equivalent at all. SGI/PPI
    // enable/group/priority (GICR_ISENABLER0 etc, SGI_base+0x100.. per
    // VMAPPLE.h's GICR_ISENABLER0=0x10100) maps onto the *same* interrupt
    // numbers (0..31) as GICD_ISENABLER/IGROUPR/IPRIORITYR in GICv2 -
    // redirect there. GICR_WAKER (core power state handshake) and
    // GICR_TYPER (topology/last-in-list marker) have no real backing
    // state to read, so they're synthesized.
    if off >= 0x1_0000 {
        let sgi_off = off - 0x1_0000;
        match sgi_off {
            0x14 => {
                // GICR_WAKER: guest sets PROCESSORSLEEP then polls for
                // CHILDRENASLEEP to clear once it does the same. No real
                // sleep state to model - always report "awake".
                if !iss.write {
                    *x_srt = 0;
                }
            }
            0x08 => {
                // GICR_TYPER: bit[4] (Last) must be set on the final
                // redistributor in the list so the guest's discovery walk
                // terminates. Single-core target for now -> always last.
                if !iss.write {
                    *x_srt = 1 << 4;
                }
            }
            0x1_0080 => redirect_group(0, iss, x_srt), // GICR_IGROUPR0
            0x1_0100 => redirect_enable(0, iss, x_srt), // GICR_ISENABLER0
            _ => {
                if !iss.write {
                    *x_srt = 0;
                }
            }
        }
        return;
    }

    if !iss.write {
        *x_srt = 0;
    }
}

unsafe fn write_gicd_byte(off: u64, val: u8) {
    unsafe { core::ptr::write_volatile((REAL_GICD_BASE + off) as *mut u8, val) };
}

fn redirect_group(irq_word: u64, iss: DataAbortIss, x_srt: &mut u64) {
    mmio_passthrough(REAL_GICD_BASE + 0x080 + irq_word * 4, iss, x_srt);
}

fn redirect_enable(irq_word: u64, iss: DataAbortIss, x_srt: &mut u64) {
    mmio_passthrough(REAL_GICD_BASE + 0x100 + irq_word * 4, iss, x_srt);
}

fn mmio_passthrough(pa: u64, iss: DataAbortIss, x_srt: &mut u64) {
    unsafe {
        match iss.sas {
            2 => {
                if iss.write {
                    core::ptr::write_volatile(pa as *mut u32, *x_srt as u32);
                } else {
                    *x_srt = core::ptr::read_volatile(pa as *const u32) as u64;
                }
            }
            3 => {
                if iss.write {
                    core::ptr::write_volatile(pa as *mut u64, *x_srt);
                } else {
                    *x_srt = core::ptr::read_volatile(pa as *const u64);
                }
            }
            _ => {
                if !iss.write {
                    *x_srt = 0;
                }
            }
        }
    }
}

// GICC (0xffc0_2000) is deliberately unhandled above by offset (it falls
// in the same trapped 2MiB block, offset REAL_GICC_BASE - TRAPPED_GIC_BLOCK_IPA):
// interrupt ack/EOI on real GICv3-configured guests goes through ICC_*
// system registers, not this MMIO window, so nothing should ever actually
// reach GICC's offset via a stage-2 data abort. If it does, that's a
// signal the guest fell back to MMIO CPU-interface access (GICC_IAR/
// GICC_EOIR, offsets 0x00c/0x010 - same as GICv2), which real GIC-400
// supports directly - passthrough is correct there too, just not
// implemented as its own case yet.
