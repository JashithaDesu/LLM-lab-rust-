#!/usr/bin/env python3
"""Re-assemble a finished fbf run from its saved step_*.md files, WITHOUT calling the model.

Builds the run twice into rust_out/<run>_reasm/ and prints the compiler-error profile:
  1. as-is      (same assembly as the original run -> should reproduce its error count)
  2. --dedup    (duplicate top-level definitions removed, owner step wins)
The difference is the effect of the deterministic de-duplication alone.

Usage
  python3 reassemble.py q7b_fbf_2
"""

import re
import shutil
import sys

import harness as H
import harness_fbf as F


def main():
    run = sys.argv[1]
    src = H.OUT_DIR / run
    pieces = []
    for step, _, _ in F.UNITS:
        md = src / f"step_{step}.md"
        if not md.exists():
            sys.exit(f"{md} missing: run did not finish all steps")
        pieces.append((step, H.extract_rust(md.read_text()).strip()))

    project = H.OUT_DIR / f"{run}_reasm"
    (project / "src").mkdir(parents=True, exist_ok=True)
    crate = re.sub(r"[^a-z0-9_]", "_", f"{run}_reasm".lower())
    (project / "Cargo.toml").write_text(H.CARGO_TOML.format(name=crate))

    deduped, removed = F.dedup(pieces)
    for label, ps in (("as-is", pieces), ("dedup", deduped)):
        code = F.assemble(ps)
        (project / "src" / "main.rs").write_text(code)
        shutil.copy(project / "src" / "main.rs", project / f"main_{label}.rs")
        ok, errors, stderr = H.build(project)
        status = f"build failed, {len(errors)} errors" if not ok else "BUILT"
        if ok:
            par_ok, n_diff, _ = H.parity(project / "target" / "release" / crate)
            status = "PARITY OK" if par_ok else f"built, {n_diff} lines differ"
        print(f"\n[{run}] {label}: {status} | {code.count(chr(10))} Rust lines")
        if label == "dedup":
            print(f"  removed {len(removed)}: {', '.join(removed) or '-'}")
        print(F.error_summary(stderr))


if __name__ == "__main__":
    main()
