#!/usr/bin/env bash
# Build the OpenDarwin EL2 hypervisor (./hv) and RAM-boot it onto a Meson
# G12A (Spotify Car Thing / Superbird) over USB, via ./tools/amlogic-boot.
#
# Cargo-workspace replacement for the old buck2 `build_superbird.sh` +
# `//tools/amlogic:amlogic-boot` pair (see ref/tools/ for the originals,
# kept only as historical reference - ./hv and ./tools/amlogic-boot are now
# the maintained copies).
#
# Prerequisite: the board must already be past MaskROM (Stage 0.16) and
# running in U-Boot USB Burn Mode (Stage 0.2 / TPL) for `ramboot` to work -
# `amlogic-boot identify` reports the current stage. Getting there from a
# cold MaskROM boot needs a real Superbird U-Boot/FIP binary passed to
# `amlogic-boot boot-g12 --uboot <path>` first; this repo does not vendor
# one (unclear licensing/provenance for a binary blob extracted from
# Spotify's signed firmware). The community-maintained
# github.com/bishopdynamics/superbird-tool repo documents extracting one
# from a factory firmware dump/OTA image, or building it from source
# against the Superbird board config.
#
# Usage:
#   tools/boot_superbird.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="$root/target/superbird"
mkdir -p "$out_dir"

echo "==> Building OpenDarwin EL2 hypervisor for Meson G12A..."
(cd "$root/hv" && cargo build --release)
hv_elf="$root/hv/target/aarch64-unknown-none/release/darwin-hv"
echo "==> ELF: $hv_elf"

echo "==> Extracting raw ARM64 image..."
llvm-objcopy -O binary "$hv_elf" "$out_dir/hv.bin"
echo "==> Raw image: $out_dir/hv.bin ($(stat -f%z "$out_dir/hv.bin" 2>/dev/null || stat -c%s "$out_dir/hv.bin") bytes)"

echo "==> Building amlogic-boot host tool..."
(cd "$root/tools" && cargo build --release -p amlogic-boot)
boot_bin="$root/tools/target/release/amlogic-boot"

echo "==> Checking device stage..."
"$boot_bin" identify

# hv is a bare-metal EL2 stub with no U-Boot/DTB dependency, so this skips
# BootG12/TPL entirely and uses the raw write-mem + run primitives, which
# work directly against MaskROM (Stage 0.16) - the same two primitives
# BootG12 itself uses to bootstrap BL2.
echo "==> Writing hv.bin to DRAM at 0x01000000..."
"$boot_bin" write-mem --addr 0x01000000 --file "$out_dir/hv.bin"

echo "==> Executing hv at 0x01000000..."
exec "$boot_bin" run --addr 0x01000000
