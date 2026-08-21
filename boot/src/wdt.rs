//! gs201's per-CPU-cluster hardware watchdogs (`google,gs201-cl0-wdt` /
//! `-cl1-wdt`, `google-modules/soc/gs/drivers/watchdog/s3c2410_wdt.c`
//! upstream). ABL leaves these **running** across the handoff to
//! whatever it boots next - the real Linux driver's own probe comment
//! says so outright: "if we're not enabling the watchdog, then ensure it
//! is disabled if it has been left running from the bootloader or other
//! source" (`s3c2410wdt_probe`).
//!
//! This hv is not that driver, and does nothing else this early, so
//! nothing else would ever service or disable it: left alone, ABL's
//! pre-armed timer fires a platform reset a few seconds after handoff -
//! regardless of what this image's own code does or doesn't touch,
//! which is exactly the "reboots to the fastboot menu ~1-3s after
//! `Boot command issued successfully`, identically with or without the
//! display MMIO probe" behavior observed on real hardware. Must run
//! as close to entry as possible, before anything else - the clock is
//! already running from ABL's handoff, not from this image's own start.

const WDT_CL0_BASE: u64 = 0x1006_0000;
const WDT_CL1_BASE: u64 = 0x1007_0000;

/// `S3C2410_WTCON` (offset 0x00): bit5 enable, bit2 interrupt-enable,
/// bit0 reset-enable. Writing 0 clears all three at once, fully
/// disarming the timer (matches `s3c2410wdt_stop`'s effect on real
/// Linux, minus the clock-gating/PMU-mask bookkeeping this hv has no
/// other use for).
const WTCON_OFFSET: u64 = 0x00;

/// Disables both cluster watchdogs. Safe to call with the MMU off
/// (identity physical access, same as every other early MMIO touch in
/// this crate) and, unlike the DPU_DMA display probe, the watchdog IP
/// is always-on/always-clocked by design (it has to be, to catch a
/// hung/uninitialized clock tree) - not gated behind the kind of
/// runtime-PM/power-domain setup the display path needs, so this is
/// safe to call unconditionally, first, before anything else.
pub fn disable_all() {
    unsafe {
        core::ptr::write_volatile(WDT_CL0_BASE as *mut u32, 0);
        core::ptr::write_volatile(WDT_CL1_BASE as *mut u32, 0);
    }
    let _ = WTCON_OFFSET; // offset is 0; kept named for documentation
}
