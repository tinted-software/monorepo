//! Low-level CPU register, Exception Level, and transition operations.

use aarch64_cpu::registers::{MPIDR_EL1, Readable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionLevel {
    El0,
    El1,
    El2,
    El3,
}

#[inline(always)]
pub fn current_el() -> ExceptionLevel {
    let el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) el, options(nomem, nostack));
    }
    match (el >> 2) & 0x3 {
        0 => ExceptionLevel::El0,
        1 => ExceptionLevel::El1,
        2 => ExceptionLevel::El2,
        3 => ExceptionLevel::El3,
        _ => ExceptionLevel::El0,
    }
}

pub const SCTLR_EL1_RESET: u64 = 0x30d0_0800;
pub const SCTLR_EL1_MMU_ENABLED: u64 = SCTLR_EL1_RESET | 1 | (1 << 2) | (1 << 12);

pub unsafe fn drop_to_el1(continuation: usize, sp_el1: usize, dtb: usize, sctlr_el1: u64) -> ! {
    unsafe {
        core::arch::asm!("msr hcr_el2, {0}", in(reg) (1u64 << 31), options(nomem, nostack));
        core::arch::asm!("msr cptr_el2, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr cnthctl_el2, {0}", in(reg) 3u64, options(nomem, nostack));
        core::arch::asm!("msr cntvoff_el2, {0}", in(reg) 0u64, options(nomem, nostack));
        core::arch::asm!("msr vttbr_el2, {0}", in(reg) 0u64, options(nomem, nostack));

        let pfr0: u64;
        core::arch::asm!("mrs {0}, id_aa64pfr0_el1", out(reg) pfr0, options(nomem, nostack));
        if (pfr0 >> 24) & 0xf != 0 {
            core::arch::asm!("msr s3_4_c12_c9_5, {0}", in(reg) 9u64, options(nomem, nostack));
        }

        core::arch::asm!("msr sctlr_el1, {0}", in(reg) sctlr_el1, options(nomem, nostack));
        core::arch::asm!("msr spsr_el2, {0}", in(reg) 0x3c5u64, options(nomem, nostack));
        core::arch::asm!("msr elr_el2, {0}", in(reg) (continuation as u64), options(nomem, nostack));
        core::arch::asm!("msr sp_el1, {0}", in(reg) (sp_el1 as u64), options(nomem, nostack));

        core::arch::asm!("dsb sy", "isb", options(nomem, nostack));
        core::arch::asm!(
            "mov x0, {dtb}",
            "eret",
            dtb = in(reg) dtb,
            options(nomem, nostack, noreturn)
        );
    }
}

#[inline(always)]
pub fn core_id() -> u64 {
    MPIDR_EL1.get() & 0xff
}

#[inline(always)]
pub fn unmask_irq() {
    unsafe {
        core::arch::asm!("msr daifclr, #2", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn mask_irq() {
    unsafe {
        core::arch::asm!("msr daifset, #2", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn wfe() {
    unsafe {
        core::arch::asm!("wfe", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn sev() {
    unsafe {
        core::arch::asm!("sev", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn dsb_ish() {
    unsafe {
        core::arch::asm!("dsb ish", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb", options(nomem, nostack));
    }
}

/// Walks the AArch64 frame-pointer chain starting at `fp` (x29), calling
/// `emit` with each return address found.
///
/// Relies on the standard `stp x29, x30, [sp, #-N]!` / `mov x29, sp`
/// prologue (guaranteed by `-Cforce-frame-pointers=yes` for this kernel
/// target): `*fp` is the caller's saved x29, `*(fp+8)` is the return
/// address into the caller. Frames only ever grow toward higher addresses
/// as the chain is walked (the stack grows down), so a non-increasing next
/// frame pointer means a corrupted chain - stop rather than risk a fault
/// while already handling one.
pub fn walk_frames(fp: u64, max_frames: usize, mut emit: impl FnMut(u64)) {
    // Bound every dereferenced address against the kernel image's static
    // VA range (where every stack - boot, secondary-core, and this whole
    // image's .bss - lives): a corrupted/garbage `fp` (e.g. read mid-fault,
    // before the interrupted frame finished its prologue) must never be
    // dereferenced, or walking the backtrace becomes a second fault that
    // masks the one actually being reported.
    let lo = crate::mm::image_start();
    let hi = crate::mm::image_end();
    let in_bounds = |addr: u64| addr >= lo && addr <= hi.saturating_sub(16);

    let mut fp = fp;
    for _ in 0..max_frames {
        if fp == 0 || fp & 0xf != 0 || !in_bounds(fp) {
            break;
        }
        let prev_fp = unsafe { core::ptr::read_volatile(fp as *const u64) };
        let ret = unsafe { core::ptr::read_volatile((fp + 8) as *const u64) };
        if ret == 0 {
            break;
        }
        emit(ret);
        if prev_fp <= fp {
            break;
        }
        fp = prev_fp;
    }
}

/// Reads the current frame pointer (x29), for backtraces started from a
/// point (like the panic handler) with no saved [`Frame`].
#[inline(always)]
pub fn current_fp() -> u64 {
    let fp: u64;
    unsafe {
        core::arch::asm!("mov {0}, x29", out(reg) fp, options(nomem, nostack));
    }
    fp
}
