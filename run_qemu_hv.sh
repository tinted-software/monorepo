#!/usr/bin/env bash
# Boot the OpenDarwin EL2 hypervisor (and the same patched+prepared vmapple
# XNU image tools/boot_xnu.sh uses) under QEMU's aarch64 `virt` machine.
#
# Exists to validate guest-entry mechanics (stage-2, EL1 init, the
# single-step tracer, eret) against a fast, deterministic, GDB-inspectable
# target before trusting a result observed only on physical Superbird
# hardware over USB with no reset between attempts (`keep_power=true`
# means every real-hardware boot this session accumulated TLB/cache state
# from every prior one - QEMU has no such confound). See hv/src/board.rs.
#
# `-cpu cortex-a76`, deliberately not `-cpu max`: like the real target, A76
# implements no FEAT_PAuth, so unpatched/mis-patched PAC instructions still
# fault here instead of executing "correctly" by accident and silently
# invalidating the comparison. This script always boots the same
# tools/patch_pac.py-neutralized image either way.
#
# A76 rather than the real board's core, though, because of one hard
# divergence: the prebuilt vmapple kernel is built __ARM_16K_PG__ (verified -
# osfmk/arm64/proc_reg.h picks ARM_PGSHIFT 14 / TCR_TG0_GRANULE_16KB /
# ARM_TT_L2_SHIFT 25, and the bootstrap tables it builds at runtime were
# observed to be exactly 16KB-granule shaped: 32MiB L2 blocks, memSize>>25
# entries, L2 index (pa>>25)&0x7FF). The 16KB granule is OPTIONAL in ARMv8
# and Amlogic G12A's Cortex-A53 does not implement it, so on real hardware
# the walker mis-parses TCR_EL1.TG0 and the guest takes a level-1 translation
# fault on its own PC the instant it enables the MMU. A76 does implement it,
# which is what lets the guest get past MMU enable at all. Running the
# prebuilt vmapple binary on A53 is therefore blocked on page size, not on
# anything this hypervisor does; closing it means a 4KB-page XNU built from
# source (we have the tree) rather than more hypervisor work.
#
# Usage: tools/run_qemu_hv.sh [-- QEMU_ARGS...]
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ "${1:-}" = "--" ]; then
    shift
fi

out_dir="$root/target/qemu-hv"
mkdir -p "$out_dir"

kdk_kernel="${KDK_KERNEL:-/Library/Developer/KDKs/KDK_27.0_26A5416b.kdk/System/Library/Kernels/kernel.development.vmapple}"
if [ ! -f "$kdk_kernel" ]; then
    echo "ERROR: vmapple kernel not found at '$kdk_kernel'" >&2
    exit 1
fi

# Must match hv/src/guest.rs's constants (board::DRAM_BASE + the same
# offsets boot_xnu.sh's Superbird build uses).
dram_base=0x40000000
guest_entry_pa=0x43004000
guest_image_len=24461312
guest_entry_offset=0x3e4480

echo "==> Applying LC_DYLD_CHAINED_FIXUPS (data-segment pointers)..."
python3 "$root/tools/apply_fixups.py" "$kdk_kernel" "$out_dir/xnu.fixed.macho"

echo "==> Neutralizing arm64e PAC instructions (no FEAT_PAuth on cortex-a76 either)..."
python3 "$root/tools/patch_pac.py" "$out_dir/xnu.fixed.macho" "$out_dir/xnu.macho"

echo "==> Laying out kernel segments for physical load..."
prep_out=$(python3 "$root/tools/prepare_guest.py" "$out_dir/xnu.macho" "$out_dir/xnu.bin")
echo "$prep_out"
prep_entry=$(printf '%s\n' "$prep_out" | awk '/^entry offset:/ {print $3}')
prep_span=$(printf '%s\n' "$prep_out" | awk '/^image span:/ {print $4}' | tr -d '()')
if [ "$prep_entry" != "$guest_entry_offset" ] || [ "$prep_span" != "$guest_image_len" ]; then
    echo "ERROR: prepared image layout ($prep_entry, $prep_span bytes) doesn't match hv/src/guest.rs's constants ($guest_entry_offset, $guest_image_len bytes)" >&2
    exit 1
fi

echo "==> Building hv for QEMU virt (board::DRAM_BASE=$dram_base)..."
(
    cd "$root/hv"
    RUSTFLAGS="-C link-arg=-T -C link-arg=boot/linker-qemu.ld -C force-frame-pointers=yes" \
        cargo build --release --features qemu
)
hv_elf="$root/hv/target/aarch64-unknown-none/release/darwin-hv"
echo "==> hv ELF: $hv_elf"

qemu_args=(
    # virtualization=on is REQUIRED: without it the `virt` machine gives the
    # CPU no EL2 at all, so this hypervisor's very first `msr vbar_el2` is
    # undefined and nothing works.
    -M virt,virtualization=on
    -cpu cortex-a76
    -m 512M
    -nographic
    -kernel "$hv_elf"
    -device "loader,file=$out_dir/xnu.bin,addr=$guest_entry_pa,force-raw=on"
)

# The guest currently ends in a tight fault loop rather than reaching a
# prompt, and -nographic wires QEMU to this terminal, so default to a bounded
# run instead of wedging the caller. QEMU_TIMEOUT=0 restores an open-ended
# interactive session.
timeout_secs="${QEMU_TIMEOUT:-15}"

echo "==> qemu-system-aarch64 ${qemu_args[*]} $*"
if [ "$timeout_secs" = "0" ]; then
    exec qemu-system-aarch64 "${qemu_args[@]}" "$@"
fi
exec timeout "$timeout_secs" qemu-system-aarch64 "${qemu_args[@]}" "$@" < /dev/null
