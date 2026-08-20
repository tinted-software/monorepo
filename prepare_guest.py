#!/usr/bin/env python3
"""Lays out a vmapple XNU kernel Mach-O as a flat image for physical loading.

XNU's arm64 boot contract is that the kernel is resident in physical memory
mirroring its *virtual* layout: for every segment, PA == phys_base +
(vmaddr - virt_base). A raw copy of the Mach-O file satisfies that only if
`vmaddr - fileoff` is identical for every segment, and for the vmapple
kernel it is not:

    __DATA        vmaddr ...7fdc000  vmsize 0x178000  filesize 0x04c000
    __BOOTDATA    vmaddr ...8154000  fileoff 0x1024000

__DATA carries 0x12c000 bytes of zero-fill that occupy memory but not file,
so every segment after it sits 0x12c000 bytes too low in a flat copy.
__BOOTDATA is exactly what XNU's early boot consumes, so a flat-copied
kernel enters `_start` correctly and then reads garbage - which is why this
tool exists instead of a `dd`.

Segments with vmsize == 0 are not memory-resident (__CTF, the empty
__PLK_*/__PRELINK_* placeholders) and are skipped; several of them share a
vmaddr with a real segment and would otherwise overwrite it.

Usage: prepare_guest.py <kernel.macho> <out.bin>
Prints the entry offset and image span for cross-checking against
hv/src/guest.rs's GUEST_ENTRY_POINT_PA / GUEST_IMAGE_LEN.
"""

from __future__ import annotations

import struct
import sys

LC_SEGMENT_64 = 0x19
LC_UNIXTHREAD = 0x05
MH_MAGIC_64 = 0xFEEDFACF
VM_PROT_EXECUTE = 0x4


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <kernel.macho> <out.bin>", file=sys.stderr)
        return 1
    in_path, out_path = sys.argv[1], sys.argv[2]

    with open(in_path, "rb") as f:
        data = f.read()

    magic, _cputype, _cpusub, _ftype, ncmds = struct.unpack_from("<IiiII", data, 0)
    if magic != MH_MAGIC_64:
        print(f"ERROR: not a 64-bit Mach-O (magic {magic:#x})", file=sys.stderr)
        return 1

    segs = []
    entry = None
    off = 32
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from("<II", data, off)
        if cmd == LC_SEGMENT_64:
            name = data[off + 8 : off + 24].rstrip(b"\0").decode()
            vmaddr, vmsize, fileoff, filesize = struct.unpack_from(
                "<QQQQ", data, off + 24
            )
            initprot = struct.unpack_from("<i", data, off + 60)[0]
            segs.append((name, vmaddr, vmsize, fileoff, filesize, initprot))
        elif cmd == LC_UNIXTHREAD:
            # ARM_THREAD_STATE64: flavor, count, x0..x28, fp, lr, sp, pc
            entry = struct.unpack_from("<34Q", data, off + 16)[32]
        off += cmdsize

    if entry is None:
        print("ERROR: no LC_UNIXTHREAD entry point", file=sys.stderr)
        return 1

    text = next((s for s in segs if s[0] == "__TEXT"), None)
    if text is None:
        print("ERROR: no __TEXT segment", file=sys.stderr)
        return 1
    virt_base = text[1]

    resident = [s for s in segs if s[2] > 0 and s[1] >= virt_base]
    span = max(s[1] + s[2] for s in resident) - virt_base

    image = bytearray(span)
    placed = []
    for name, vmaddr, vmsize, fileoff, filesize, initprot in resident:
        dest = vmaddr - virt_base
        n = min(filesize, vmsize)
        if n:
            image[dest : dest + n] = data[fileoff : fileoff + n]
        placed.append((name, dest, vmsize, n, initprot))

    with open(out_path, "wb") as f:
        f.write(image)

    entry_off = entry - virt_base
    print(f"virt_base:    {virt_base:#x}")
    print(f"entry pc:     {entry:#x}")
    print(f"entry offset: {entry_off:#x}")
    print(f"image span:   {span:#x} ({span} bytes)")
    print()
    print(
        f"{'segment':18} {'dest off':>10} {'vmsize':>10} {'copied':>10} {'flat off':>10} exec"
    )
    for (name, dest, vmsize, n, initprot), seg in zip(placed, resident):
        flat = seg[3]
        mark = "*" if flat != dest else " "
        print(
            f"{name:18} {dest:#010x} {vmsize:#010x} {n:#010x} {flat:#010x}{mark} "
            f"{'yes' if initprot & VM_PROT_EXECUTE else 'no'}"
        )
    print()
    print("* = segment a flat file copy would have placed at the wrong offset")
    return 0


if __name__ == "__main__":
    sys.exit(main())
