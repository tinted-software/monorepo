//! The one piece of board-specific address knowledge every other
//! board-agnostic module (`stage2.rs`, `guest.rs`) needs: where DRAM
//! starts. Everything else - `context.rs`, `exceptions.rs`, `asm.rs`,
//! `hvc.rs`, `guest.rs`'s entry mechanics - has no board dependency at
//! all, which is what makes it possible to validate them against QEMU
//! (fast, deterministic, no USB/`keep_power` staleness, real GDB) before
//! trusting a result observed only on physical Superbird hardware.
//!
//! Superbird (Meson G12A): DRAM at physical `0x0`, per `linker.ld`'s
//! module comment. QEMU's `virt` machine: DRAM at `0x4000_0000` always,
//! regardless of `-m` size - a fixed property of the machine model, not
//! something `-m` changes.

#[cfg(feature = "qemu")]
pub const DRAM_BASE: u64 = 0x4000_0000;
#[cfg(not(feature = "qemu"))]
pub const DRAM_BASE: u64 = 0x0000_0000;

/// Matches both boards' actual test configuration: Superbird's real
/// 512MiB, and QEMU launched with `-m 512M` to match it exactly so the
/// two environments stay comparable.
pub const DRAM_SIZE: u64 = 0x2000_0000;
