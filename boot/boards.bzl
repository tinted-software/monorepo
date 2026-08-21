"""The board list. Each entry is a `board(...)` record (see `board.bzl`);
`BUILD.bazel` just does `hv_targets(BOARDS)` once instead of hand-rolling
a rust_library/rust_binary/platform_transition_binary trio per board.

To port to a new board (e.g. gs201/Tensor G2), add a `board(...)` entry
here plus a matching linker script - no other file needs editing.
"""

load(":board.bzl", "board")

BOARDS = [
    # Superbird (Amlogic Meson G12A) - the only physical hardware target
    # today. `crate_features = []` selects the non-qemu cfg throughout
    # the crate (see board.rs / lib.rs / boot.rs module comments).
    board(
        name = "tinted-boot",
        linker_script = "linker.ld",
    ),

    # QEMU `virt` machine - validates guest-entry mechanics (stage-2, EL1
    # init, eret) fast and deterministically before trusting a result
    # observed only on physical hardware over USB. See linker-qemu.ld /
    # board.rs's module comment for why this exists alongside Superbird.
    board(
        name = "tinted-boot-qemu",
        linker_script = "linker-qemu.ld",
        crate_features = ["qemu"],
        runner = {},
        runner_name = "run_qemu",
    ),

    # Pixel 7a (Tensor G2 / gs201) - display-only bring-up scaffold: no
    # guest is loaded yet, this just validates EL2 entry + adopting ABL's
    # already-lit DPU_DMA L0 plane as a boot console (see
    # `display/gs201.rs`). GIC-v3 (not GIC-400 like Superbird) and PSCI
    # 1.0 (arm-psci crate should work unmodified) per this crate's
    # port-investigation notes.
    board(
        name = "tinted-boot-gs201",
        linker_script = "linker-gs201.ld",
        crate_features = ["gs201"],
    ),

    # Same as tinted-boot-gs201, but with display::init_early()'s DPU_DMA
    # register probe compiled out entirely (see boot.rs's hvMain doc
    # comment). Diagnostic-only: fastboot boot this first to check
    # whether the hv survives past EL2 entry/exception-vector-install at
    # all without ever touching 0x1C0B_0000, isolating that MMIO probe
    # as the reboot cause (or ruling it out) independent of everything
    # else. Delete once gs201 display bring-up is confirmed working.
    board(
        name = "tinted-boot-gs201-probe",
        linker_script = "linker-gs201.ld",
        crate_features = ["gs201", "gs201_probe"],
    ),
]
