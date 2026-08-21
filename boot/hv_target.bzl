"""Generic target generation for `boot/boards.bzl`'s `BOARDS` list.

Each board previously hand-wrote its own copy of the rust_library +
rust_binary + platform_transition_binary trio (~50 lines each,
identical apart from crate_features/linker script) directly in
BUILD.bazel. `hv_targets(BOARDS)` replaces all of that with one call.
"""

load("@rules_rs//rs:rust_binary.bzl", "rust_binary")
load("@rules_rs//rs:rust_library.bzl", "rust_library")
load("@bazel_lib//lib:transitions.bzl", "platform_transition_binary")
load(":qemu.bzl", "qemu_hv_runner")

def _hv_target(name, linker_script, crate_features):
    """Defines `<name>-lib`, `<name>_raw`, and `<name>` (platform-transitioned)."""
    rust_library(
        name = name + "-lib",
        srcs = native.glob(["src/**/*.rs"], exclude = ["src/main.rs"]),
        crate_name = "tinted_boot",
        crate_features = crate_features,
        edition = "2024",
        tags = ["manual"],
        deps = [
            "@crates//:aarch64-cpu",
            "@crates//:spin",
        ],
    )

    rust_binary(
        name = name + "_raw",
        srcs = ["src/main.rs"],
        compile_data = [linker_script],
        crate_features = crate_features,
        edition = "2024",
        rustc_flags = [
            "-C",
            "link-arg=-Tboot/" + linker_script,
            "-C",
            "force-frame-pointers=yes",
        ],
        tags = ["manual"],
        deps = [
            ":" + name + "-lib",
            "@crates//:aarch64-cpu",
            "@crates//:spin",
        ],
    )

    platform_transition_binary(
        name = name,
        binary = ":" + name + "_raw",
        target_platform = "@rules_rs//rs/platforms:aarch64-unknown-none",
    )

def hv_targets(boards):
    """Defines the lib/binary/transition trio - and, where requested, a
    `qemu_hv_runner` - for every `board(...)` entry in `boards`.
    """
    for b in boards:
        _hv_target(
            name = b.name,
            linker_script = b.linker_script,
            crate_features = b.crate_features,
        )

        if b.runner != None:
            qemu_hv_runner(
                name = b.runner_name if b.runner_name else "run_" + b.name,
                hv = ":" + b.name + "_raw",
                **b.runner
            )
