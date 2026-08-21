//! Minimal, dependency-free ELF64 program-header reader.
//!
//! `rust_binary`/`bazel build`-produced hv images (`*_raw` Bazel targets)
//! are full ELF executables, not flat binaries. Bootloaders like ABL/LK
//! don't parse ELF - they DMA the bytes handed to them straight into RAM
//! at a fixed physical address (the boot image header's `kernel_addr`)
//! and jump to the very first byte. Handing an ELF file to `fastboot
//! boot` as-is means the bootloader executes the `\x7fELF...` header
//! bytes as AArch64 instructions, which crashes within the first few
//! cycles - indistinguishable, from the fastboot/USB side, from a normal
//! boot command that was "issued successfully" followed by the device
//! immediately watchdog-resetting back to the bootloader.
//!
//! `flatten_elf` performs the same transformation as `llvm-objcopy -O
//! binary`: concatenates each `PT_LOAD` segment's file contents at its
//! `p_paddr`-relative offset into one contiguous buffer (zero-filling any
//! `p_memsz > p_filesz` bss tail), and reports the lowest `p_paddr` as
//! the address that first byte of the flattened image must be loaded at
//! - i.e. the boot image header's `kernel_addr`, derived automatically
//! instead of needing to be hand-supplied (and easy to get out of sync
//! with a board's linker script) on every `fastboot boot` invocation.

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const PT_LOAD: u32 = 1;

pub struct FlatImage {
    pub bytes: Vec<u8>,
    /// Physical address the first byte of `bytes` must be loaded at.
    pub load_addr: u64,
    /// Entry point (informational only - the bootloader always jumps to
    /// `load_addr`, so this must equal `load_addr` for `_start` to
    /// actually run; mismatches are a linker-script bug, not something
    /// this tool can fix, but worth surfacing).
    pub entry: u64,
}

pub fn is_elf(data: &[u8]) -> bool {
    data.len() >= 4 && data[..4] == ELF_MAGIC
}

fn u16_at(data: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(off..off + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(off..off + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], off: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(off..off + 8)?.try_into().ok()?))
}

/// Flattens an ELF64/little-endian executable's `PT_LOAD` segments into a
/// single contiguous physical-memory image, mirroring `objcopy -O binary`.
pub fn flatten_elf(data: &[u8]) -> Result<FlatImage, String> {
    if !is_elf(data) {
        return Err("missing ELF magic".to_string());
    }
    if data.len() < 64 {
        return Err("truncated ELF header".to_string());
    }
    if data[4] != ELFCLASS64 {
        return Err("only ELF64 is supported (got 32-bit ELF)".to_string());
    }
    if data[5] != ELFDATA2LSB {
        return Err("only little-endian ELF is supported".to_string());
    }

    let e_entry = u64_at(data, 24).ok_or("truncated e_entry")?;
    let e_phoff = u64_at(data, 32).ok_or("truncated e_phoff")? as usize;
    let e_phentsize = u16_at(data, 54).ok_or("truncated e_phentsize")? as usize;
    let e_phnum = u16_at(data, 56).ok_or("truncated e_phnum")? as usize;

    struct Load {
        offset: usize,
        filesz: usize,
        paddr: u64,
        memsz: usize,
    }

    let mut loads = Vec::new();
    for i in 0..e_phnum {
        let ph_off = e_phoff + i * e_phentsize;
        let p_type = u32_at(data, ph_off).ok_or("truncated program header")?;
        if p_type != PT_LOAD {
            continue;
        }
        let p_offset = u64_at(data, ph_off + 8).ok_or("truncated p_offset")? as usize;
        let p_paddr = u64_at(data, ph_off + 24).ok_or("truncated p_paddr")?;
        let p_filesz = u64_at(data, ph_off + 32).ok_or("truncated p_filesz")? as usize;
        let p_memsz = u64_at(data, ph_off + 40).ok_or("truncated p_memsz")? as usize;

        if data.len() < p_offset + p_filesz {
            return Err(format!(
                "PT_LOAD segment file range [{p_offset:#x}, {:#x}) exceeds file size {:#x}",
                p_offset + p_filesz,
                data.len()
            ));
        }

        loads.push(Load {
            offset: p_offset,
            filesz: p_filesz,
            paddr: p_paddr,
            memsz: p_memsz,
        });
    }

    if loads.is_empty() {
        return Err("no PT_LOAD segments found".to_string());
    }

    let load_addr = loads.iter().map(|l| l.paddr).min().unwrap();
    let end = loads
        .iter()
        .map(|l| l.paddr + l.memsz as u64)
        .max()
        .unwrap();
    if end < load_addr {
        return Err("PT_LOAD segment address range overflowed/inverted".to_string());
    }

    let mut bytes = vec![0u8; (end - load_addr) as usize];
    for l in &loads {
        let dst_off = (l.paddr - load_addr) as usize;
        bytes[dst_off..dst_off + l.filesz].copy_from_slice(&data[l.offset..l.offset + l.filesz]);
        // memsz > filesz (bss) is left zero-filled, matching objcopy -O binary.
    }

    Ok(FlatImage {
        bytes,
        load_addr,
        entry: e_entry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16le(v: u16) -> [u8; 2] {
        v.to_le_bytes()
    }
    fn u32le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }
    fn u64le(v: u64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// Builds a minimal synthetic ELF64 LE executable with one PT_LOAD
    /// segment (code) plus one bss-only PT_LOAD segment, entry == the
    /// code segment's paddr, for `flatten_elf` to chew on.
    fn build_test_elf() -> Vec<u8> {
        const EHSIZE: usize = 64;
        const PHENTSIZE: usize = 56;
        let code = [0xAAu8; 16];
        let code_paddr: u64 = 0x8100_0000;
        let code_offset: usize = EHSIZE + 2 * PHENTSIZE;

        let bss_paddr: u64 = code_paddr + 0x1000;
        let bss_memsz: u64 = 0x100;

        let mut elf = vec![0u8; code_offset + code.len()];
        elf[..4].copy_from_slice(&ELF_MAGIC);
        elf[4] = ELFCLASS64;
        elf[5] = ELFDATA2LSB;
        elf[24..32].copy_from_slice(&u64le(code_paddr)); // e_entry
        elf[32..40].copy_from_slice(&u64le(EHSIZE as u64)); // e_phoff
        elf[54..56].copy_from_slice(&u16le(PHENTSIZE as u16)); // e_phentsize
        elf[56..58].copy_from_slice(&u16le(2)); // e_phnum

        // Program header 0: code, PT_LOAD
        let ph0 = EHSIZE;
        elf[ph0..ph0 + 4].copy_from_slice(&u32le(PT_LOAD));
        elf[ph0 + 8..ph0 + 16].copy_from_slice(&u64le(code_offset as u64)); // p_offset
        elf[ph0 + 24..ph0 + 32].copy_from_slice(&u64le(code_paddr)); // p_paddr
        elf[ph0 + 32..ph0 + 40].copy_from_slice(&u64le(code.len() as u64)); // p_filesz
        elf[ph0 + 40..ph0 + 48].copy_from_slice(&u64le(code.len() as u64)); // p_memsz

        // Program header 1: bss, PT_LOAD, filesz 0
        let ph1 = EHSIZE + PHENTSIZE;
        elf[ph1..ph1 + 4].copy_from_slice(&u32le(PT_LOAD));
        elf[ph1 + 8..ph1 + 16].copy_from_slice(&u64le(0)); // p_offset (unused, filesz 0)
        elf[ph1 + 24..ph1 + 32].copy_from_slice(&u64le(bss_paddr)); // p_paddr
        elf[ph1 + 32..ph1 + 40].copy_from_slice(&u64le(0)); // p_filesz
        elf[ph1 + 40..ph1 + 48].copy_from_slice(&u64le(bss_memsz)); // p_memsz

        elf[code_offset..code_offset + code.len()].copy_from_slice(&code);
        elf
    }

    #[test]
    fn detects_elf_magic() {
        assert!(is_elf(&ELF_MAGIC));
        assert!(!is_elf(b"ANDROID!not an elf"));
        assert!(!is_elf(b"\x7fEL")); // too short
    }

    #[test]
    fn flattens_load_segments_at_correct_offsets() {
        let elf = build_test_elf();
        let flat = flatten_elf(&elf).expect("flatten failed");

        assert_eq!(flat.load_addr, 0x8100_0000);
        assert_eq!(flat.entry, 0x8100_0000);
        // code segment (16 bytes of 0xAA) + gap up to bss start (0x1000) +
        // bss tail (0x100), zero-filled.
        assert_eq!(flat.bytes.len(), 0x1000 + 0x100);
        assert_eq!(&flat.bytes[..16], &[0xAAu8; 16]);
        assert!(flat.bytes[16..].iter().all(|&b| b == 0));
    }

    #[test]
    fn rejects_non_elf() {
        assert!(flatten_elf(b"not an elf at all").is_err());
    }
}
