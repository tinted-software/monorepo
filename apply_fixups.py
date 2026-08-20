#!/usr/bin/env python3
"""Applies LC_DYLD_CHAINED_FIXUPS to a vmapple XNU kernel Mach-O in place.

# Why this is needed

An arm64e kernel is shipped with its data-segment pointers *not* stored as
pointers. Every slot that would hold an address instead holds a packed
"chained fixup" entry: a target offset, optional PAC signing metadata, and an
11-bit `next` field linking to the following slot on the same page. Real boot
loaders (iBoot, or the KernelCollection builder for a kernelcache) walk those
chains at load time and overwrite each slot with the finished address.

Nothing in this project did that, so every pointer XNU loaded out of
__DATA_CONST/__DATA/__BOOTDATA was a raw chain entry. The symptom is a data
abort on an address with nonsense high bits very early in `arm_init` - e.g. a
fetch through `0x2000000021ca20`, which is not an address at all but a rebase
entry with `next=4` and `target=0x21ca20` (that target, added to the image
base, is the sane KVA `0xfffffe0007220a20` this script now writes).

# Format

`DYLD_CHAINED_PTR_ARM64E_KERNEL` (pointer_format 7), 4-byte chain stride.
Each 64-bit slot, discriminated by the top two bits:

    bit 63 auth, bit 62 bind, bits 51..61 next

    auth=0 (rebase):       target bits 0..42, high8 bits 43..50
    auth=1 (auth rebase):  target bits 0..31, diversity 32..47,
                           addrDiv 48, key 49..50

`target` is a runtime offset from the image's base VA (its lowest segment
`vmaddr`), not an absolute address - verified empirically against this
binary: interpreting it as an offset yields in-image kernel VAs for every
chain, while interpreting it as an absolute vmaddr does not.

The PAC metadata on auth entries is intentionally discarded: this project
neutralizes pointer authentication wholesale (see tools/patch_pac.py - the
target CPUs implement no FEAT_PAuth, so sign/auth/strip all degenerate to
identity), so the correct fixed-up value is the plain unsigned target.

Bind entries would need an import table; this kernel has `imports_count == 0`,
so encountering one means the format assumption is wrong and it is an error
rather than something to skip silently.

Usage: apply_fixups.py <in.macho> <out.macho>
"""
from __future__ import annotations

import struct
import sys

LC_SEGMENT_64 = 0x19
LC_DYLD_CHAINED_FIXUPS = 0x80000034
ARM64E_KERNEL = 7
CHAIN_STRIDE = 4


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <in.macho> <out.macho>", file=sys.stderr)
        return 1
    data = bytearray(open(sys.argv[1], "rb").read())

    ncmds = struct.unpack_from("<I", data, 16)[0]
    segs: list[tuple[str, int, int, int]] = []  # name, vmaddr, fileoff, filesize
    fixups: tuple[int, int] | None = None
    off = 32
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from("<II", data, off)
        if cmd == LC_SEGMENT_64:
            name = data[off + 8 : off + 24].rstrip(b"\0").decode()
            vmaddr, _vmsize, fileoff, filesize = struct.unpack_from("<4Q", data, off + 24)
            segs.append((name, vmaddr, fileoff, filesize))
        elif cmd == LC_DYLD_CHAINED_FIXUPS:
            fixups = struct.unpack_from("<II", data, off + 8)
        off += cmdsize

    if fixups is None:
        print("ERROR: no LC_DYLD_CHAINED_FIXUPS", file=sys.stderr)
        return 1

    # Image base is the lowest segment vmaddr that actually occupies memory.
    base = min(v for _n, v, _fo, fs in segs if fs)
    print(f"image base vmaddr: {base:#x}")

    blob, _blob_size = fixups
    _ver, starts_off, _imp_off, _sym_off, imports_count, _ifmt, _sfmt = struct.unpack_from(
        "<7I", data, blob
    )
    if imports_count:
        print(f"ERROR: {imports_count} imports; bind entries unsupported", file=sys.stderr)
        return 1

    starts = blob + starts_off
    seg_count = struct.unpack_from("<I", data, starts)[0]
    seg_offs = struct.unpack_from(f"<{seg_count}I", data, starts + 4)

    total = 0
    auth_total = 0
    lo, hi = None, None
    for idx, sofs in enumerate(seg_offs):
        if sofs == 0:
            continue
        p = starts + sofs
        _size, page_size, ptr_fmt, seg_off, _maxvp = struct.unpack_from("<IHHQI", data, p)
        page_count = struct.unpack_from("<H", data, p + 20)[0]
        page_start = struct.unpack_from(f"<{page_count}H", data, p + 22)
        if ptr_fmt != ARM64E_KERNEL:
            print(f"ERROR: unsupported pointer_format {ptr_fmt}", file=sys.stderr)
            return 1

        # `segment_offset` is a vmaddr delta from the image base; convert to a
        # file offset via the owning segment so slots are written in the file.
        seg_vmaddr = base + seg_off
        seg = next(
            (s for s in segs if s[3] and s[1] <= seg_vmaddr < s[1] + s[3]),
            None,
        )
        if seg is None:
            print(f"ERROR: no segment covers {seg_vmaddr:#x}", file=sys.stderr)
            return 1
        seg_file = seg[2] + (seg_vmaddr - seg[1])

        count = 0
        for page_idx, start in enumerate(page_start):
            if start == 0xFFFF:  # DYLD_CHAINED_PTR_START_NONE
                continue
            cursor = seg_file + page_idx * page_size + start
            while True:
                slot = struct.unpack_from("<Q", data, cursor)[0]
                auth = (slot >> 63) & 1
                bind = (slot >> 62) & 1
                nxt = (slot >> 51) & 0x7FF
                if bind:
                    print(f"ERROR: bind entry at file {cursor:#x}", file=sys.stderr)
                    return 1
                if auth:
                    value = base + (slot & 0xFFFFFFFF)
                    auth_total += 1
                else:
                    target = slot & 0x7FF_FFFF_FFFF
                    high8 = (slot >> 43) & 0xFF
                    value = (base + target) | (high8 << 56)
                struct.pack_into("<Q", data, cursor, value)
                lo = value if lo is None else min(lo, value)
                hi = value if hi is None else max(hi, value)
                count += 1
                if nxt == 0:
                    break
                cursor += nxt * CHAIN_STRIDE
        print(f"  {seg[0]:16s} {count} fixups")
        total += count

    open(sys.argv[2], "wb").write(data)
    print(f"applied {total} fixups ({auth_total} authenticated, PAC metadata dropped)")
    print(f"fixed-up value range: {lo:#x} .. {hi:#x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
