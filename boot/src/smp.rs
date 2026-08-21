//! Per-core "kicked" flags backing VMAPPLE_VCPU_KICK/VMAPPLE_VCPU_WFK
//! (hvc.rs). Distinct from `hv_smp_wake_flag` in start_el2.S, which is
//! the one-shot *boot-time* park release for secondary cores before any
//! guest exists; this is the guest's own runtime IPI-equivalent, used
//! after each core is already running its vCPU.
//!
//! WFE/SEV (not a real GIC SGI) is sufficient here: every core shares one
//! exclusive-monitor domain, so `sev` unblocks every `wfe`'d core
//! regardless of which one issued it, and the per-core flag below is what
//! disambiguates "was *this* core the one that got kicked" from a
//! spurious wakeup. Good enough for the single-core-guest target this
//! hypervisor is actually being built toward first; a real multi-vCPU
//! guest would want this to route through a GIC SGI instead so a kicked
//! core wakes even if it's blocked on something other than WFE.

use core::sync::atomic::{AtomicBool, Ordering};

const MAX_CORES: usize = 4;
static KICKED: [AtomicBool; MAX_CORES] = [const { AtomicBool::new(false) }; MAX_CORES];

pub fn kick_cpu(phys_id: u64) {
    if let Some(slot) = KICKED.get(phys_id as usize) {
        slot.store(true, Ordering::Release);
        unsafe { core::arch::asm!("sev", options(nomem, nostack)) };
    }
}

/// Blocks the calling core until `kick_cpu` targets it. `ien` mirrors
/// VMAPPLE_VCPU_WFK's second argument (whether the guest wants interrupts
/// left unmasked while waiting) - not yet honored: interrupts stay
/// whatever they already were, since nothing routes physical IRQs to a
/// parked guest core yet.
pub fn wait_for_kick(_ien: bool) {
    let id = crate::cpu::core_id() as usize;
    let Some(slot) = KICKED.get(id) else { return };
    while !slot.swap(false, Ordering::AcqRel) {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}
