//! Only what `cpu.rs`'s backtrace bounds-check needs from the native
//! kernel's full `mm::mmu` module (`ref/src/kernel/src/mm/mmu.rs`, ~700
//! lines of dynamic stage-1 table management): this hypervisor never
//! enables its own EL2 stage-1 translation. It stays MMU-off and
//! physically addressed throughout - a minimal Type-1 stub has no dynamic
//! address space of its own to manage, unlike the native kernel which
//! juggles per-task tables. Guest-facing translation is entirely separate
//! (stage-2, in `stage2.rs`).
//!
//! `boot/linker.ld` gives exact bounds for this image (`__image_start`/
//! `__image_end`, spanning .text/.rodata/.data/.bss plus the boot and
//! secondary-core stacks and the stage-2 tables), so unlike the native
//! kernel's `KERNEL_IMAGE_MAX_LEN` - a fixed guess used because that
//! image's actual linked size can vary by build config - the backtrace
//! walker here can bound against the real linked extent directly.

unsafe extern "C" {
    static __image_start: u8;
    static __image_end: u8;
}

/// Lowest valid address in this image's static VA range (== PA range,
/// since the MMU is never enabled).
pub fn image_start() -> u64 {
    core::ptr::addr_of!(__image_start) as u64
}

/// One past the highest valid address in this image's static range.
pub fn image_end() -> u64 {
    core::ptr::addr_of!(__image_end) as u64
}
