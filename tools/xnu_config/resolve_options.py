#!/usr/bin/env python3
"""Faithful, bounded re-implementation of xnu's SETUP/config/doconf attribute
resolution algorithm (see xnu/config/MASTER header comment for the spec this
implements) so that a real `config`-format sysconf file can be generated for
a target that Apple's device map (Kernel Debug Kit / EDM database) normally
supplies, but which is not present in the public xnu source drop.

This only *reads* upstream MASTER/MASTER.<cpu>.<platform> files verbatim; it
does not modify xnu sources. It reproduces the exact rules documented at the
top of xnu/config/MASTER:

    <configuration> = [ <attr0> <attr1> ... <attrN> ]
    A directive tagged `# <foo,bar>` is selected if "foo" or "bar" is in the
    resolved attribute set for the configuration being built. `<!foo,bar>`
    selects the line if none of "foo"/"bar" are in the set. No tag => always
    selected.

Usage:
    resolve_options.py --master MASTER --master-cpu MASTER.arm64.MacOSX \
        --config DEVELOPMENT --define SOC_IS_VIRTUALIZED --out sysconf.in \
        --machine arm64 --sourcedir /abs/path/to/xnu \
        --objectdir /abs/path/to/objdir --builddir BUILDDIR
"""
import argparse
import re
import sys

MACRO_RE = re.compile(r'^#\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\[(.*)\]\s*$')
IF_RE = re.compile(r'^#if\s+(.*)$')
ELIF_RE = re.compile(r'^#elif\s+(.*)$')
ELSE_RE = re.compile(r'^#else\b')
ENDIF_RE = re.compile(r'^#endif\b')
DIRECTIVE_RE = re.compile(
    r'^(machine|options|makeoptions|pseudo-device|profile|mandatory)\b(.*)$')
TAG_RE = re.compile(r'#\s*<([^>]*)>\s*$')


def eval_cpp_expr(expr, defines):
    """Tiny boolean-expr evaluator for the handful of preprocessor macros
    (MASTER_CONFIG_ENABLE_EXCLAVES/SPTM/KERNEL_TAG, SOC_CONFIG_*,
    SOC_IS_VIRTUALIZED) used inside MASTER*.  Supports &&, ||, !, parens and
    bare identifiers."""
    expr = expr.strip()
    tokens = re.findall(r'\(|\)|&&|\|\||!|[A-Za-z_][A-Za-z0-9_]*', expr)
    pos = 0

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def parse_or():
        nonlocal pos
        v = parse_and()
        while peek() == '||':
            pos += 1
            v = parse_and() or v
        return v

    def parse_and():
        nonlocal pos
        v = parse_not()
        while peek() == '&&':
            pos += 1
            v = parse_not() and v
        return v

    def parse_not():
        nonlocal pos
        if peek() == '!':
            pos += 1
            return not parse_not()
        return parse_atom()

    def parse_atom():
        nonlocal pos
        if peek() == '(':
            pos += 1
            v = parse_or()
            assert peek() == ')'
            pos += 1
            return v
        ident = tokens[pos]
        pos += 1
        return bool(defines.get(ident, False))

    return parse_or()


def strip_cpp_conditionals(lines, defines):
    """Drop lines guarded by #if/#elif/#else/#endif blocks whose condition
    evaluates false, using `defines`. Only understands the small subset of
    cpp used inside MASTER* files."""
    out = []
    stack = []  # list of (taken_before, branch_taken_already, active)

    def active():
        return all(s[2] for s in stack)

    for line in lines:
        s = line.rstrip('\n')
        m = IF_RE.match(s.strip())
        if m:
            cond = eval_cpp_expr(m.group(1), defines) if active() else False
            stack.append([active(), cond, cond])
            continue
        m = ELIF_RE.match(s.strip())
        if m:
            parent_active, taken, _ = stack[-1]
            cond = (not taken) and parent_active and eval_cpp_expr(m.group(1), defines)
            stack[-1] = [parent_active, taken or cond, cond]
            continue
        if ELSE_RE.match(s.strip()):
            parent_active, taken, _ = stack[-1]
            cond = (not taken) and parent_active
            stack[-1] = [parent_active, taken or cond, cond]
            continue
        if ENDIF_RE.match(s.strip()):
            stack.pop()
            continue
        if active():
            out.append(s)
    return out


def parse_macros(lines):
    macros = {}
    for line in lines:
        m = MACRO_RE.match(line)
        if m:
            name, body = m.group(1), m.group(2)
            macros[name] = body.split()
    return macros


def expand_tags(name, macros, seen=None):
    """Recursively expand a macro name (e.g. DEVELOPMENT) into the flat set
    of leaf attribute tags, per the KERNEL_BASE/KERNEL_DEV/... nesting."""
    if seen is None:
        seen = set()
    result = set()
    for tok in macros.get(name, [name]):
        if tok in seen:
            continue
        if tok in macros:
            seen.add(tok)
            result |= expand_tags(tok, macros, seen)
        else:
            result.add(tok.lower())
    return result


def parse_directives(lines):
    """Return list of (keyword, rest_of_line) for real (non-comment)
    directive lines, honoring trailing `# <tag,tag>` selectors resolved by
    the caller."""
    out = []
    for line in lines:
        if line.lstrip().startswith('#'):
            continue
        m = DIRECTIVE_RE.match(line.strip())
        if not m:
            continue
        out.append(line)
    return out


def line_selected(line, tagset):
    m = TAG_RE.search(line)
    if not m:
        return True
    taglist = m.group(1).strip()
    negate = taglist.startswith('!')
    if negate:
        taglist = taglist[1:]
    tags = [t.strip().lower() for t in taglist.split(',') if t.strip()]
    hit = any(t in tagset for t in tags)
    return (not hit) if negate else hit


def strip_directive_body(line):
    # Drop trailing comment (both plain "# text" and "# <tag>") for emission.
    body = line.split('#', 1)[0].rstrip()
    return body


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--master', required=True)
    ap.add_argument('--master-cpu', required=True)
    ap.add_argument('--config', required=True, help='e.g. DEVELOPMENT')
    ap.add_argument('--define', action='append', default=[],
                     help='cpp macro considered true, e.g. SOC_IS_VIRTUALIZED')
    ap.add_argument('--machine', required=True)
    ap.add_argument('--sourcedir', required=True)
    ap.add_argument('--objectdir', required=True)
    ap.add_argument('--builddir', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()

    defines = {d: True for d in args.define}

    with open(args.master) as f:
        master_lines = f.readlines()
    with open(args.master_cpu) as f:
        cpu_lines = f.readlines()

    master_lines = strip_cpp_conditionals(master_lines, defines)
    cpu_lines = strip_cpp_conditionals(cpu_lines, defines)

    macros = {}
    macros.update(parse_macros(master_lines))
    # cpu-specific macro defs override/extend machine-independent ones.
    macros.update(parse_macros(cpu_lines))

    tagset = expand_tags(args.config, macros)
    tagset.add(args.config.lower())

    directives = parse_directives(master_lines) + parse_directives(cpu_lines)
    selected = [strip_directive_body(l) for l in directives if line_selected(l, tagset)]

    with open(args.out, 'w') as out:
        out.write('machine\t\t"%s"\n' % args.machine)
        for d in selected:
            d = d.strip()
            if not d or d.startswith('machine'):
                continue
            out.write(d + '\n')
        out.write('builddir\t"%s"\n' % args.builddir)
        out.write('objectdir\t"%s"\n' % args.objectdir)
        out.write('sourcedir\t"%s"\n' % args.sourcedir)

    sys.stderr.write('resolved %d attribute tags, %d directives selected\n' %
                      (len(tagset), len(selected)))


if __name__ == '__main__':
    main()
