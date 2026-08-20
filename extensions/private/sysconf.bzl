"""Drives xnu's real `config`/doconf machinery to resolve a MASTER config
into a per-subsystem generated Makefile, then extracts the file/define lists
that Makefile declares."""

load(":exec.bzl", "run")

def gen_sysconf_and_run(ctx, python3, resolver, config_bin, subsys, objectdir_abs):
    confdir = subsys + "/conf"
    sysconf_name = "DEVELOPMENT_ARM64VMAPPLE"
    root_abs = str(ctx.path("."))
    run(ctx, [
        python3,
        str(resolver),
        "--master",
        "config/MASTER",
        "--master-cpu",
        "config/MASTER.arm64.MacOSX",
        "--config",
        "DEVELOPMENT",
        "--define",
        "SOC_IS_VIRTUALIZED",
        "--machine",
        "arm64",
        "--sourcedir",
        root_abs,
        "--objectdir",
        objectdir_abs,
        "--builddir",
        ".",
        "--out",
        confdir + "/" + sysconf_name,
    ])
    ctx.execute(["mkdir", "-p", objectdir_abs])
    run(ctx, [str(ctx.path(config_bin)), "-c", ".", sysconf_name], cwd = confdir)
    makefile = ctx.path(objectdir_abs + "/Makefile")
    return ctx.read(makefile)

def _ws_split(line):
    return [tok for tok in line.replace("\t", " ").split(" ") if tok]

def _shlex_split(line):
    """Whitespace-split `line`, except spaces inside a "..." span (needed
    for CONFIG_DEFINES values like `-DCONFIG_NMBCLUSTERS="((1024*256)/X)"`)."""
    tokens = []
    cur = ""
    in_quotes = False
    for i in range(len(line)):
        ch = line[i]
        if ch == '"':
            in_quotes = not in_quotes
            cur += ch
        elif ch in (" ", "\t") and not in_quotes:
            if cur:
                tokens.append(cur)
                cur = ""
        else:
            cur += ch
    if cur:
        tokens.append(cur)
    return tokens

def extract_defines(text):
    """Extract the real, config-computed `-D...` flags for every declared
    MASTER option (`export CONFIG_DEFINES = -DFOO -DBAR=1 ...` in the
    generated Makefile) - authoritative source for option macros that
    aren't routed through meta_features.h (e.g. CONFIG_CLUTCH, which
    osfmk/kern/sched.h itself derives CONFIG_SCHED_CLUTCH from)."""
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("export CONFIG_DEFINES") or line.startswith("CONFIG_DEFINES"):
            idx = line.find("=")
            if idx != -1:
                return _shlex_split(line[idx + 1:])
    return []

def extract_var(text, name):
    """Extract a (possibly backslash-continued) `NAME=...` make variable's
    whitespace-separated values from a generated Makefile."""
    lines = text.splitlines()
    out = []
    capture = False
    for line in lines:
        if not capture:
            if line.startswith(name + "="):
                capture = True
                line = line[len(name) + 1:]
            else:
                continue
        cont = line.endswith("\\")
        if cont:
            line = line[:-1]
        out.extend(_ws_split(line))
        capture = cont
    return out

def normalize(subsys, files):
    """Convert config-emitted paths (either "./rel/to/subsys" or
    "$(SOURCE_DIR)/rel/to/root") into paths relative to the xnu repo root."""
    out = []
    for f in files:
        if f.startswith("$(SOURCE_DIR)/"):
            out.append(f[len("$(SOURCE_DIR)/"):])
        elif f.startswith("./"):
            out.append(subsys + "/" + f[2:])
        else:
            out.append(subsys + "/" + f)
    return out
