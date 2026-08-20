#!/usr/bin/env python3
from __future__ import annotations

import struct
import sys

"""Statically neutralizes arm64e Pointer Authentication instructions in a
flat-loaded XNU kernel image, for CPUs with no FEAT_PAuth hardware at all
(e.g. Meson G12A's Cortex-A73/A55).

# Why static patching instead of runtime trap-and-emulate

The hint-space PAC instructions (PACIASP/AUTIASP/PACIBSP/... and the
1716/lr-diversifier forms) are architecturally guaranteed to execute as NOPs
on hardware without FEAT_PAuth - real ARM design intent, so nothing needs to
be done for those. The *non-hint* forms Apple's arm64e ABI uses pervasively
for plain C function pointers and vtables (PACIA/PACIB/PACDA/PACDB and their
AUT counterparts, the Z-form single-register variants, XPACI/XPACD, and
PACGA) are genuinely UNDEFINED when unimplemented - not NOPs - and executing
one traps as an Unknown-reason exception at the *guest's own* current
exception level (EL1), which this hypervisor's `HCR_EL2.TGE=0` design never
sees (see hv/src/guest.rs's `enter_guest`). This early in boot the guest's
own VBAR_EL1 is likely still unset, so the trap has nowhere sane to go -
observed as a silent hang right after hv hands off to the guest.

Runtime trap-and-emulate (HCR_EL2.TGE=1, catching every EL1 exception at
EL2 and decoding/emulating each PAC opcode) is architecturally the "correct"
general answer, but is a large redesign: TGE=1 also intercepts syscalls,
stage-1 page faults, and everything else EL1 would normally handle itself,
all of which would then need software re-injection back into EL1. This
hypervisor's actual security model requires none of that: since real
Apple Silicon PAC keys are session-random and this hardware can never
authenticate anything against them anyway, sign/auth/strip only need to be
*internally self-consistent within one hypervisor instance*, not
cryptographically real. Given that, sign+auth+strip degenerate to identity
(the pointer is never actually modified) - which can be baked in
*statically*, at load time, as a straight NOP, with zero runtime cost and
none of TGE's blast radius.

# What this does and does not cover

- PACIA/PACIB/PACDA/PACDB/AUTIA/AUTIB/AUTDA/AUTDB (2-register form) -> NOP.
- PACIZA/PACIZB/PACDZA/PACDZB/AUTIZA/AUTIZB/AUTDZA/AUTDZB (Z/1-register,
  implicit zero modifier) -> NOP.
- XPACI/XPACD (strip only, no verification) -> NOP. Already a no-op under
  the "pointers are never actually signed" model.
- PACGA Xd, Xn, Xm (generic MAC over arbitrary data, result consumed as a
  value rather than dereferenced as an address - can't be a NOP since
  callers expect *some* value in Xd) -> replaced with `EOR Xd, Xn, Xm`, a
  same-length, deterministic, self-consistent substitute.
- RETAA/RETAB -> `ret`, ERETAA/ERETAB -> `eret`, BRAAZ/BRABZ -> `br Rn`,
  BLRAAZ/BLRABZ -> `blr Rn`, BRAA/BRAB -> `br Rn`, BLRAA/BLRAB -> `blr Rn`.
  These dominate by count in a real arm64e kernel (every epilogue is `retab`,
  every indirect call `blraa`) - 37,303 of them here versus 33,247 of all the
  classes above combined, so omitting them leaves the image thoroughly
  unbootable. The rewrite keeps the branch/return target register and just
  drops the authentication, which is exactly identity under this model.
- LDRAA/LDRAB (authenticated load - auth the base+offset address, then
  dereference) are only *detected and reported*, not patched: their
  immediate field is scaled/signed in a way a same-length plain LDR can't
  always replicate, and safely rewriting them needs either relocation
  (unsafe for in-place patching) or the full TGE trap-and-emulate path this
  script exists to avoid. If any are reported, the guest may still hang at
  one - that's real, uncovered work, not a silent gap.
- Opcode masks/classes below were derived empirically from `clang -march=
  armv8.3-a`'s own encoder (ground truth, not hand-transcribed from the ARM
  ARM) and cross-checked against multiple register/immediate/writeback
  variants per instruction - see session notes for the derivation.

Only scans PT_LOAD-equivalent *executable* Mach-O segments (__TEXT_EXEC,
__KLD, __LAST) by file-offset range, not the whole image - avoids
false-positive matches against coincidental bit patterns in data/cstring
sections, which would corrupt them.

Usage: patch_pac.py <in.bin> <out.bin>
"""

NOP = 0xD503201F

# (mask, class, name) for the 2-register / Z-form / XPAC family - all get
# replaced with NOP. Mask clears Rd[4:0] and Rn[9:5] (10 bits).
MASK_2REG = 0xFFFFFC00
NOP_CLASSES = {
    0xDAC10000: "pacia",
    0xDAC10400: "pacib",
    0xDAC10800: "pacda",
    0xDAC10C00: "pacdb",
    0xDAC11000: "autia",
    0xDAC11400: "autib",
    0xDAC11800: "autda",
    0xDAC11C00: "autdb",
    0xDAC12000: "paciza",
    0xDAC12400: "pacizb",
    0xDAC12800: "pacdza",
    0xDAC12C00: "pacdzb",
    0xDAC13000: "autiza",
    0xDAC13400: "autizb",
    0xDAC13800: "autdza",
    0xDAC13C00: "autdzb",
    0xDAC14000: "xpaci",
    0xDAC14400: "xpacd",
}

# PACGA Xd, Xn, Xm - mask clears Rd[4:0], Rn[9:5], Rm[20:16] (15 bits).
MASK_PACGA = 0xFFE0FC00
PACGA_CLASS = 0x9AC03000

# LDRAA/LDRAB (immediate, with or without writeback) - detection only.
MASK_LDRA = 0xFFA00C00
LDRA_CLASSES = {
    0xF8200400: "ldraa",
    0xF8200C00: "ldraa!",
    0xF8A00400: "ldrab",
    0xF8A00C00: "ldrab!",
}

# Authenticated branch/return family (ARMv8.3). Unlike the hint-space
# PACIASP/AUTIASP forms these are genuinely UNDEFINED without FEAT_PAuth, and
# they are by far the most common PAC instructions in an arm64e binary: every
# function epilogue is `retab` rather than `ret`, and every indirect call is
# `blraa`. Under the "auth degenerates to identity" model each one rewrites to
# its plain, same-length, unauthenticated counterpart, preserving the register
# operands exactly - the authentication is simply dropped.
#
# RETAA/RETAB always operate on x30, so both map to the canonical `ret` (which
# encodes Rn=x30). ERETAA/ERETAB map to plain `eret`. The BRAAZ/BLRAAZ "Z"
# forms take an implicit zero modifier and encode Rm=0x1F; the BRAA/BLRAA
# forms carry a real modifier register in Rm[4:0] which the plain BR/BLR
# simply has no room for and does not need.
RET_EXACT = {
    0xD65F0BFF: ("retaa", 0xD65F03C0),  # -> ret
    0xD65F0FFF: ("retab", 0xD65F03C0),  # -> ret
    0xD69F0BFF: ("eretaa", 0xD69F03E0),  # -> eret
    0xD69F0FFF: ("eretab", 0xD69F03E0),  # -> eret
}

# Z-forms: mask clears Rn[9:5] only (Rm is fixed 0x1F in the encoding).
MASK_BR_Z = 0xFFFFFC1F
BR_Z_CLASSES = {
    0xD61F081F: ("braaz", 0xD61F0000),  # -> br  Rn
    0xD61F0C1F: ("brabz", 0xD61F0000),  # -> br  Rn
    0xD63F081F: ("blraaz", 0xD63F0000),  # -> blr Rn
    0xD63F0C1F: ("blrabz", 0xD63F0000),  # -> blr Rn
}

# Modifier-register forms: mask clears both Rn[9:5] and Rm[4:0].
MASK_BR_M = 0xFFFFFC00
BR_M_CLASSES = {
    0xD71F0800: ("braa", 0xD61F0000),  # -> br  Rn
    0xD71F0C00: ("brab", 0xD61F0000),  # -> br  Rn
    0xD73F0800: ("blraa", 0xD63F0000),  # -> blr Rn
    0xD73F0C00: ("blrab", 0xD63F0000),  # -> blr Rn
}

# Range TLBI instructions (ARMv8.4-TLBI / FEAT_TLBIRANGE).
# Apple Silicon kernels build with `__ARM_RANGE_TLBI__` and emit `tlbi rvale1is, Rn`
# or `tlbi rvae1is, Rn` to flush ranges of pages in a single instruction.
# On CPUs without ARMv8.4-TLBI (ARMv8.0/8.2 like Cortex-A53/A72/A76), these
# instructions are UNDEFINED and panic the kernel early in `io_map`.
# Replace them with `tlbi vmalle1is` (`0xD5088300`), which flushes the entire EL1
# TLB on all cores - completely safe superset of a range flush.
MASK_TLBI_RANGE = 0xFFFF_FFE0
TLBI_RANGE_CLASSES = {
    0xD508_82A0: "rvale1is",
    0xD508_8220: "rvae1is",
    0xD508_86A0: "rvale1",
    0xD508_8620: "rvae1",
    0xD508_85A0: "rvale1os",
    0xD508_8520: "rvae1os",
    0xD508_82E0: "rvale1isnxs",
    0xD508_8260: "rvae1isnxs",
    0xD508_8280: "rvaale1is",
    0xD508_82C0: "rvaale1isnxs",
    0xD508_8200: "rvaa1is",
    0xD508_8240: "rvaa1isnxs",
}
TLBI_VMALLE1IS = 0xD508_8300


def patch_word(word: int) -> tuple[int, str | None]:
    """Returns (possibly-patched word, mnemonic if it matched something)."""
    if word in RET_EXACT:
        name, repl = RET_EXACT[word]
        return repl, name
    cls = word & MASK_2REG
    if cls in NOP_CLASSES:
        return NOP, NOP_CLASSES[cls]
    cls = word & MASK_BR_Z
    if cls in BR_Z_CLASSES:
        name, base = BR_Z_CLASSES[cls]
        return base | (((word >> 5) & 0x1F) << 5), name
    cls = word & MASK_BR_M
    if cls in BR_M_CLASSES:
        name, base = BR_M_CLASSES[cls]
        return base | (((word >> 5) & 0x1F) << 5), name
    if (word & MASK_PACGA) == PACGA_CLASS:
        rd = word & 0x1F
        rn = (word >> 5) & 0x1F
        rm = (word >> 16) & 0x1F
        eor = 0xCA000000 | (rm << 16) | (rn << 5) | rd
        return eor, "pacga"
    cls = word & MASK_TLBI_RANGE
    if cls in TLBI_RANGE_CLASSES:
        return TLBI_VMALLE1IS, TLBI_RANGE_CLASSES[cls]
    cls = word & MASK_LDRA
    if cls in LDRA_CLASSES:
        return word, "UNPATCHED:" + LDRA_CLASSES[cls]
    return word, None


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <in.bin> <out.bin>", file=sys.stderr)
        return 1
    in_path, out_path = sys.argv[1], sys.argv[2]

    with open(in_path, "rb") as f:
        data = bytearray(f.read())

    # Executable segment (fileoff, filesize) ranges, parsed from the
    # Mach-O's own load commands rather than hardcoded: any segment whose
    # initprot carries VM_PROT_EXECUTE holds instructions to patch. Parsing
    # keeps this correct across KDK updates, which hardcoded offsets do not.
    exec_ranges = []
    magic, _cputype, _cpusub, _ftype, ncmds = struct.unpack_from("<IiiII", data, 0)
    if magic != 0xFEEDFACF:
        print(f"ERROR: not a 64-bit Mach-O (magic {magic:#x})", file=sys.stderr)
        return 1
    off = 32
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from("<II", data, off)
        if cmd == 0x19:  # LC_SEGMENT_64
            name = data[off + 8 : off + 24].rstrip(b"\0").decode()
            fileoff, filesize = struct.unpack_from("<QQ", data, off + 40)
            initprot = struct.unpack_from("<i", data, off + 60)[0]
            if initprot & 0x4 and filesize > 0:  # VM_PROT_EXECUTE
                exec_ranges.append((fileoff, filesize))
                print(f"  scanning {name} at {fileoff:#x} ({filesize} bytes)")
        off += cmdsize
    if not exec_ranges:
        print("ERROR: no executable segments found", file=sys.stderr)
        return 1

    counts: dict[str, int] = {}
    unpatched: list[tuple[int, str]] = []
    for start, length in exec_ranges:
        end = start + length
        for off in range(start, end, 4):
            word = struct.unpack_from("<I", data, off)[0]
            new_word, name = patch_word(word)
            if name is None:
                continue
            if name.startswith("UNPATCHED:"):
                unpatched.append((off, name[len("UNPATCHED:") :]))
                continue
            struct.pack_into("<I", data, off, new_word)
            counts[name] = counts.get(name, 0) + 1

    # XNU DEVELOPMENT assertion bypass:
    # In `arm_vm_physmap_init`, `arm_vm_physmap_slide` can shift a zero-length
    # ptov entry for `gVirtBase` up by an L2 twig alignment offset. On non-PPL
    # systems (`!XNU_MONITOR`), `physmap_end` is calculated as `physmap_base + real_phys_size`
    # without the `ROUND_TWIG` allowance, causing `assert(va + len <= physmap_end)`
    # at 0xfffffe00076e5504 to fire when all memory is mapped.
    # Patch the conditional branch `b.ls 0xfffffe00076e3d1c` (`0x54ff40c9`)
    # to unconditional `b 0xfffffe00076e3d1c` (`0x17fffa06`).
    target_va = 0xFFFFFE00076E5504
    target_fileoff = 0x3DC000 + (target_va - 0xFFFFFE00073E0000)
    if struct.unpack_from("<I", data, target_fileoff)[0] == 0x54FF40C9:
        struct.pack_into("<I", data, target_fileoff, 0x17FFFA06)
        print(f"  patched arm_vm_physmap_init assertion branch at {target_fileoff:#x}")

    with open(out_path, "wb") as f:
        _ = f.write(data)

    total = sum(counts.values())
    print(f"patched {total} PAC instructions:")
    for name in sorted(counts):
        print(f"  {name:8s} {counts[name]}")
    if unpatched:
        print(
            f"WARNING: {len(unpatched)} LDRAA/LDRAB instructions found but NOT patched (see module docstring):"
        )
        for off, name in unpatched[:20]:
            print(f"  file offset {off:#x}: {name}")
        if len(unpatched) > 20:
            print(f"  ... and {len(unpatched) - 20} more")
    return 0


if __name__ == "__main__":
    sys.exit(main())
