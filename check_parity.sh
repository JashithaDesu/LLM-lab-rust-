#!/usr/bin/env bash
# Usage: ./check_parity.sh <program> <baseline-file>
# Runs <program> inside inih/tests (it reads the .ini files from there)
# and compares its output, line by line, with the expected baseline.
set -u
prog="$(realpath "$1")"
base="$(realpath "$2")"
cd "$(dirname "$base")"
out="$(mktemp)"
"$prog" > "$out" 2>&1
if diff -u "$base" "$out" > "$out.diff"; then
    echo "PARITY OK  ($(wc -l < "$base") lines identical)"
    exit 0
else
    echo "MISMATCH   ($(grep -c '^[-+][^-+]' "$out.diff") lines differ)"
    head -20 "$out.diff"
    exit 1
fi
