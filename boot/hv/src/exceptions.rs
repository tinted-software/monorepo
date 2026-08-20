//! EL2 exception dispatch - the Rust side of vectors_el2.S's four
//! trampolines. Modeled on kernel-lib's `arch::aarch64::exceptions`, but
//! for traps taken *from* the guest (always EL1 here) rather than the
//! kernel's own self-exceptions.

use crate::context::{DataAbortIss, Ec, Frame};

#[unsafe(no_mangle)]
pub extern "C" fn handleSyncExceptionEl2(frame: &mut Frame) {
    match frame.ec() {
        Ec::SoftwareStepLowerEl => {
            // Boot-time instruction tracer: `enter_guest` arms
            // MDCR_EL2.TDE + MDSCR_EL1.SS + PSTATE.SS to route every
            // retired guest instruction here as a debug exception.
            // `frame.elr_el2` is the PC that just retired (same field
            // every other lower-EL exception uses). PSTATE.SS survives in
            // the saved `spsr_el2` untouched, so simply returning keeps
            // stepping - clearing bit 21 there is what turns it off.
            use core::sync::atomic::{AtomicU32, Ordering};
            const STEP_LIMIT: u32 = 2500;
            const SPSR_SS_BIT: u64 = 1 << 21;
            static STEPS: AtomicU32 = AtomicU32::new(0);
            let n = STEPS.fetch_add(1, Ordering::Relaxed);
            if n < STEP_LIMIT {
                crate::hv_println!("hv: step[{}] elr_el1={:#x}", n, frame.elr_el2);
            } else {
                crate::hv_println!("hv: step trace limit reached, disarming single-step");
                frame.spsr_el2 &= !SPSR_SS_BIT;
            }
            // Not a real instruction: do NOT advance elr_el2 - the
            // hardware's own step logic already points it at the next
            // instruction to execute.
        }
        Ec::Hvc64 => {
            crate::hvc::dispatch(frame);
            frame.elr_el2 = frame.elr_el2.wrapping_add(4);
        }
        Ec::DataAbortLowerEl => {
            let ipa = frame.fault_ipa();
            let iss = DataAbortIss::decode(frame.esr_el2);
            if !iss.isv {
                crate::hv_println!(
                    "hv: data abort with invalid syndrome at ipa={:#x} elr={:#x} - halting",
                    ipa,
                    frame.elr_el2
                );
                halt_forever();
            }
            let srt = iss.srt as usize;
            // x31 in the SRT field means the zero/discard register (XZR/
            // WZR), not a real GPR slot - never index into `frame.x` for
            // it, synthesize a scratch instead.
            let mut scratch: u64 = if srt == 31 { 0 } else { frame.x[srt] };
            // Same rationale as hvc.rs's trace: an emulated MMIO access is
            // handled silently, so without this a guest busily driving the
            // GIC looks exactly like a guest that never started. Bounded to
            // keep a polling loop from scrolling the panel.
            {
                use core::sync::atomic::{AtomicU32, Ordering};
                const TRACE_LIMIT: u32 = 24;
                static TRACED: AtomicU32 = AtomicU32::new(0);
                let n = TRACED.fetch_add(1, Ordering::Relaxed);
                if n < TRACE_LIMIT {
                    crate::hv_println!(
                        "hv: mmio[{}] {} ipa={:#x} elr={:#x}",
                        n,
                        if iss.write { "wr" } else { "rd" },
                        ipa,
                        frame.elr_el2
                    );
                } else if n == TRACE_LIMIT {
                    crate::hv_println!("hv: mmio trace limit reached, further faults silent");
                }
            }
            crate::stage2::handle_fault(ipa, iss, &mut scratch);
            if srt != 31 && !iss.write {
                frame.x[srt] = scratch;
            }
            frame.elr_el2 = frame.elr_el2.wrapping_add(4);
        }
        Ec::SysReg => {
            // See stage2.rs's module comment: reachability of this path on
            // hardware with no GICv3 CPU-interface system registers at all
            // is unverified. Surface it loudly rather than silently
            // guessing a return value if it does fire.
            crate::hv_println!(
                "hv: trapped MSR/MRS esr={:#x} elr={:#x} (unimplemented - ICC_* emulation not wired up)",
                frame.esr_el2,
                frame.elr_el2
            );
            halt_forever();
        }
        ec => {
            crate::hv_println!(
                "hv: unexpected sync exception ec={} ({:#x}) esr={:#x} elr={:#x} far={:#x}",
                ec.name(),
                (frame.esr_el2 >> 26) & 0x3f,
                frame.esr_el2,
                frame.elr_el2,
                frame.far_el2
            );
            halt_forever();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn handleIrqExceptionEl2(frame: &Frame) {
    // Physical IRQs are routed to EL2 (HCR_EL2.{IMO,FMO}=1, set in
    // boot.rs) so the vGIC layer can eventually inject them into whichever
    // guest core should see them. No injection path exists yet - this
    // just needs to not silently hang the whole board on the first timer
    // tick, so for now it's visible and inert.
    crate::hv_println!(
        "hv: unhandled IRQ at elr={:#x} (injection not implemented)",
        frame.elr_el2
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn handleFiqExceptionEl2(frame: &Frame) {
    crate::hv_println!("hv: unexpected FIQ at elr={:#x}", frame.elr_el2);
    halt_forever();
}

#[unsafe(no_mangle)]
pub extern "C" fn handleSErrorExceptionEl2(frame: &Frame) {
    crate::hv_println!(
        "hv: SError esr={:#x} elr={:#x} far={:#x}",
        frame.esr_el2,
        frame.elr_el2,
        frame.far_el2
    );
    halt_forever();
}

#[unsafe(no_mangle)]
pub extern "C" fn handleUnexpectedExceptionEl2(frame: &Frame) {
    crate::hv_println!(
        "hv: exception in an unhandled vector slot, elr={:#x}",
        frame.elr_el2
    );
    halt_forever();
}

fn halt_forever() -> ! {
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}

/// Installs `vector_table_el2` (vectors_el2.S) as VBAR_EL2. Must run once
/// per core before that core can safely take any EL2 exception (including
/// the HVC calls the guest issues almost immediately after `eret`).
pub fn init() {
    unsafe extern "C" {
        static vector_table_el2: u8;
    }
    let addr = core::ptr::addr_of!(vector_table_el2) as u64;
    unsafe {
        core::arch::asm!("msr vbar_el2, {0}", "isb", in(reg) addr, options(nomem, nostack));
    }
}
