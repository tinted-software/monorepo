//! Rust entry points called from `start_el2.S` once each core has a
//! stack and (primary core only) .bss is zeroed. Mirrors kernel-lib's own
//! `kmain`/`secondaryMain` split.

#[cfg(not(feature = "qemu"))]
use crate::display;

#[unsafe(no_mangle)]
pub extern "C" fn hvMain(_uboot_dtb_phys: u64) -> ! {
    // Disable gs201's per-cluster hardware watchdogs FIRST, before
    // anything else at all - see wdt.rs's module doc comment. ABL
    // leaves them running across handoff with a short (bootloader-
    // configured, not the 30s Linux later requests) timeout as an
    // anti-brick safety net; nothing else in this image would ever
    // service or disable them otherwise, so left alone they fire a
    // platform reset a few seconds after entry regardless of what any
    // other code here does or doesn't touch - which matches exactly
    // what was observed even with the display MMIO probe compiled out
    // entirely (`gs201_probe`). The clock is already running from ABL's
    // handoff, not from this image's own start, so every instruction
    // before this one was pure overhead against that budget.
    #[cfg(feature = "gs201")]
    crate::wdt::disable_all();

    // Install the EL2 exception vector table FIRST, before anything else
    // - in particular before `display::init_early()`'s hardware register
    // probe below. On gs201 that probe reads the DPU_DMA/RDMA block at
    // 0x1C0B_0000, which (unlike Superbird's VPU/canvas block) may sit
    // behind a power/clock domain that ABL doesn't leave enabled for an
    // arbitrary `fastboot boot`-ed image (Linux's own exynos DRM driver
    // does its own `clk_prepare_enable`/power-domain `get_sync` before
    // ever touching this register block - see the display/gs201.rs doc
    // comment). A read from a clock-gated register in that state can
    // raise a synchronous/external abort at the AXI/bus level; with no
    // VBAR_EL2 installed yet that goes to whatever garbage the boot ROM
    // left there, which cascades into undefined behavior and, empirically,
    // a platform reset a couple seconds later - a data point, not a
    // certainty, since there's no serial console to confirm it directly.
    // - in particular before `display::init_early()`'s hardware register
    // probe below. On gs201 that probe reads the DPU_DMA/RDMA block at
    // 0x1C0B_0000, which (unlike Superbird's VPU/canvas block) may sit
    // behind a power/clock domain that ABL doesn't leave enabled for an
    // arbitrary `fastboot boot`-ed image (Linux's own exynos DRM driver
    // does its own `clk_prepare_enable`/power-domain `get_sync` before
    // ever touching this register block - see the display/gs201.rs doc
    // comment). A read from a clock-gated register in that state can
    // raise a synchronous/external abort at the AXI/bus level; with no
    // VBAR_EL2 installed yet that goes to whatever garbage the boot ROM
    // left there, which cascades into undefined behavior and, empirically,
    // a platform reset a couple seconds later - a data point, not a
    // certainty, since there's no serial console to confirm it directly.
    crate::exceptions::init();

    // Read CurrentEL before EL2-only work. Pixel ABL's `fastboot boot`
    // handoff is not guaranteed to be EL2 (pKVM may keep EL2). EL3 can
    // write all the `*_el2` registers this file goes on to program, so a
    // mis-assumed EL3 handoff prints a successful-looking boot log and
    // then silently fails at `eret` (SPSR_EL3/ELR_EL3, not EL2).
    let current_el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }

    // Same panel-adoption call the native kernel's `display::init_early`
    // makes when it detects it's running on Meson/gs201 hardware (see
    // that function, and `display/mod.rs`'s board-submodule dispatch) -
    // done unconditionally here since this image only ever targets one
    // physical board per build (boot/linker*.ld), unlike kernel-lib
    // which also targets QEMU virt. Under the `qemu` feature there is no
    // panel to adopt; `pl011::init` is a no-op (see that module's doc
    // comment). Skipped entirely under `gs201_probe` (see that feature's
    // use in boards.bzl) so a bisection build can confirm/rule out this
    // MMIO probe as the reboot cause independent of everything else.
    #[cfg(all(not(feature = "qemu"), not(feature = "gs201_probe")))]
    display::init_early();
    #[cfg(feature = "qemu")]
    crate::pl011::init();

    crate::hv_println!(
        "opendarwin hv: resident on core 0 at EL{}",
        (current_el >> 2) & 0b11
    );
    crate::hv_println!("hv: exception vectors installed");

    if (current_el >> 2) & 0b11 != 2 {
        crate::hv_println!("hv: not EL2 - display-only, skipping stage-2/guest");
        loop {
            unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
        }
    }

    unsafe { crate::stage2::init() };
    crate::hv_println!("hv: stage-2 tables built, VTTBR_EL2 armed");

    let boot_args_pa = unsafe {
        crate::guest::build_boot_args(
            "-v serial=3 debug=0x14e -enable_kprintf_spam serial-device=0x100 serial-device-name=uart0",
        )
    };
    crate::hv_println!("hv: boot_args staged at {:#x}", boot_args_pa);

    // `tools/boot_xnu.sh` writes the guest Mach-O to GUEST_ENTRY_PA over
    // USB before starting this image; nothing else populates that
    // address, so a magic-number check is the only way this hypervisor
    // can tell a real loaded kernel apart from whatever cold-boot DRAM
    // happened to contain (see guest.rs's MACHO_MAGIC_64 doc comment).
    let magic = unsafe { core::ptr::read_volatile(crate::guest::GUEST_ENTRY_PA as *const u32) };
    if magic != crate::guest::MACHO_MAGIC_64 {
        crate::hv_println!(
            "hv: no guest image loaded at {:#x} yet (magic {:#010x}) - halting",
            crate::guest::GUEST_ENTRY_PA,
            magic
        );
        loop {
            unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
        }
    }

    crate::hv_println!(
        "hv: entering guest at {:#x} (boot_args {:#x})",
        crate::guest::GUEST_ENTRY_POINT_PA,
        boot_args_pa
    );
    unsafe { crate::guest::enter_guest(crate::guest::GUEST_ENTRY_POINT_PA, boot_args_pa) }
}

#[unsafe(no_mangle)]
pub extern "C" fn hvSecondaryMain(_core_id: u64) -> ! {
    crate::exceptions::init();
    // Secondary cores wait to be kicked by the guest running on core 0
    // (smp.rs) before there's anything for them to do - core 0 hasn't
    // even entered a guest yet (see hvMain), so this just parks.
    crate::smp::wait_for_kick(true);
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}
