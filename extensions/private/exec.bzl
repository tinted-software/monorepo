"""Shared `ctx.execute`-and-check helper for the xnu repository rule."""

def run(ctx, args, cwd = None):
    res = ctx.execute(args, working_directory = cwd or ".", timeout = 600)
    if res.return_code != 0:
        fail("command failed: %s\n--- stdout ---\n%s\n--- stderr ---\n%s" %
             (" ".join(args), res.stdout, res.stderr))
    return res
