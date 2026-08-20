//! Minimal PL011 UART driver, QEMU-`virt`-machine only.
//!
//! QEMU's `virt` machine always instantiates a PL011 at physical
//! `0x0900_0000` regardless of `-m`/`-cpu`/devicetree contents - a fixed
//! property of the machine model, same as `board::DRAM_BASE`. Exists
//! purely so this hypervisor's own diagnostics (`hv_println!`) can go to
//! `-serial stdio` instead of the physical Superbird panel, which QEMU
//! obviously has no equivalent of. Write-only: this hypervisor never
//! needs to read guest/operator input from it.

const UART_BASE: usize = 0x0900_0000;
const UARTDR: usize = 0x000; // Data register
const UARTFR: usize = 0x018; // Flag register
const UARTFR_TXFF: u32 = 1 << 5; // Transmit FIFO full

#[inline]
unsafe fn read32(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

#[inline]
unsafe fn write32(addr: usize, val: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, val) };
}

/// No-op: QEMU's PL011 model is already enabled and configured with sane
/// defaults (8N1, no flow control) the instant the machine starts - unlike
/// the Superbird panel, there's no hardware to adopt or probe for
/// presence. Kept as a function so `boot.rs`'s init call site reads the
/// same regardless of which console backend is active.
pub fn init() {}

pub fn putc(c: u8) {
    unsafe {
        while (read32(UART_BASE + UARTFR) & UARTFR_TXFF) != 0 {}
        write32(UART_BASE + UARTDR, c as u32);
    }
}

pub fn print(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}
