#!/usr/bin/env python3
"""LLM-assisted C -> Rust migration harness.

One run = one attempt to translate inih (ini.h + ini.c) and its test driver
(tests/unittest.c) into a single Rust program, then:
  1. cargo build             -> does it compile? how many errors?
  2. check_parity.sh         -> same output as baseline_multi.txt?
  3. static scan             -> unsafe blocks, external crates (hallucinated deps)
  4. token + time accounting -> from Ollama's own counters

Modes
  plain     one prompt, one answer
  feedback  if build or parity fails, send the compiler errors / output diff
            back to the model and let it fix the code (up to --max-iters rounds)

Every iteration appends one row to results.csv, so the "plain" result is the
feedback run's iteration 1 as well.

Usage
  python3 harness.py --model qwen2.5-coder:7b --mode plain    --name q7b_plain_1
  python3 harness.py --model qwen2.5-coder:7b --mode feedback --name q7b_fb_1
"""

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INIH = ROOT / "c_src" / "inih"
BASELINE = INIH / "tests" / "baseline_multi.txt"
OUT_DIR = ROOT / "rust_out"
RESULTS = ROOT / "results.csv"
OLLAMA = os.environ.get("OLLAMA_HOST", "http://localhost:11434").rstrip("/")

SYSTEM = (
    "You are an expert C and Rust engineer migrating legacy C code to safe, idiomatic Rust. "
    "Preserve behaviour exactly: the Rust program must print byte-for-byte the same output "
    "as the C program. Use only the Rust standard library (no external crates). "
    "Avoid `unsafe`. Reply with ONE complete src/main.rs inside a single ```rust code block "
    "and nothing else."
)

TASK = """Translate the following C code into ONE Rust program (src/main.rs).

Requirements:
- It must behave like the C test program compiled with DEFAULT settings from ini.h
  (no extra -D flags). Compile-time options you do not need may be dropped.
- main() must parse the same files, in the same order, and print exactly the same
  lines as unittest.c does (same format, same spacing, same error codes).
- Files are opened relative to the current working directory, like the C code.
- Standard library only. No `unsafe`.

=== ini.h ===
{ini_h}

=== ini.c ===
{ini_c}

=== tests/unittest.c ===
{unittest_c}
"""

FIX_BUILD = """Your Rust code does not compile. Here are the compiler errors:

{errors}

Fix the code. Reply with the complete corrected src/main.rs in one ```rust block."""

FIX_PARITY = """Your Rust program compiles, but its output differs from the C program.
Lines starting with '-' are expected (C output), '+' are what your program printed:

{diff}

Fix the code so the output is identical. Reply with the complete corrected src/main.rs in one ```rust block."""

STD_ROOTS = {"std", "core", "alloc", "crate", "self", "super"}
CARGO_TOML = """[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
"""


def chat(model, messages, timeout):
    body = json.dumps({
        "model": model, "messages": messages, "stream": False,
        "options": {"temperature": 0, "seed": 7, "num_ctx": 16384, "num_predict": 8192},
        "keep_alive": "30m",
    }).encode()
    req = urllib.request.Request(f"{OLLAMA}/api/chat", data=body, method="POST",
                                 headers={"Content-Type": "application/json"})
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        out = json.loads(resp.read())
    return (out["message"]["content"], out.get("prompt_eval_count", 0),
            out.get("eval_count", 0), time.perf_counter() - t0, out.get("done_reason", ""))


def extract_rust(text):
    blocks = re.findall(r"```(?:rust|rs)?\s*\n(.*?)```", text, re.S)
    if blocks:
        return max(blocks, key=len)
    return text if "fn main" in text else ""


def scan(code):
    crates = set()
    for m in re.finditer(r"^\s*(?:pub\s+)?use\s+([A-Za-z_][A-Za-z0-9_]*)", code, re.M):
        if m.group(1) not in STD_ROOTS:
            crates.add(m.group(1))
    for m in re.finditer(r"extern\s+crate\s+([A-Za-z_][A-Za-z0-9_]*)", code):
        crates.add(m.group(1))
    return len(re.findall(r"\bunsafe\b", code)), sorted(crates)


def build(project):
    p = subprocess.run(["cargo", "build", "--release", "--quiet", "--offline"], cwd=project,
                       capture_output=True, text=True, timeout=600)
    errors = [ln for ln in p.stderr.splitlines() if re.match(r"^error(\[E\d+\])?:", ln)]
    errors = [e for e in errors if "aborting due to" not in e and "could not compile" not in e]
    return p.returncode == 0, errors, p.stderr


def parity(binary):
    p = subprocess.run([str(ROOT / "check_parity.sh"), str(binary), str(BASELINE)],
                       capture_output=True, text=True, timeout=120)
    first = p.stdout.splitlines()[0] if p.stdout else "NO OUTPUT"
    m = re.search(r"\((\d+) lines differ\)", first)
    return p.returncode == 0, int(m.group(1)) if m else 0, p.stdout


def log_row(row):
    new = not RESULTS.exists()
    with open(RESULTS, "a", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(row))
        if new:
            w.writeheader()
        w.writerow(row)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", required=True)
    ap.add_argument("--mode", choices=["plain", "feedback"], default="plain")
    ap.add_argument("--name", required=True, help="attempt name, e.g. q7b_plain_1")
    ap.add_argument("--max-iters", type=int, default=3)
    ap.add_argument("--timeout", type=int, default=3600, help="seconds per model call")
    args = ap.parse_args()
    sys.stdout.reconfigure(line_buffering=True)

    src = {k: (INIH / p).read_text() for k, p in
           {"ini_h": "ini.h", "ini_c": "ini.c", "unittest_c": "tests/unittest.c"}.items()}
    messages = [{"role": "system", "content": SYSTEM},
                {"role": "user", "content": TASK.format(**src)}]
    iters = args.max_iters if args.mode == "feedback" else 1
    tot_in = tot_out = 0

    for it in range(1, iters + 1):
        project = OUT_DIR / f"{args.name}_it{it}"
        (project / "src").mkdir(parents=True, exist_ok=True)
        print(f"[{args.name}] iteration {it}: asking {args.model} ...")
        try:
            reply, t_in, t_out, secs, reason = chat(args.model, messages, args.timeout)
        except Exception as exc:
            print(f"  model call failed: {exc}")
            break
        tot_in += t_in
        tot_out += t_out
        (project / "reply.md").write_text(reply)
        code = extract_rust(reply)
        (project / "src" / "main.rs").write_text(code)
        crate_name = re.sub(r"[^a-z0-9_]", "_", f"{args.name}_it{it}".lower())
        (project / "Cargo.toml").write_text(CARGO_TOML.format(name=crate_name))
        unsafe_n, crates = scan(code)

        ok_build, errors, stderr = build(project) if code else (False, ["no code block in reply"], "")
        ok_parity, n_diff, parity_out = (False, 0, "")
        if ok_build:
            ok_parity, n_diff, parity_out = parity(project / "target" / "release" / crate_name)

        row = {
            "time": time.strftime("%Y-%m-%d %H:%M"), "name": args.name, "model": args.model,
            "mode": args.mode, "iteration": it, "compiled": ok_build, "compile_errors": len(errors),
            "parity_ok": ok_parity, "lines_differ": n_diff if ok_build else "",
            "unsafe_count": unsafe_n, "external_crates": " ".join(crates),
            "prompt_tokens": t_in, "output_tokens": t_out, "cum_tokens": tot_in + tot_out,
            "seconds": round(secs, 1), "truncated": reason == "length", "code_lines": code.count("\n"),
        }
        log_row(row)
        status = "PARITY OK" if ok_parity else (f"built, {n_diff} lines differ" if ok_build
                                                 else f"build failed, {len(errors)} errors")
        print(f"  {status} | unsafe={unsafe_n} crates={crates or '-'} | tokens in={t_in} out={t_out} "
              f"| {secs:.0f}s{' | ANSWER CUT OFF' if reason == 'length' else ''}")

        if ok_parity or args.mode == "plain":
            break
        messages.append({"role": "assistant", "content": reply})
        if not ok_build:
            messages.append({"role": "user", "content": FIX_BUILD.format(
                errors="\n".join(stderr.splitlines()[:80]) or "\n".join(errors))})
        else:
            messages.append({"role": "user", "content": FIX_PARITY.format(
                diff="\n".join(parity_out.splitlines()[1:41]))})

    print(f"done. total tokens: {tot_in + tot_out} (in {tot_in}, out {tot_out}). Rows in {RESULTS.name}")


if __name__ == "__main__":
    main()
