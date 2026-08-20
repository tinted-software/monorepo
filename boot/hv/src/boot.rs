//! Rust entry points called from `start_el2.S` once each core has a
//! stack and (primary core only) .bss is zeroed. Mirrors kernel-lib's own
//! `kmain`/`secondaryMain` split.

#[cfg(not(feature = "qemu"))]
use crate::display;

#[unsafe(no_mangle)]
pub extern "C" fn hvMain(_uboot_dtb_phys: u64) -> ! {
    // Same panel-adoption call the native kernel's `display::init_early`
    // makes when it detects it's running on Meson hardware (see that
    // function) - done unconditionally here since this image only ever
    // targets Superbird (boot/linker.ld), unlike kernel-lib which also
    // targets QEMU virt. Under the `qemu` feature there is no panel to
    // adopt; `pl011::init` is a no-op (see that module's doc comment).
    #[cfg(not(feature = "qemu"))]
    display::init_amlogic_vpu(display::VPU_BASE, display::CANVAS_BASE);
    #[cfg(feature = "qemu")]
    crate::pl011::init();

    // Read CurrentEL before touching anything else. The MaskROM's
    // `REQ_RUN_IN_ADDR` handoff makes no documented guarantee about which
    // exception level it drops to, and the difference is invisible in
    // every other diagnostic: EL3 can write all the `*_el2` registers
    // this file goes on to program, so a mis-assumed EL3 handoff prints
    // an entirely successful-looking boot log and then silently fails at
    // `eret` - which consumes SPSR_EL3/ELR_EL3, not the SPSR_EL2/ELR_EL2
    // `guest::enter_guest` sets, and so branches to whatever ELR_EL3
    // happened to hold.
    let current_el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }
    crate::hv_println!(
        "opendarwin hv: resident on core 0 at EL{}",
        (current_el >> 2) & 0b11
    );

    crate::exceptions::init();
    crate::hv_println!("hv: VBAR_EL2 installed");

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
