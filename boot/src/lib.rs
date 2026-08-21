#![no_std]

pub mod adt;
pub mod asm;
pub mod board;
pub mod boot;
pub mod context;
mod cpu;
#[cfg(not(feature = "qemu"))]
mod display;
pub mod exceptions;
mod gic;
pub mod guest;
pub mod hvc;
mod mm;
#[cfg(feature = "qemu")]
mod pl011;
pub mod smp;
pub mod stage2;
#[cfg(feature = "gs201")]
pub mod wdt;

/// Diagnostic output for the hv itself, not the guest. On Superbird this
/// is the physical display panel (see `boot.rs`'s `hvMain` - a separate
/// adoption from whatever the guest itself may later draw to the same
/// panel), since UART is physically unreachable there; under the `qemu`
/// feature it's QEMU's fixed PL011 instead, so this hypervisor's own
/// boot log is instant `-serial stdio` text rather than panel round-trips
/// - see `board.rs`'s module comment for why that pairing exists at all.
pub(crate) struct DisplayWriter;

impl core::fmt::Write for DisplayWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        #[cfg(not(feature = "qemu"))]
        display::print(s);
        #[cfg(feature = "qemu")]
        pl011::print(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! hv_println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut w = $crate::DisplayWriter;
        let _ = writeln!(w, $($arg)*);
    }};
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    #[cfg(not(feature = "qemu"))]
    display::print_panic("");
    let mut w = DisplayWriter;
    if let Some(loc) = info.location() {
        let _ = write!(w, "hv panic {}:{}\n", loc.file(), loc.line());
    }
    let _ = write!(w, "{}\n", info.message());
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}
