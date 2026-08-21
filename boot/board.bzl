"""Board-definition struct, Fuchsia `//boards/*.gni`-style: each board is a
small data record in its own file under `//boot/boards/`, and target
generation (`hv_target.bzl`) is generic over the whole list instead of
each board hand-writing its own copy of the rust_library/rust_binary/
platform_transition_binary trio. Adding a board (e.g. gs201) means adding
one `board(...)` value to `boot/boards/BUILD.bazel`'s list, not editing
`BUILD.bazel` or duplicating build rules.
"""

def board(name, linker_script, crate_features = [], runner = None, runner_name = None):
    """Describes one hv target board.

    Args:
        name: base target name, e.g. "tinted-boot", "tinted-boot-qemu".
        linker_script: linker script label (relative to //boot), applied
            both as compile_data and as the `-Tboot/<script>` link-arg.
        crate_features: `#[cfg(feature = "...")]` gates, forwarded to both
            the lib and the binary so they never drift apart.
        runner: optional dict of extra kwargs for a `qemu_hv_runner` bound
            to this board's `_raw` binary (e.g. `{}` for qemu; omit/None
            for boards with no hosted runner, like real hardware).
        runner_name: target name for the runner (defaults to "run_<name>").
    """
    return struct(
        name = name,
        linker_script = linker_script,
        crate_features = crate_features,
        runner = runner,
        runner_name = runner_name,
    )
