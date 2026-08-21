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
//! something `-m` changes. Pixel 7a / gs201 (Tensor G2): DRAM at
//! `0x8000_0000` - confirmed live via `/proc/device-tree/memory@80000000`
//! on real hardware (see this crate's port-investigation notes) - plus
//! several more discontiguous banks above 32GiB (`0x8_8000_0000`,
//! `0x9_0000_0000`, `0x9_8000_0000`) that this single-bank `DRAM_BASE`/
//! `DRAM_SIZE` model doesn't represent; fine for now since display-only
//! bring-up only needs the first bank.

#[cfg(feature = "qemu")]
pub const DRAM_BASE: u64 = 0x4000_0000;
#[cfg(feature = "gs201")]
pub const DRAM_BASE: u64 = 0x8000_0000;
#[cfg(not(any(feature = "qemu", feature = "gs201")))]
pub const DRAM_BASE: u64 = 0x0000_0000;

/// Matches Superbird's/QEMU's actual test configuration: Superbird's real
/// 512MiB, and QEMU launched with `-m 512M` to match it exactly so the
/// two environments stay comparable. gs201's first bank's exact size is
/// still TBD (SELinux blocks reading `reg` from an unrooted shell - see
/// port-investigation notes); 512MiB is a conservative placeholder,
/// comfortably smaller than any real Tensor G2 DRAM bank.
pub const DRAM_SIZE: u64 = 0x2000_0000;
