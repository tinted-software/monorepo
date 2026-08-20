#!/usr/bin/env bash
# Extraction half of osfmk/conf/Makefile.template's assym.s rule (lines
# ~511-512): turns the `DEFINITION__define__NAME: .ascii "VALUE"` markers
# that genassym.c's DECLARE() macro (osfmk/arm64/genassym.c:113-114) embeds
# into an assembly listing into `#define NAME VALUE` (+ `NAME_NUM` companion)
# lines. The actual target-arch compile producing that assembly listing is
# done as a Bazel action in gen_assym.bzl (uses the real cc_toolchain, since
# offsets depend on the target's struct layout, not the host's).
#
# Usage: gen_assym_extract.sh <raw .s from `clang -S`> <out assym.s>
set -euo pipefail
raw=$1
out=$2

sed \
  -e '/^[[:space:]]*DEFINITION__define__/!d;{N;s/\n//;}' \
  -e 's/^[[:space:]]*DEFINITION__define__\([^:]*\):.*ascii.*"[$]*\([-0-9#]*\)".*$/#define \1 \2/' \
  -e 'p' \
  -e 's/#//2' \
  -e 's/^[[:space:]]*#define \([A-Za-z0-9_]*\)[[:space:]]*[$#]*\([-0-9]*\).*$/#define \1_NUM \2/' \
  "$raw" > "$out"
