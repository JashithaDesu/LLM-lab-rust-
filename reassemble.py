#!/usr/bin/env python3
"""Re-assemble a finished fbf run from its saved step_*.md files, WITHOUT calling the model.

Builds the run three ways into rust_out/<run>_reasm/ and prints the compiler-error profile:
  1. as-is      (same assembly as the original run -> should reproduce its error count)
  2. --dedup    (duplicate top-level definitions removed, owner step wins)
  3. --dedup + --merge-uses (all `use` imports expanded, de-duplicated, hoisted)

Usage
  python3 reassemble.py q7b_fbf_2
  python3 reassemble.py <run> --v2     (force the v2 prelude if results.csv has no row for the run)
"""

import csv
import re
import shutil
import sys

import harness as H
import harness_fbf as F


def main():
    run = [a for a in sys.argv[1:] if not a.startswith('--')][0]
    src = H.OUT_DIR / run
    # v2 runs start with the harness-defined constants block; detect from results.csv (mode "fbf-v2...")
    rows = list(csv.DictReader(open(H.RESULTS))) if H.RESULTS.exists() else []
    v2 = any(r["name"] == run and r["mode"].startswith("fbf-v2") for r in rows) or "--v2" in sys.argv
    pieces = [("prelude", F.PRELUDE_V2)] if v2 else []
    print(f"[{run}] prompt {'v2 (prelude included)' if v2 else 'v1'}")
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
    for label, ps, mu in (("as-is", pieces, False), ("dedup", deduped, False),
                          ("dedup+merge-uses", deduped, True)):
        code = F.assemble(ps, merge_uses=mu)
        (project / "src" / "main.rs").write_text(code)
        shutil.copy(project / "src" / "main.rs", project / f"main_{label.replace('+', '_')}.rs")
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
