//! VMAPPLE hypercall (HVC) service handler.
//!
//! Implements the exact ABI vmapple XNU's early boot path and PAC-key
//! setup issue via `hvc #0` (osfmk/arm64/{hv_hvc.h,arm64_hypercall.c} in
//! the xnu-12377.121.6 tree at /Volumes/Dev/xnu) - the calling convention,
//! function-ID encoding, and UUID byte-packing below are transcribed
//! directly from those two files, not guessed. The Apple CPU/OEM service
//! UID/REVISION/FEATURES discovery calls follow ARM's SMCCC-style
//! convention (ARM DEN 0028F) that `hvg_is_hcall_available` probes before
//! trusting any of the real OEM calls.
//!
//! Calling convention (see arm64_hypercall.c's hvc_5/hvc32_4): the
//! function ID goes in x0 (or w0 for the 32-bit discovery calls), up to
//! four more arguments in x1..x4, and the *same* registers carry the
//! return value(s) - x0 (or w0) doubles as a signed success/failure code
//! (`>= 0` success) for every call except PAC_* (which the guest spins
//! forever on unless x0 comes back exactly 0 - see pac_asm.h/
//! machine_routines_asm.s's `cbnz x0, .`).

use crate::context::Frame;

const HVC_FID_FAST_CALL: u64 = 0x8000_0000;
const HVC_FID_HVC64: u64 = 0x4000_0000;
const HVC_FID_CPU: u64 = 0x0100_0000;
const HVC_FID_OEM: u64 = 0x0300_0000;

const HVC_FID_UID: u64 = 0xff01;
const HVC_FID_REVISION: u64 = 0xff03;
const HVC_FID_FEATURES: u64 = 0xfeff;

const HVC_CPU_SERVICE: u64 = HVC_FID_FAST_CALL | HVC_FID_HVC64 | HVC_FID_CPU;
const HVC_OEM_SERVICE: u64 = HVC_FID_FAST_CALL | HVC_FID_HVC64 | HVC_FID_OEM;
/// 32-bit (no HVC64 bit) discovery calls - hvg_get_uid/_revision/_features
/// use `hvc32_4`/`hvc32_2`, not `hvc_5`, so these never set HVC_FID_HVC64.
const HVC32_CPU_DISCOVERY: u64 = HVC_FID_FAST_CALL | HVC_FID_CPU;
const HVC32_OEM_DISCOVERY: u64 = HVC_FID_FAST_CALL | HVC_FID_OEM;

const VMAPPLE_PAC_SET_INITIAL_STATE: u64 = HVC_CPU_SERVICE | 0x0;
const VMAPPLE_PAC_GET_DEFAULT_KEYS: u64 = HVC_CPU_SERVICE | 0x1;
const VMAPPLE_PAC_SET_A_KEYS: u64 = HVC_CPU_SERVICE | 0x2;
const VMAPPLE_PAC_SET_B_KEYS: u64 = HVC_CPU_SERVICE | 0x3;
const VMAPPLE_PAC_SET_EL0_DIVERSIFIER: u64 = HVC_CPU_SERVICE | 0x4;
const VMAPPLE_PAC_SET_EL0_DIVERSIFIER_AT_EL1: u64 = HVC_CPU_SERVICE | 0x5;
const VMAPPLE_PAC_SET_G_KEY: u64 = HVC_CPU_SERVICE | 0x6;
const VMAPPLE_PAC_NOP: u64 = HVC_CPU_SERVICE | 0xf0;

const VMAPPLE_GET_MABS_OFFSET: u64 = HVC_OEM_SERVICE | 0x3;
const VMAPPLE_GET_BOOTSESSIONUUID: u64 = HVC_OEM_SERVICE | 0x4;
const VMAPPLE_VCPU_WFK: u64 = HVC_OEM_SERVICE | 0x5;
const VMAPPLE_VCPU_KICK: u64 = HVC_OEM_SERVICE | 0x6;

const CPU_UID_QUERY: u64 = HVC32_CPU_DISCOVERY | HVC_FID_UID;
const CPU_REVISION_QUERY: u64 = HVC32_CPU_DISCOVERY | HVC_FID_REVISION;
const CPU_FEATURES_QUERY: u64 = HVC32_CPU_DISCOVERY | HVC_FID_FEATURES;
const OEM_UID_QUERY: u64 = HVC32_OEM_DISCOVERY | HVC_FID_UID;
const OEM_REVISION_QUERY: u64 = HVC32_OEM_DISCOVERY | HVC_FID_REVISION;
const OEM_FEATURES_QUERY: u64 = HVC32_OEM_DISCOVERY | HVC_FID_FEATURES;

const HVC32_OEM_MAJOR_VER: u64 = 1;
const HVC32_OEM_MINOR_VER: u64 = 0;

/// `VMAPPLE_HVC_UID` ("3B878185-AA62-4E1F-9DC9-D6799CBB6EBB") packed into
/// the four little-endian 32-bit words `regs_to_uuid` in
/// arm64_hypercall.c expects back from a UID query, i.e. the exact
/// inverse of that function: `uuid[15 - i*4 - j] = (reg[i] >> j*8) & 0xff`
/// for i,j in 0..4. Solved by hand against the fixed UUID string; not
/// derived at runtime since it never changes.
const VMAPPLE_HVC_UID_REGS: [u32; 4] = [0x9CBB_6EBB, 0x9DC9_D679, 0xAA62_4E1F, 0x3B87_8185];

/// Placeholder boot-session UUID returned from VMAPPLE_GET_BOOTSESSIONUUID
/// - xnu only threads this through as an opaque session identifier
/// (diagnostics/telemetry correlation on the real vmapple host), nothing
/// in the boot path re-derives or validates it, so any stable value is
/// fine here.
const BOOT_SESSION_UUID_REGS: [u32; 4] = [0x0000_0001, 0x0000_0000, 0x0000_0000, 0x0000_0000];

/// Dispatches one HVC trap. `frame` is the saved guest register state;
/// this function reads the function ID from x0 and overwrites the
/// relevant x0..x4 with the call's return value(s) in place, mirroring
/// exactly what arm64_hypercall.c's `hvc_5`/`hvc32_4` expect to read back
/// after the real `hvc` instruction.
pub fn dispatch(frame: &mut Frame) {
    let fid = frame.hvc_arg(0);

    // Trace every call, not just unhandled ones. The guest kernel has no
    // console on this board - boot_args carries no framebuffer and the SoC
    // has no reachable UART - so hypercalls are the *only* evidence that it
    // is executing at all, and a handled call is otherwise indistinguishable
    // from a hang. Bounded so a per-context-switch call (PAC key
    // reprogramming) can't scroll the earlier boot log off the panel.
    {
        use core::sync::atomic::{AtomicU32, Ordering};
        const TRACE_LIMIT: u32 = 32;
        static TRACED: AtomicU32 = AtomicU32::new(0);
        let n = TRACED.fetch_add(1, Ordering::Relaxed);
        if n < TRACE_LIMIT {
            crate::hv_println!("hv: hvc[{}] fid={:#x} elr={:#x}", n, fid, frame.elr_el2);
        } else if n == TRACE_LIMIT {
            crate::hv_println!("hv: hvc trace limit reached, further calls silent");
        }
    }

    match fid {
        // PAC key programming: real Apple Silicon under Virtualization.
        // framework traps these to the host because a VM guest can't be
        // allowed to program its own PAC keys directly (they're a host
        // security boundary). Here there is no hardware PAC (FEAT_PAuth)
        // on G12A's Cortex-A73/A55 at all, so `pacia`/`autia`/... execute
        // as architectural NOPs in the guest regardless of whether we
        // "really" program anything - success (x0 == 0, checked via
        // `cbnz x0, .` in the guest, not merely `>= 0`) is all that's
        // required to keep boot moving.
        VMAPPLE_PAC_SET_INITIAL_STATE
        | VMAPPLE_PAC_SET_A_KEYS
        | VMAPPLE_PAC_SET_B_KEYS
        | VMAPPLE_PAC_SET_EL0_DIVERSIFIER
        | VMAPPLE_PAC_SET_EL0_DIVERSIFIER_AT_EL1
        | VMAPPLE_PAC_SET_G_KEY => {
            frame.set_hvc_ret(0, 0);
        }
        VMAPPLE_PAC_GET_DEFAULT_KEYS => {
            // Real call returns the host's default APIAKey/APIBKey/etc
            // pairs in x1..x4; zero keys are architecturally valid (just
            // not securely random) and sufficient since PAC isn't real
            // hardware here anyway.
            frame.set_hvc_ret(0, 0);
            frame.set_hvc_ret(1, 0);
            frame.set_hvc_ret(2, 0);
        }
        VMAPPLE_PAC_NOP => {
            // hvg_is_hcall_available's probe: must come back != 0xffff_ffff
            // and non-negative (as i64). 0 satisfies both.
            frame.set_hvc_ret(0, 0);
        }

        VMAPPLE_GET_MABS_OFFSET => {
            // x1 in: guest's own ml_get_abstime_offset() (its notion of
            // when mach_absolute_time's counter should read zero); x1
            // out: offset to apply. No cross-VM time sync exists here
            // (single physical timeline, no nested host), so 0 leaves
            // the guest's own value untouched.
            frame.set_hvc_ret(0, 0);
            frame.set_hvc_ret(1, 0);
        }
        VMAPPLE_GET_BOOTSESSIONUUID => {
            frame.set_hvc_ret(0, 0);
            for (i, w) in BOOT_SESSION_UUID_REGS.iter().enumerate() {
                frame.set_hvc_ret(1 + i, *w as u64);
            }
        }
        VMAPPLE_VCPU_KICK => {
            let phys_id = frame.hvc_arg(1);
            crate::smp::kick_cpu(phys_id);
            frame.set_hvc_ret(0, 0);
        }
        VMAPPLE_VCPU_WFK => {
            let ien = frame.hvc_arg(1) != 0;
            crate::smp::wait_for_kick(ien);
            frame.set_hvc_ret(0, 0);
        }

        // SMCCC-style discovery calls (ARM DEN 0028F): hvg_is_hcall_available
        // walks UID -> REVISION -> FEATURES for whichever OEM hypercall
        // it's about to rely on. Only the OEM range is ever actually
        // exercised by vmapple XNU's boot path (hvg_hcall_get_mabs_offset
        // et al are all OEM); the CPU-range discovery constants are
        // answered identically for completeness/symmetry, since nothing
        // in xnu-12377 queries them before a PAC_* call - those are
        // issued unconditionally, not probed first.
        OEM_UID_QUERY | CPU_UID_QUERY => {
            for (i, w) in VMAPPLE_HVC_UID_REGS.iter().enumerate() {
                frame.set_hvc_ret(i, *w as u64);
            }
        }
        OEM_REVISION_QUERY | CPU_REVISION_QUERY => {
            frame.set_hvc_ret(0, HVC32_OEM_MAJOR_VER);
            frame.set_hvc_ret(1, HVC32_OEM_MINOR_VER);
        }
        OEM_FEATURES_QUERY | CPU_FEATURES_QUERY => {
            // w1 in carries the fast-call ID being probed; every OEM call
            // this hypervisor implements above is "supported" (features
            // bitmask 0, non-negative x0).
            frame.set_hvc_ret(0, 0);
        }

        other => {
            crate::hv_println!(
                "hv: unhandled HVC fid={:#x} (x1={:#x})",
                other,
                frame.hvc_arg(1)
            );
            // Match hvg_is_hcall_available's "unknown hypercall" contract
            // (pre-Sydro host behavior it explicitly still handles):
            // 0xffff_ffff in x0.
            frame.set_hvc_ret(0, 0xffff_ffff);
        }
    }
}
