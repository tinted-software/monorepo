//! Universal ARM Generic Interrupt Controller driver supporting both GICv2 and GICv3.
//!
//! - GICv2: used on QEMU `virt` machine (MMIO-mapped distributor + CPU interface).
//! - GIC-400 (GICv2-architecture): the real interrupt controller on Amlogic
//!   Meson G12A hardware (`meson-g12a.dtsi` `gic: interrupt-controller@ffc01000`,
//!   `compatible = "arm,gic-400"`) - MMIO distributor + MMIO CPU interface,
//!   *not* GICv3 system registers. GICD at `0xffc0_1000`, GICC at `0xffc0_2000`.
//! - GIC-v3: Pixel 7a / gs201 (Tensor G2)'s real interrupt controller -
//!   confirmed both from kernel DTS source and live on real hardware via
//!   `/proc/device-tree` (`interrupt-controller@10400000`, `compatible =
//!   "arm,gic-v3"`) - true GICv3 distributor + redistributor MMIO frames,
//!   so `init_v3`/`set_v3_bases` below apply directly, unlike Superbird.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const BOOTSTRAP_DIST_BASE: u64 = 0x0800_0000;
pub const BOOTSTRAP_CPU_BASE: u64 = 0x0801_0000;
pub const MMIO_LEN: u64 = 0x0002_0000;

/// Amlogic G12A GIC-400 distributor base (`meson-g12a.dtsi` `gic@ffc01000`).
pub const AMLOGIC_DIST_BASE: u64 = 0xffc0_1000;
/// Amlogic G12A GIC-400 CPU interface base.
pub const AMLOGIC_CPU_BASE: u64 = 0xffc0_2000;

/// gs201 (Tensor G2) GIC-v3 distributor base (`interrupt-controller@10400000`,
/// `reg = <0x10400000 0x10000>, <0x10440000 0x100000>` - confirmed both
/// from kernel DTS and live `/proc/device-tree` on real hardware).
pub const GS201_DIST_BASE: u64 = 0x1040_0000;
/// gs201 GIC-v3 first redistributor frame base - 8 frames of `0x2_0000`
/// each follow, one per CPU core (see `set_v3_bases`'s caller for how the
/// per-core redistributor is selected).
pub const GS201_REDIST_BASE: u64 = 0x1044_0000;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GicVersion {
    V2 = 2,
    V3 = 3,
}

// GICD register offsets (common & v2)
const GICD_CTLR: usize = 0x000;
const GICD_IGROUPR: usize = 0x080;
const GICD_ISENABLER: usize = 0x100;
const GICD_IPRIORITYR: usize = 0x400;
const GICD_ITARGETSR: usize = 0x800;
const GICD_IROUTER: usize = 0x6000;

// GICC register offsets (GICv2 CPU interface)
const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;

// GICR register offsets (GICv3 Redistributor)
const GICR_WAKER: usize = 0x0014;
const GICR_SGI_BASE_OFFSET: usize = 0x1_0000;
const GICR_IGROUPR0: usize = 0x0080;
const GICR_ISENABLER0: usize = 0x0100;
const GICR_IPRIORITYR: usize = 0x0400;

static GIC_VERSION: AtomicU32 = AtomicU32::new(GicVersion::V2 as u32);
static DIST_BASE: AtomicU64 = AtomicU64::new(BOOTSTRAP_DIST_BASE);
static CPU_OR_REDIST_BASE: AtomicU64 = AtomicU64::new(BOOTSTRAP_CPU_BASE);

pub fn set_version(ver: GicVersion) {
    GIC_VERSION.store(ver as u32, Ordering::Release);
}

pub fn version() -> GicVersion {
    if GIC_VERSION.load(Ordering::Acquire) == GicVersion::V3 as u32 {
        GicVersion::V3
    } else {
        GicVersion::V2
    }
}

pub fn set_bases(dist: u64, cpu: u64) {
    DIST_BASE.store(dist, Ordering::Release);
    CPU_OR_REDIST_BASE.store(cpu, Ordering::Release);
}

pub fn set_v3_bases(dist: u64, redist: u64) {
    set_version(GicVersion::V3);
    DIST_BASE.store(dist, Ordering::Release);
    CPU_OR_REDIST_BASE.store(redist, Ordering::Release);
}

/// Initializes the GIC distributor and local CPU interface according to detected version.
pub fn init() {
    match version() {
        GicVersion::V2 => init_v2(),
        GicVersion::V3 => init_v3(),
    }
}

fn init_v2() {
    let dist = DIST_BASE.load(Ordering::Acquire);
    let cpu = CPU_OR_REDIST_BASE.load(Ordering::Acquire);

    unsafe {
        // Enable distributor (Group 0 & Group 1)
        let gicd_ctlr = (dist + GICD_CTLR as u64) as *mut u32;
        core::ptr::write_volatile(gicd_ctlr, 3);

        // Set CPU interface Priority Mask Register to allow all priorities
        let gicc_pmr = (cpu + GICC_PMR as u64) as *mut u32;
        core::ptr::write_volatile(gicc_pmr, 0xff);

        // Enable CPU interface (Group 0 & Group 1)
        let gicc_ctlr = (cpu + GICC_CTLR as u64) as *mut u32;
        core::ptr::write_volatile(gicc_ctlr, 3);
    }
}

fn init_v3() {
    let dist = DIST_BASE.load(Ordering::Acquire);
    let redist = CPU_OR_REDIST_BASE.load(Ordering::Acquire);

    unsafe {
        // 1. Enable SRE (System Register Interface) at EL1
        let mut sre: u64;
        core::arch::asm!("mrs {0}, s3_0_c12_c12_5", out(reg) sre, options(nomem, nostack)); // ICC_SRE_EL1
        sre |= 0x1; // SRE bit
        core::arch::asm!("msr s3_0_c12_c12_5, {0}", in(reg) sre, options(nomem, nostack));
        core::arch::asm!("isb", options(nomem, nostack));

        // 2. Wake up Redistributor for CPU0
        let gicr_waker = (redist + GICR_WAKER as u64) as *mut u32;
        let mut waker = core::ptr::read_volatile(gicr_waker);
        waker &= !(1 << 1); // Clear ProcessorSleep
        core::ptr::write_volatile(gicr_waker, waker);

        // Poll until ChildrenAsleep (bit 2) is cleared
        let mut timeout = 100_000;
        while (core::ptr::read_volatile(gicr_waker) & (1 << 2)) != 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }

        // 3. Configure Distributor: Enable Group 0, Group 1 NS, and ARE (Affinity Routing)
        let gicd_ctlr = (dist + GICD_CTLR as u64) as *mut u32;
        // GICD_CTLR: ARE_NS (bit 4) | EnableGrp1NS (bit 1) | EnableGrp1A (bit 0)
        core::ptr::write_volatile(gicd_ctlr, 0x13);

        // 4. Set CPU interface Priority Mask to allow all priorities
        core::arch::asm!("msr s3_0_c4_c6_0, {0}", in(reg) 0xffu64, options(nomem, nostack)); // ICC_PMR_EL1

        // 5. Enable Group 1 interrupts on CPU interface
        core::arch::asm!("msr s3_0_c12_c12_7, {0}", in(reg) 1u64, options(nomem, nostack)); // ICC_IGRPEN1_EL1
        core::arch::asm!("isb", options(nomem, nostack));
    }
}

/// Enables the given interrupt ID.
pub fn enable(irq: u32) {
    match version() {
        GicVersion::V2 => enable_v2(irq),
        GicVersion::V3 => enable_v3(irq),
    }
}

fn enable_v2(irq: u32) {
    let dist = DIST_BASE.load(Ordering::Acquire);
    let reg_idx = (irq / 32) as usize;
    let bit_idx = (irq % 32) as u32;

    unsafe {
        let gicd_isenabler = (dist + (GICD_ISENABLER + reg_idx * 4) as u64) as *mut u32;
        core::ptr::write_volatile(gicd_isenabler, 1 << bit_idx);

        // Set target CPU for SPIs (IRQs >= 32)
        if irq >= 32 {
            let target_reg = (dist + (GICD_ITARGETSR + (irq as usize & !3)) as u64) as *mut u32;
            let shift = (irq % 4) * 8;
            let mut val = core::ptr::read_volatile(target_reg);
            val |= 0x01 << shift; // Route to CPU 0
            core::ptr::write_volatile(target_reg, val);
        }
    }
}

fn enable_v3(irq: u32) {
    let dist = DIST_BASE.load(Ordering::Acquire);
    let redist = CPU_OR_REDIST_BASE.load(Ordering::Acquire);

    unsafe {
        if irq < 32 {
            // SGI / PPI in Redistributor SGI frame (offset +0x10000)
            let sgi_base = redist + GICR_SGI_BASE_OFFSET as u64;

            // Set Group 1 NS
            let igroupr0 = (sgi_base + GICR_IGROUPR0 as u64) as *mut u32;
            let mut val = core::ptr::read_volatile(igroupr0);
            val |= 1 << irq;
            core::ptr::write_volatile(igroupr0, val);

            // Set priority
            let prio_reg = (sgi_base + (GICR_IPRIORITYR + (irq as usize)) as u64) as *mut u8;
            core::ptr::write_volatile(prio_reg, 0x80);

            // Enable interrupt
            let isenabler0 = (sgi_base + GICR_ISENABLER0 as u64) as *mut u32;
            core::ptr::write_volatile(isenabler0, 1 << irq);
        } else {
            // SPI in Distributor
            let reg_idx = (irq / 32) as usize;
            let bit_idx = (irq % 32) as u32;

            // Set Group 1 NS
            let igroupr = (dist + (GICD_IGROUPR + reg_idx * 4) as u64) as *mut u32;
            let mut val = core::ptr::read_volatile(igroupr);
            val |= 1 << bit_idx;
            core::ptr::write_volatile(igroupr, val);

            // Set priority
            let prio_reg = (dist + (GICD_IPRIORITYR + (irq as usize)) as u64) as *mut u8;
            core::ptr::write_volatile(prio_reg, 0x80);

            // Route to CPU 0 affinity (affinity index 0)
            let irouter = (dist + (GICD_IROUTER + (irq as usize - 32) * 8) as u64) as *mut u64;
            core::ptr::write_volatile(irouter, 0); // Aff0 = CPU0

            // Enable interrupt
            let isenabler = (dist + (GICD_ISENABLER + reg_idx * 4) as u64) as *mut u32;
            core::ptr::write_volatile(isenabler, 1 << bit_idx);
        }
    }
}

/// Acknowledges and claims the highest-priority pending interrupt.
pub fn claim() -> Option<u32> {
    match version() {
        GicVersion::V2 => claim_v2(),
        GicVersion::V3 => claim_v3(),
    }
}

fn claim_v2() -> Option<u32> {
    let cpu = CPU_OR_REDIST_BASE.load(Ordering::Acquire);
    unsafe {
        let gicc_iar = (cpu + GICC_IAR as u64) as *const u32;
        let iar = core::ptr::read_volatile(gicc_iar);
        let irq = iar & 0x3ff;
        if irq >= 1020 {
            None // Spurious
        } else {
            Some(irq)
        }
    }
}

fn claim_v3() -> Option<u32> {
    let iar: u64;
    unsafe {
        core::arch::asm!("mrs {0}, s3_0_c12_c12_0", out(reg) iar, options(nomem, nostack)); // ICC_IAR1_EL1
    }
    let irq = (iar & 0x00ff_ffff) as u32;
    if irq >= 1020 { None } else { Some(irq) }
}

/// Signals completion (End of Interrupt) for the acknowledged interrupt ID.
pub fn complete(irq: u32) {
    match version() {
        GicVersion::V2 => complete_v2(irq),
        GicVersion::V3 => complete_v3(irq),
    }
}

fn complete_v2(irq: u32) {
    let cpu = CPU_OR_REDIST_BASE.load(Ordering::Acquire);
    unsafe {
        let gicc_eoir = (cpu + GICC_EOIR as u64) as *mut u32;
        core::ptr::write_volatile(gicc_eoir, irq);
    }
}

fn complete_v3(irq: u32) {
    unsafe {
        core::arch::asm!("msr s3_0_c12_c12_1, {0}", in(reg) (irq as u64), options(nomem, nostack)); // ICC_EOIR1_EL1
        core::arch::asm!("isb", options(nomem, nostack));
    }
}
