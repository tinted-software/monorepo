"""Build-time MIG generation using //migcom (not the @xnu fetch)."""

# osfmk/mach/Makefile MIG_UUHDRS / MIG_KUHDRS+MIG_KUSRC / MIG_KSHDRS+MIG_KSSRC
_MIG_UUHDRS = [
    "clock",
    "clock_priv",
    "host_priv",
    "host_security",
    "mach_host",
    "mach_port",
    "mach_vm",
    "mach_voucher",
    "mach_voucher_attr_control",
    "memory_entry",
    "processor",
    "processor_set",
    "task",
    "thread_act",
    "vfs_nspace",
]
_MIG_KUHDRS = [
    "audit_triggers",
    "clock_reply",
    "doubleagent_mig",
    "exc",
    "host_notify_reply",
    "ktrace_background",
    "mach_exc",
    "mach_notify",
    "mach_test_upcall",
    "resource_notify",
    "task_access",
    "upl",
    "vm_map",
]
_MIG_KSHDRS = [
    "arcade_register",
    "clock",
    "mach_eventlink",
    "exc",
    "host_priv",
    "host_security",
    "mach_exc",
    "mach_host",
    "mach_notify",
    "mach_port",
    "mach_vm",
    "mach_voucher",
    "memory_entry",
    "processor",
    "processor_set",
    "restartable",
    "task",
    "thread_act",
    "upl",
    "vm_map",
    "vm32_map",
]

_CPP_BASE = [
    "-D__MACH30__",
    "-DAPPLE",
    "-DKERNEL",
    "-DKERNEL_PRIVATE",
    "-DXNU_KERNEL_PRIVATE",
    "-DPRIVATE",
    "-D__MACHO__=1",
    "-Dvolatile=__volatile",
    "-D__arm64__",
    "-D__LP64__",
]

def _slot(path):
    return "/dev/null" if path == "/dev/null" else "$(execpath %s)" % path

def _mig_genrule(rule_name, defs_name, header, user, server, sheader, mig_flags, cpp_flags):
    outs = [p for p in [header, user, server, sheader] if p != "/dev/null"]
    native.genrule(
        name = rule_name,
        srcs = [
            "@xnu//:osfmk/mach/%s.defs" % defs_name,
            "@xnu//:osfmk_defs",
            "@xnu//:osfmk_headers",
        ],
        tools = [
            "//migcom:migcom",
            "//tools/xnu_config:run_mig.sh",
        ],
        outs = outs,
        cmd = " ".join([
            "$(execpath //tools/xnu_config:run_mig.sh)",
            "$(execpath //migcom:migcom)",
            "$(execpath @xnu//:osfmk/mach/%s.defs)" % defs_name,
            _slot(header),
            _slot(user),
            _slot(server),
            _slot(sheader),
        ] + mig_flags + ["--"] + _CPP_BASE + cpp_flags),
        tags = ["manual"],
    )

def xnu_mig_stubs():
    """Declare genrules for every MIG invocation xnu's kernel build needs."""
    hdrs = []
    srcs = []

    for n in _MIG_UUHDRS:
        out = "mig/mach/%s.h" % n
        _mig_genrule("mig_%s_plain" % n, n, out, "/dev/null", "/dev/null", "/dev/null", [], [])
        hdrs.append(out)

    for n in _MIG_KUHDRS:
        h = "mig/mach/%s.h" % n
        c = "mig/mach/%s_user.c" % n
        _mig_genrule(
            "mig_%s_ku" % n,
            n,
            h,
            c,
            "/dev/null",
            "/dev/null",
            ["-maxonstack", "1024"],
            ["-DMACH_KERNEL_PRIVATE", "-DKERNEL_USER=1", "-DEXC_SERVER_AUDITTOKEN=1", "-DMACH_EXC_SERVER_AUDITTOKEN=1"],
        )
        hdrs.append(h)
        srcs.append(c)

    for n in _MIG_KSHDRS:
        c = "mig/mach/%s_server.c" % n
        h = "mig/mach/%s_server.h" % n
        _mig_genrule(
            "mig_%s_ks" % n,
            n,
            "/dev/null",
            "/dev/null",
            c,
            h,
            ["-mach_msg2"],
            ["-DMACH_KERNEL_PRIVATE", "-DKERNEL_SERVER=1"],
        )
        hdrs.append(h)
        srcs.append(c)

    native.filegroup(name = "mig_headers", srcs = hdrs, visibility = ["//visibility:public"])
    native.filegroup(name = "mig_srcs", srcs = srcs, visibility = ["//visibility:public"])

def xnu_files(paths):
    return ["@xnu//:" + p for p in paths]
