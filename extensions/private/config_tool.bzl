"""Builds xnu's real SETUP/config `config` binary from host bison/flex/cc, the
same doconf-driving tool Apple's own Makefiles invoke to resolve a MASTER
config into a per-target file list."""

load(":exec.bzl", "run")

def build_config_tool(ctx):
    bison = ctx.which("bison")
    flex = ctx.which("flex")
    cc = ctx.which("cc") or ctx.which("clang") or ctx.which("gcc")
    if not (bison and flex and cc):
        fail("xnu_kernel_source requires host bison, flex and a C compiler " +
             "on PATH to build xnu's SETUP/config tool")

    cfgsrc = "SETUP/config"
    build = "SETUP/config/.build"
    ctx.execute(["mkdir", "-p", build])

    run(ctx, [bison, "-y", "-d", "-d", "-o", build + "/parser.c", cfgsrc + "/parser.y"])
    run(ctx, [flex, "--header-file=" + build + "/lexer.yy.h", "-o", build + "/lexer.yy.c", cfgsrc + "/lexer.l"])

    objs = []
    for src, out in [
        (build + "/parser.c", build + "/parser.o"),
        (build + "/lexer.yy.c", build + "/lexer.o"),
        (cfgsrc + "/externs.c", build + "/externs.o"),
        (cfgsrc + "/main.c", build + "/main.o"),
        (cfgsrc + "/mkheaders.c", build + "/mkheaders.o"),
        (cfgsrc + "/mkioconf.c", build + "/mkioconf.o"),
        (cfgsrc + "/mkmakefile.c", build + "/mkmakefile.o"),
        (cfgsrc + "/openp.c", build + "/openp.o"),
        (cfgsrc + "/searchp.c", build + "/searchp.o"),
    ]:
        run(ctx, [cc, "-DYY_NO_INPUT", "-I" + cfgsrc, "-I" + build, "-w", "-c", "-o", out, src])
        objs.append(out)

    config_bin = build + "/config"
    run(ctx, [cc, "-o", config_bin] + objs)
    return config_bin
