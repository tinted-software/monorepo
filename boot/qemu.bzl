"""QEMU hypervisor runner rule consuming rules_qemu system toolchain."""

_QEMU_SYSTEM_TOOLCHAIN_TYPE = Label("@rules_qemu//qemu:exec_toolchain_type")

def _transition_platform_impl(_, attr):
    return {"//command_line_option:platforms": [str(attr.target_platform)]}

_transition_platform = transition(
    implementation = _transition_platform_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _qemu_hv_runner_impl(ctx):
    toolchain = ctx.toolchains[_QEMU_SYSTEM_TOOLCHAIN_TYPE]

    # ctx.attr.hv is transitioned
    hv_binary = ctx.file.hv

    executable = ctx.actions.declare_file(ctx.label.name)

    # Generate shell launcher running hermetic QEMU with the HV ELF and arguments
    content = """#!/usr/bin/env bash
set -euo pipefail

# Locate runfiles
if [ -n "${{RUNFILES_MANIFEST_FILE:-}}" ]; then
    rlocation() {{
        local target="$1"
        local match
        match="$(grep -m1 "^$target " "$RUNFILES_MANIFEST_FILE" | cut -d' ' -f2-)"
        if [ -n "$match" ]; then
            echo "$match"
        else
            echo "$target"
        fi
    }}
elif [ -n "${{RUNFILES_DIR:-}}" ]; then
    rlocation() {{
        echo "$RUNFILES_DIR/$1"
    }}
else
    rlocation() {{
        echo "$PWD/$1"
    }}
fi

QEMU_BIN="$(rlocation "{qemu_system_path}")"
HV_ELF="$(rlocation "{hv_path}")"
QEMU_DATA_DIR="$(rlocation "{qemu_data_path}")"

QEMU_ARGS=(
    -M virt,virtualization=on
    -cpu cortex-a76
    -m 512M
    -nographic
    -L "$QEMU_DATA_DIR"
    -kernel "$HV_ELF"
)

TIMEOUT="${{QEMU_TIMEOUT:-15}}"

if [ "$#" -gt 0 ]; then
    QEMU_ARGS+=("$@")
fi

echo "==> Running $QEMU_BIN ${{QEMU_ARGS[*]}}"
if [ "$TIMEOUT" = "0" ]; then
    exec "$QEMU_BIN" "${{QEMU_ARGS[@]}}"
fi
exec timeout "$TIMEOUT" "$QEMU_BIN" "${{QEMU_ARGS[@]}}" < /dev/null
""".format(
        qemu_system_path = toolchain.qemu_system.short_path,
        hv_path = hv_binary.short_path,
        qemu_data_path = toolchain.system_data_anchor.short_path,
    )

    ctx.actions.write(
        output = executable,
        content = content,
        is_executable = True,
    )

    runfiles = ctx.runfiles(
        files = [
            executable,
            hv_binary,
            toolchain.qemu_system,
            toolchain.qemu_img,
            toolchain.system_data_anchor,
        ],
        transitive_files = toolchain.system_data_files,
    )

    return [
        DefaultInfo(
            executable = executable,
            files = depset([executable]),
            runfiles = runfiles,
        ),
    ]

qemu_hv_runner = rule(
    implementation = _qemu_hv_runner_impl,
    attrs = {
        "hv": attr.label(
            allow_single_file = True,
            cfg = _transition_platform,
            mandatory = True,
            doc = "The hypervisor binary (transitioned to target_platform).",
        ),
        "target_platform": attr.label(
            default = "@rules_rs//rs/platforms:aarch64-unknown-none",
            doc = "The platform to transition the HV binary to.",
        ),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
    executable = True,
    toolchains = [_QEMU_SYSTEM_TOOLCHAIN_TYPE],
    doc = "Runs the hypervisor under QEMU system-mode emulation with platform transition.",
)
