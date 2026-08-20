#!/usr/bin/env bash
# Preprocess an osfmk/mach/*.defs file and run //migcom on it.
# Usage:
#   run_mig.sh <migcom> <defs> <header> <user> <server> <sheader> \
#     [migcom flags...] -- [cpp flags...]
set -euo pipefail
migcom=$1
defs=$2
header=$3
user=$4
server=$5
sheader=$6
shift 6

mig_flags=()
while [[ $# -gt 0 && "$1" != "--" ]]; do
  mig_flags+=("$1")
  shift
done
if [[ $# -gt 0 && "$1" == "--" ]]; then
  shift
fi
cpp_flags=("$@")

osfmk=$(dirname "$(dirname "$defs")")
cc="${CC:-cc}"

for f in "$header" "$user" "$server" "$sheader"; do
  if [[ "$f" != /dev/null ]]; then
    mkdir -p "$(dirname "$f")"
  fi
done

{
  printf '#line 1 "%s"\n' "$(basename "$defs")"
  cat "$defs"
} | "$cc" -E -I"$osfmk" "${cpp_flags[@]}" - | "$migcom" -novouchers "${mig_flags[@]}" \
  -header "$header" \
  -user "$user" \
  -server "$server" \
  -sheader "$sheader"
