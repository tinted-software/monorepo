#!/usr/bin/env bash
# Extract the vmapple XNU kernelcache from the installed KDK, load it plus
# the OpenDarwin EL2 hypervisor (./hv) onto a Meson G12A (Superbird) over
# USB, and start hv - which then eret's into the guest.
#
# Kernel layout (see hv/src/guest.rs's GUEST_VIRT_BASE doc comment for the
# derivation): the vmapple kernelcache Mach-O's segments satisfy
# `vmaddr - fileoff == GUEST_VIRT_BASE` for every mapped segment, so the
# raw file bytes [0, GUEST_IMAGE_LEN) can be copied byte-for-byte to
# GUEST_ENTRY_PA with no per-segment remapping. If the KDK is ever
# updated, re-derive GUEST_VIRT_BASE/GUEST_IMAGE_LEN/GUEST_ENTRY_POINT_PA
# in guest.rs against the new kernel (see that file's comments for the
# exact llvm-objdump invocations) before re-running this script.
#
# Usage:
#   tools/boot_xnu.sh [path/to/kernel.development.vmapple]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="$root/target/superbird"
mkdir -p "$out_dir"

kdk_kernel="${1:-/Library/Developer/KDKs/KDK_27.0_26A5416b.kdk/System/Library/Kernels/kernel.development.vmapple}"
if [ ! -f "$kdk_kernel" ]; then
    echo "ERROR: vmapple kernel not found at '$kdk_kernel'" >&2
    echo "       pass the path explicitly: tools/boot_xnu.sh /path/to/kernel.development.vmapple" >&2
    exit 1
fi

# Must match GUEST_IMAGE_LEN in hv/src/guest.rs exactly. This is the span
# of the laid-out *image in memory*, which is larger than the Mach-O file's
# loadable extent - see prepare_guest.py and guest.rs's GUEST_VIRT_BASE.
guest_image_len=24461312
guest_entry_offset=0x3e4480

# Fix up and patch first, then lay out: both tools work on Mach-O file
# offsets and load commands, so they must see the real Mach-O, not the
# flattened image.
echo "==> Applying LC_DYLD_CHAINED_FIXUPS (data-segment pointers)..."
python3 "$root/tools/apply_fixups.py" "$kdk_kernel" "$out_dir/xnu.fixed.macho"

echo "==> Neutralizing arm64e PAC instructions (no FEAT_PAuth on G12A)..."
python3 "$root/tools/patch_pac.py" "$out_dir/xnu.fixed.macho" "$out_dir/xnu.macho"

echo "==> Laying out kernel segments for physical load..."
prep_out=$(python3 "$root/tools/prepare_guest.py" "$out_dir/xnu.macho" "$out_dir/xnu.bin")
echo "$prep_out"

# Cross-check the tool's derived layout against the constants hv was built
# with; a KDK update that shifts either one silently boots into the wrong
# address otherwise.
prep_entry=$(printf '%s\n' "$prep_out" | awk '/^entry offset:/ {print $3}')
prep_span=$(printf '%s\n' "$prep_out" | awk '/^image span:/ {print $4}' | tr -d '()')
if [ "$prep_entry" != "$guest_entry_offset" ]; then
    echo "ERROR: entry offset $prep_entry != $guest_entry_offset that hv/src/guest.rs's GUEST_ENTRY_POINT_PA assumes" >&2
    exit 1
fi
if [ "$prep_span" != "$guest_image_len" ]; then
    echo "ERROR: image span $prep_span != GUEST_IMAGE_LEN $guest_image_len in hv/src/guest.rs" >&2
    exit 1
fi
actual_len=$(stat -f%z "$out_dir/xnu.bin" 2>/dev/null || stat -c%s "$out_dir/xnu.bin")
if [ "$actual_len" -ne "$guest_image_len" ]; then
    echo "ERROR: laid-out image is $actual_len bytes, expected $guest_image_len" >&2
    exit 1
fi

echo "==> Building OpenDarwin EL2 hypervisor for Meson G12A..."
(cd "$root/hv" && cargo build --release)
hv_elf="$root/hv/target/aarch64-unknown-none/release/darwin-hv"

echo "==> Extracting raw ARM64 image..."
llvm-objcopy -O binary "$hv_elf" "$out_dir/hv.bin"
echo "==> hv image: $out_dir/hv.bin ($(stat -f%z "$out_dir/hv.bin" 2>/dev/null || stat -c%s "$out_dir/hv.bin") bytes)"

echo "==> Building amlogic-boot host tool..."
(cd "$root/tools" && cargo build --release -p amlogic-boot)
boot_bin="$root/tools/target/release/amlogic-boot"

echo "==> Checking device stage..."
"$boot_bin" identify

# Order matters: hvMain checks for the guest image's Mach-O magic
# immediately on entry (see boot.rs), so the guest kernel must already be
# resident in DRAM before hv.bin is written and run.
echo "==> Writing guest kernel to DRAM at 0x03004000..."
"$boot_bin" write-mem --addr 0x03004000 --file "$out_dir/xnu.bin"

echo "==> Writing hv.bin to DRAM at 0x01000000..."
"$boot_bin" write-mem --addr 0x01000000 --file "$out_dir/hv.bin"

echo "==> Executing hv at 0x01000000..."
exec "$boot_bin" run --addr 0x01000000
