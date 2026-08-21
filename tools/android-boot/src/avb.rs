//! Appends a minimal, unsigned AVB footer + vbmeta blob to a boot image,
//! matching the on-disk structure `avbtool add_hash_footer` produces
//! (`external/avb/libavb/avb_footer.h`, `avb_vbmeta_image.h` upstream) -
//! by hand, since no `avbtool`/`avb` Python package is available in this
//! environment (see the investigation that led here).
//!
//! Confirmed necessary against real hardware: the device's stock
//! `boot.img` has this exact structure (`AVBf` footer as its last 64
//! bytes, pointing at an embedded `AVB0` vbmeta blob right after the
//! "real" image data) and boots fine via `fastboot boot`; a byte-
//! corrupted copy of that *same* stock image (breaking its hash, so
//! verification fails) still gets past ABL's initial parsing and
//! attempts to boot (hangs at the Google logo instead of resetting) -
//! proving an unlocked bootloader tolerates a bad/missing *signature*.
//! A from-scratch image with no AVB footer at all instead bounces back
//! to the fastboot menu within ~1-3s, every time, regardless of what the
//! payload itself is or does - the structure's mere *presence* appears
//! to be required independent of whether it actually verifies.
//!
//! `algorithm_type = 0` (NONE) here means no hash/signature block at
//! all - which is exactly what "unverified but structurally valid"
//! looks like to `libavb`, and is the standard representation for an
//! eng/unverified image (not a workaround unique to this tool).

const AVB_FOOTER_MAGIC: &[u8; 4] = b"AVBf";
const AVB_FOOTER_SIZE: usize = 64;

const AVB_VBMETA_MAGIC: &[u8; 4] = b"AVB0";
const AVB_VBMETA_HEADER_SIZE: usize = 256;

/// Builds a 256-byte `AvbVBMetaImageHeader` with `algorithm_type = NONE`
/// (0) and no auxiliary data (no descriptors, no embedded public key) -
/// the entire header is the vbmeta blob; there's no trailing
/// authentication/auxiliary data block to append since both of those
/// block sizes are 0. All multi-byte fields are big-endian, per AVB's
/// on-disk format (unlike the rest of this tool's little-endian
/// `bootimg_bindgen.rs` structs).
fn build_vbmeta() -> [u8; AVB_VBMETA_HEADER_SIZE] {
    let mut vbmeta = [0u8; AVB_VBMETA_HEADER_SIZE];
    vbmeta[0..4].copy_from_slice(AVB_VBMETA_MAGIC);
    vbmeta[4..8].copy_from_slice(&1u32.to_be_bytes()); // required_libavb_version_major
    vbmeta[8..12].copy_from_slice(&0u32.to_be_bytes()); // required_libavb_version_minor
    // authentication_data_block_size (12..20) = 0, auxiliary_data_block_size
    // (20..28) = 0, algorithm_type (28..32) = 0 (NONE), and every
    // offset/size field after that = 0 - all already correct from the
    // zero-initialized array.
    vbmeta
}

/// Builds the 64-byte `AvbFooter` describing `original_image_len` bytes
/// of real image data followed immediately by a vbmeta blob of
/// `vbmeta_len` bytes (no gap/alignment padding between them - `libavb`
/// only requires the footer to be readable at
/// `total_image_size - AVB_FOOTER_SIZE`, not any particular alignment of
/// what precedes it).
fn build_footer(original_image_len: u64, vbmeta_len: u64) -> [u8; AVB_FOOTER_SIZE] {
    let mut footer = [0u8; AVB_FOOTER_SIZE];
    footer[0..4].copy_from_slice(AVB_FOOTER_MAGIC);
    footer[4..8].copy_from_slice(&1u32.to_be_bytes()); // version_major
    footer[8..12].copy_from_slice(&0u32.to_be_bytes()); // version_minor
    footer[12..20].copy_from_slice(&original_image_len.to_be_bytes());
    footer[20..28].copy_from_slice(&original_image_len.to_be_bytes()); // vbmeta_offset
    footer[28..36].copy_from_slice(&vbmeta_len.to_be_bytes());
    // reserved[28] (36..64) stays zero.
    footer
}

/// Appends an unsigned vbmeta blob + AVB footer to `image` (a complete
/// boot image, e.g. from [`crate::mkbootimg::build_boot_image_v4`]),
/// returning the combined bytes ready to hand to `fastboot boot`.
pub fn append_footer(image: &[u8]) -> Vec<u8> {
    let vbmeta = build_vbmeta();
    let footer = build_footer(image.len() as u64, vbmeta.len() as u64);

    let mut out = Vec::with_capacity(image.len() + vbmeta.len() + footer.len());
    out.extend_from_slice(image);
    out.extend_from_slice(&vbmeta);
    out.extend_from_slice(&footer);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_is_readable_from_the_end_of_the_combined_image() {
        let image = vec![0xABu8; 100];
        let combined = append_footer(&image);

        let footer_start = combined.len() - AVB_FOOTER_SIZE;
        assert_eq!(&combined[footer_start..footer_start + 4], AVB_FOOTER_MAGIC);

        let original_image_size = u64::from_be_bytes(
            combined[footer_start + 12..footer_start + 20]
                .try_into()
                .unwrap(),
        );
        let vbmeta_offset = u64::from_be_bytes(
            combined[footer_start + 20..footer_start + 28]
                .try_into()
                .unwrap(),
        );
        let vbmeta_size = u64::from_be_bytes(
            combined[footer_start + 28..footer_start + 36]
                .try_into()
                .unwrap(),
        );

        assert_eq!(original_image_size, image.len() as u64);
        assert_eq!(vbmeta_offset, image.len() as u64);
        assert_eq!(
            &combined[vbmeta_offset as usize..(vbmeta_offset + 4) as usize],
            AVB_VBMETA_MAGIC
        );
        assert_eq!(vbmeta_size, AVB_VBMETA_HEADER_SIZE as u64);
        assert_eq!(vbmeta_offset + vbmeta_size, footer_start as u64);
    }
}
