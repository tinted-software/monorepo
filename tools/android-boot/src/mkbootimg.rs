//! Builds Android boot images (header version 2) on the fly from a raw
//! kernel (ELF or flat binary) and an optional initrd/ramdisk, so `fastboot
//! boot` can be pointed directly at those artifacts instead of requiring a
//! pre-built `boot.img` (as produced by AOSP's `mkbootimg.py`).
//!
//! Field values and the `id` (SHA1 digest) computation mirror
//! `mkbootimg.py`'s defaults for maximum bootloader compatibility.

use crate::bootimg_bindgen::{BOOT_MAGIC, BOOT_MAGIC_SIZE, boot_img_hdr_v2, boot_img_hdr_v4};
use sha1::{Digest, Sha1};
use zerocopy::IntoBytes;

/// Default physical load addresses, matching AOSP `mkbootimg.py`'s
/// `--base`/`--kernel_offset`/`--ramdisk_offset`/`--tags_offset` defaults.
const DEFAULT_BASE: u32 = 0x1000_0000;
const DEFAULT_KERNEL_OFFSET: u32 = 0x0000_8000;
const DEFAULT_RAMDISK_OFFSET: u32 = 0x0100_0000;
const DEFAULT_TAGS_OFFSET: u32 = 0x0000_0100;
const DEFAULT_PAGE_SIZE: u32 = 2048;

/// Parameters controlling how the synthesized boot image is laid out.
/// Defaults match `mkbootimg.py`.
#[derive(Debug, Clone)]
pub struct BootImageParams {
    pub kernel_addr: u32,
    pub ramdisk_addr: u32,
    pub tags_addr: u32,
    pub page_size: u32,
    pub cmdline: String,
    pub os_version: u32,
}

impl Default for BootImageParams {
    fn default() -> Self {
        Self {
            kernel_addr: DEFAULT_BASE + DEFAULT_KERNEL_OFFSET,
            ramdisk_addr: DEFAULT_BASE + DEFAULT_RAMDISK_OFFSET,
            tags_addr: DEFAULT_BASE + DEFAULT_TAGS_OFFSET,
            page_size: DEFAULT_PAGE_SIZE,
            cmdline: String::new(),
            os_version: 0,
        }
    }
}

/// Returns `true` if `data` already looks like an Android boot image
/// (starts with the `ANDROID!` magic), in which case it should be sent to
/// the device as-is rather than re-wrapped.
pub fn is_boot_image(data: &[u8]) -> bool {
    let magic_size = BOOT_MAGIC_SIZE as usize;
    data.len() >= magic_size && data[..magic_size] == BOOT_MAGIC[..magic_size]
}

fn page_align(size: usize, page_size: u32) -> usize {
    let page_size = page_size as usize;
    size.div_ceil(page_size) * page_size
}

fn write_fixed(buf: &mut [u8], s: &[u8]) {
    let n = s.len().min(buf.len());
    buf[..n].copy_from_slice(&s[..n]);
}

/// Builds a header-version-2 Android boot image (single self-contained
/// image: no separate `vendor_boot` partition required) wrapping `kernel`
/// (an ELF or flat binary image - the bootloader is responsible for
/// interpreting its format) and an optional `ramdisk` (initrd).
pub fn build_boot_image(
    kernel: &[u8],
    ramdisk: Option<&[u8]>,
    params: &BootImageParams,
) -> Vec<u8> {
    let ramdisk = ramdisk.unwrap_or(&[]);
    let second: &[u8] = &[];
    let recovery_dtbo: &[u8] = &[];
    let dtb: &[u8] = &[];

    // `id` = SHA1 over each section + its length, matching mkbootimg.py's
    // `generate_id` for header_version == 2.
    let mut hasher = Sha1::new();
    hasher.update(kernel);
    hasher.update((kernel.len() as u32).to_le_bytes());
    hasher.update(ramdisk);
    hasher.update((ramdisk.len() as u32).to_le_bytes());
    hasher.update(second);
    hasher.update((second.len() as u32).to_le_bytes());
    hasher.update(recovery_dtbo);
    hasher.update((recovery_dtbo.len() as u32).to_le_bytes());
    hasher.update(dtb);
    hasher.update((dtb.len() as u32).to_le_bytes());
    let digest = hasher.finalize();

    let mut id = [0u32; 8];
    let id_bytes = id.as_mut_bytes();
    write_fixed(id_bytes, &digest);

    let mut cmdline = [0u8; 512];
    write_fixed(&mut cmdline, params.cmdline.as_bytes());

    let mut hdr = boot_img_hdr_v2::default();
    write_fixed(
        &mut hdr._base._base.magic,
        &BOOT_MAGIC[..BOOT_MAGIC_SIZE as usize],
    );
    hdr._base._base.kernel_size = kernel.len() as u32;
    hdr._base._base.kernel_addr = params.kernel_addr;
    hdr._base._base.ramdisk_size = ramdisk.len() as u32;
    hdr._base._base.ramdisk_addr = params.ramdisk_addr;
    hdr._base._base.second_size = 0;
    hdr._base._base.second_addr = 0;
    hdr._base._base.tags_addr = params.tags_addr;
    hdr._base._base.page_size = params.page_size;
    hdr._base._base.header_version = 2;
    hdr._base._base.os_version = params.os_version;
    hdr._base._base.cmdline = cmdline;
    hdr._base._base.id = id;
    hdr._base.recovery_dtbo_size = 0;
    hdr._base.recovery_dtbo_offset = 0;
    hdr._base.header_size = core::mem::size_of::<boot_img_hdr_v2>() as u32;
    hdr.dtb_size = 0;
    hdr.dtb_addr = 0;

    let page_size = params.page_size;
    let header_bytes = hdr.as_bytes();

    let mut out = Vec::with_capacity(
        page_align(header_bytes.len(), page_size)
            + page_align(kernel.len(), page_size)
            + page_align(ramdisk.len(), page_size),
    );

    out.extend_from_slice(header_bytes);
    out.resize(page_align(out.len(), page_size), 0);

    out.extend_from_slice(kernel);
    out.resize(page_align(out.len(), page_size), 0);

    out.extend_from_slice(ramdisk);
    out.resize(page_align(out.len(), page_size), 0);

    out
}

/// GKI (Generic Kernel Image) boot images always use this fixed page
/// size - unlike v0-v2, the v3/v4 header struct doesn't even have a
/// `page_size` field (see `bootimg_bindgen.rs`).
const GKI_PAGE_SIZE: u32 = 4096;

/// Builds a header-version-4 (GKI) Android boot image. Unlike
/// [`build_boot_image`] (header v2), there is no `kernel_addr`/
/// `ramdisk_addr`/`tags_addr` in this header at all - GKI bootloaders
/// (including the Pixel 7a / gs201's stock ABL, confirmed against its
/// shipped `boot.img`) instead read the kernel's own load address out of
/// an "arm64 Image" header (`text_offset`) embedded as the first 64
/// bytes of `kernel` itself - see `arm64_image.rs`, which `kernel` here
/// is expected to already be wrapped with.
///
/// `header_version` on real devices is a property of the *partition*,
/// not something `fastboot boot` gets to choose freely - it must match
/// what the currently-flashed `vendor_boot` (and the bootloader's own
/// expectations) already agree on, discovered by inspecting a real
/// `boot.img` for the target device rather than guessed.
pub fn build_boot_image_v4(
    kernel: &[u8],
    ramdisk: Option<&[u8]>,
    cmdline: &str,
    os_version: u32,
) -> Vec<u8> {
    let ramdisk = ramdisk.unwrap_or(&[]);

    let mut cmdline_fixed = [0u8; 1536]; // BOOT_ARGS_SIZE + BOOT_EXTRA_ARGS_SIZE
    write_fixed(&mut cmdline_fixed, cmdline.as_bytes());

    let mut hdr = boot_img_hdr_v4::default();
    write_fixed(
        &mut hdr._base.magic,
        &BOOT_MAGIC[..BOOT_MAGIC_SIZE as usize],
    );
    hdr._base.kernel_size = kernel.len() as u32;
    hdr._base.ramdisk_size = ramdisk.len() as u32;
    hdr._base.os_version = os_version;
    hdr._base.header_size = core::mem::size_of::<boot_img_hdr_v4>() as u32;
    hdr._base.reserved = [0; 4];
    hdr._base.header_version = 4;
    hdr._base.cmdline = cmdline_fixed;
    hdr.signature_size = 0;

    let header_bytes = hdr.as_bytes();

    let mut out = Vec::with_capacity(
        page_align(header_bytes.len(), GKI_PAGE_SIZE)
            + page_align(kernel.len(), GKI_PAGE_SIZE)
            + page_align(ramdisk.len(), GKI_PAGE_SIZE),
    );

    out.extend_from_slice(header_bytes);
    out.resize(page_align(out.len(), GKI_PAGE_SIZE), 0);

    out.extend_from_slice(kernel);
    out.resize(page_align(out.len(), GKI_PAGE_SIZE), 0);

    out.extend_from_slice(ramdisk);
    out.resize(page_align(out.len(), GKI_PAGE_SIZE), 0);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootimg::BootImage;

    #[test]
    fn detects_existing_boot_image() {
        assert!(!is_boot_image(b"ELF stuff not a boot image"));
        assert!(is_boot_image(b"ANDROID!rest-of-header-doesnt-matter"));
    }

    #[test]
    fn build_and_parse_roundtrip() {
        let kernel = vec![0xAAu8; 100_000];
        let ramdisk = vec![0xBBu8; 50_000];
        let mut params = BootImageParams::default();
        params.cmdline = "console=ttyS0".to_string();

        let image = build_boot_image(&kernel, Some(&ramdisk), &params);
        assert!(is_boot_image(&image));

        let parsed = BootImage::parse(&image[..]).expect("failed to parse built image");
        match parsed {
            BootImage::V2(hdr) => {
                assert_eq!({ hdr._base._base.kernel_size }, kernel.len() as u32);
                assert_eq!({ hdr._base._base.ramdisk_size }, ramdisk.len() as u32);
                assert_eq!({ hdr._base._base.kernel_addr }, params.kernel_addr);
                assert_eq!({ hdr._base._base.ramdisk_addr }, params.ramdisk_addr);
                assert_eq!({ hdr._base._base.header_version }, 2);
                assert_eq!({ hdr._base._base.page_size }, DEFAULT_PAGE_SIZE);
            }
            other => panic!("expected V2 header, got {other:?}"),
        }
    }

    #[test]
    fn build_without_ramdisk() {
        let kernel = vec![0x11u8; 4096];
        let params = BootImageParams::default();
        let image = build_boot_image(&kernel, None, &params);
        let parsed = BootImage::parse(&image[..]).expect("failed to parse");
        match parsed {
            BootImage::V2(hdr) => assert_eq!({ hdr._base._base.ramdisk_size }, 0),
            other => panic!("expected V2 header, got {other:?}"),
        }
    }
}
