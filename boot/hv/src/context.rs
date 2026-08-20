//! Saved register frame pushed by vectors_el2.S's SAVE_CONTEXT macro onto a
//! trap taken *to* EL2 (always from the EL1 guest here - see that file's
//! module comment). Field layout must match the assembly macro exactly;
//! keep the two in lockstep by hand.

/// ESR_EL2 Exception Class values relevant to guest trap handling. Full
/// list in the ARM ARM (DDI 0487), section D17.2.37 ("ESR_ELx, Exception
/// Syndrome Register"); only the subset this hypervisor currently routes
/// is named.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ec {
    /// Trapped WFI/WFE - unused today (WFx left untrapped), named for
    /// `ec_name` completeness.
    Wfx,
    /// HVC instruction executed at EL1 (AArch64) - the vmapple hypercall
    /// path (VMAPPLE_PAC_*, VMAPPLE_GET_MABS_OFFSET, VMAPPLE_VCPU_*, and
    /// the SMCCC-style UID/REVISION/FEATURES discovery calls). See
    /// hvc.rs.
    Hvc64,
    /// Trapped MSR/MRS/system instruction. Reserved for ICC_* (GICv3 CPU
    /// interface) system-register emulation - NOT wired up yet. Whether
    /// accesses to system-register encodings the physical core doesn't
    /// implement (G12A's Cortex-A73/A55 have no GICv3 CPU interface at
    /// all) actually route here rather than straight to the guest's own
    /// EL1 Unknown-reason vector is unverified; see stage2.rs's module
    /// comment for why GICD/GICR MMIO emulation doesn't share this
    /// uncertainty.
    SysReg,
    /// Data abort taken from a lower EL - stage-2 translation/permission
    /// fault. Drives the GICD/GICR MMIO trap-and-emulate path
    /// (stage2.rs) via HPFAR_EL2 for the faulting IPA.
    DataAbortLowerEl,
    /// Software Step debug exception taken from a lower EL (EC 0x32) -
    /// fires once per retired guest instruction when `enter_guest` arms
    /// `MDCR_EL2.TDE`/`MDSCR_EL1.SS`/`PSTATE.SS` for the boot-time
    /// instruction tracer. `frame.elr_el2` is the guest PC that just
    /// retired, exactly as for any other lower-EL exception.
    SoftwareStepLowerEl,
    Other(u32),
}

impl Ec {
    pub fn decode(esr_el2: u64) -> Self {
        match (esr_el2 >> 26) & 0x3f {
            0x01 => Ec::Wfx,
            0x16 => Ec::Hvc64,
            0x18 => Ec::SysReg,
            0x24 => Ec::DataAbortLowerEl,
            0x32 => Ec::SoftwareStepLowerEl,
            other => Ec::Other(other as u32),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ec::Wfx => "WFI/WFE",
            Ec::Hvc64 => "HVC (AArch64)",
            Ec::SysReg => "MSR/MRS/system instruction",
            Ec::DataAbortLowerEl => "data abort (lower EL)",
            Ec::SoftwareStepLowerEl => "software step (lower EL)",
            Ec::Other(_) => "other",
        }
    }
}

/// ISS field of a Data Abort ESR (ESR_EL2.EC == 0x24/0x25) - see ARM ARM
/// D17.2.37, table for "Data Abort". Only the fields stage2.rs's MMIO
/// emulation needs are decoded.
#[derive(Debug, Clone, Copy)]
pub struct DataAbortIss {
    /// Access size: 0=byte, 1=halfword, 2=word, 3=doubleword.
    pub sas: u8,
    /// Register transferred, x0..x30 (SRT).
    pub srt: u8,
    /// Write (true) vs read (false) access.
    pub write: bool,
    /// Whether the syndrome is valid (ISV) - if false, the emulator must
    /// decode the trapped instruction itself instead of trusting `sas`/
    /// `srt`. All GICD/GICR accesses this hypervisor cares about are
    /// plain `ldr`/`str` with regular encodings, which the architecture
    /// guarantees produce a valid syndrome, so an invalid one here means
    /// something unexpected touched the trapped IPA range.
    pub isv: bool,
}

impl DataAbortIss {
    pub fn decode(esr_el2: u64) -> Self {
        let iss = esr_el2 as u32;
        Self {
            isv: (iss >> 24) & 1 != 0,
            sas: ((iss >> 22) & 0b11) as u8,
            srt: ((iss >> 16) & 0b1_1111) as u8,
            write: (iss >> 6) & 1 != 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub x: [u64; 31], // x0..x30 (x[30] is LR)
    pub sp_el0: u64,
    pub elr_el2: u64,
    pub spsr_el2: u64,
    pub esr_el2: u64,
    pub far_el2: u64,
    /// Faulting IPA (bits [51:12], plus [15:12] as the low bits when the
    /// faulting VA isn't page-aligned) for stage-2 aborts. Only valid
    /// when `Ec::decode(esr_el2) == Ec::DataAbortLowerEl`.
    pub hpfar_el2: u64,
    _pad: u64,
    pub q: [u128; 32],
}

const _FRAME_SIZE_MATCHES_ASM: () = assert!(core::mem::size_of::<Frame>() == 816);

impl Frame {
    #[inline]
    pub fn ec(&self) -> Ec {
        Ec::decode(self.esr_el2)
    }

    /// Faulting IPA reconstructed from HPFAR_EL2 (bits [39:4], giving
    /// IPA[47:12]) and FAR_EL2's page-offset bits (ARM ARM D17.2.35).
    #[inline]
    pub fn fault_ipa(&self) -> u64 {
        ((self.hpfar_el2 & 0xf_ffff_ffff_f0) << 8) | (self.far_el2 & 0xfff)
    }

    /// x0 register, the sole in/out operand for every VMAPPLE_* hypercall
    /// this hypervisor answers (hvc.rs) - reads the function ID before
    /// dispatch, then gets overwritten with the return value.
    #[inline]
    pub fn hvc_arg(&self, n: usize) -> u64 {
        self.x[n]
    }

    #[inline]
    pub fn set_hvc_ret(&mut self, n: usize, v: u64) {
        self.x[n] = v;
    }
}
