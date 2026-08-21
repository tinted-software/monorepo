//! Wraps a flat kernel payload in the (Linux/EFI) "arm64 Image" header
//! (`Documentation/arch/arm64/booting.rst`) that GKI-era bootloaders
//! (header-version-4 `boot.img`, like the Pixel 7a / gs201's stock ABL)
//! expect the kernel section to start with - regardless of `boot.img`
//! header version, ARM64 Android bootloaders locate the *kernel's own*
//! load address by parsing this embedded header (`text_offset`/`flags`),
//! not from any `kernel_addr` field (header v3/v4 don't even have one -
//! see `bootimg_bindgen.rs`'s `boot_img_hdr_v3`/`v4`).
//!
//! Real GKI kernels are usually LZ4/gzip-compressed at this position
//! (magic bytes recognized by the bootloader's decompression probing);
//! this always produces the *uncompressed* form, which is the documented
//! fallback every such loader supports when no known compression magic
//! matches (mirrors how U-Boot's `booti`/Android's GKI boot flow treat an
//! unrecognized-but-`ARM\x64`-magic'd kernel section).

pub const ARM64_IMAGE_MAGIC: u32 = 0x644d_5241; // "ARM\x64", little-endian
const HEADER_LEN: u64 = 64;
const MAGIC_OFFSET: usize = 56;

/// True if `kernel` already starts with a valid arm64 Image header (magic
/// at byte offset 56) - e.g. a flattened ELF whose own `_start` already
/// bakes one in (see `boot/src/asm.rs`'s module comment), in which case
/// it should be used as-is rather than wrapped again: nesting two such
/// headers works (the bootloader only reads the outer one), but is
/// needless indirection and an extra way for the two `image_size`/
/// `text_offset` computations to disagree. Prefer whatever the kernel
/// image already declares about itself.
pub fn has_arm64_magic(kernel: &[u8]) -> bool {
    kernel.len() >= MAGIC_OFFSET + 4
        && u32::from_le_bytes(kernel[MAGIC_OFFSET..MAGIC_OFFSET + 4].try_into().unwrap())
            == ARM64_IMAGE_MAGIC
}

/// Wraps `kernel` (already a flat, position-dependent physical-memory
/// image - see `elf::flatten_elf`) so that, once the bootloader places
/// byte 0 of the *wrapped* image at `dram_base + text_offset` per the
/// arm64 Image protocol and jumps there, `kernel`'s own byte 0 ends up
/// loaded at exactly `link_addr` (`kernel`'s actual linked address) and
/// runs immediately: `text_offset` is chosen as `link_addr - dram_base -
/// HEADER_LEN`, and the header's `code0` field is a `B` instruction
/// branching over the header to `kernel`'s first instruction, so nothing
/// in `kernel` itself needs to change to accommodate the header being
/// prepended.
///
/// Uses arm64 Image protocol flags requesting placement "as close as
/// possible to the base of DRAM" (bits 1-2 == 0b00) rather than
/// "anywhere, bootloader's choice" (0b01) - required here since `kernel`
/// is a position-*dependent* image (absolute-addressed stack/bss/stage-2
/// table symbols baked in by the linker script), not a relocatable one.
pub fn wrap_arm64_image(kernel: &[u8], link_addr: u64, dram_base: u64) -> Vec<u8> {
    let text_offset = (link_addr - dram_base).wrapping_sub(HEADER_LEN);
    let image_size = HEADER_LEN + kernel.len() as u64;
    // bit0 = 0 (LE kernel), bits1-2 = 0b00 ("as close as possible to
    // base of DRAM"), bit3 = 0 (no 4K-only requirement).
    let flags: u64 = 0;

    // code0: `b #64` (skip the 64-byte header, landing on kernel's own
    // first instruction). B <imm26> encoding: 0x1400_0000 | (offset in
    // words). code1 is never executed (code0 always branches past it)
    // and is conventionally left zero.
    let code0: u32 = 0x1400_0000 | ((HEADER_LEN / 4) as u32 & 0x03ff_ffff);
    let code1: u32 = 0;

    let mut out = Vec::with_capacity(image_size as usize);
    out.extend_from_slice(&code0.to_le_bytes());
    out.extend_from_slice(&code1.to_le_bytes());
    out.extend_from_slice(&text_offset.to_le_bytes());
    out.extend_from_slice(&image_size.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes()); // res2
    out.extend_from_slice(&0u64.to_le_bytes()); // res3
    out.extend_from_slice(&0u64.to_le_bytes()); // res4
    out.extend_from_slice(&ARM64_IMAGE_MAGIC.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // res5 (PE header offset, unused)
    debug_assert_eq!(out.len() as u64, HEADER_LEN);

    out.extend_from_slice(kernel);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_64_bytes_and_starts_with_skip_branch() {
        let kernel = vec![0xAAu8; 32];
        let wrapped = wrap_arm64_image(&kernel, 0x8100_0000, 0x8000_0000);

        assert_eq!(wrapped.len(), 64 + kernel.len());
        // code0 == `b #64` == 0x14000010
        assert_eq!(&wrapped[0..4], &0x1400_0010u32.to_le_bytes());
        assert_eq!(&wrapped[56..60], &ARM64_IMAGE_MAGIC.to_le_bytes());
        // kernel's own bytes land immediately after the 64-byte header.
        assert_eq!(&wrapped[64..], &kernel[..]);
    }

    #[test]
    fn text_offset_places_kernel_at_link_addr() {
        let kernel = vec![0u8; 8];
        let link_addr = 0x8100_0000u64;
        let dram_base = 0x8000_0000u64;
        let wrapped = wrap_arm64_image(&kernel, link_addr, dram_base);

        let text_offset = u64::from_le_bytes(wrapped[8..16].try_into().unwrap());
        // dram_base + text_offset + HEADER_LEN must equal link_addr.
        assert_eq!(dram_base + text_offset + HEADER_LEN, link_addr);
    }
}
