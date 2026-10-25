#!/usr/bin/env python3
"""Function-by-function (fbf) C -> Rust migration.

Hypothesis tested: whole-file prompts make the model drop functionality
(71-103 Rust lines for 416 C lines, 0/15 parity). Translating one unit at a
time, each with the Rust written so far as context, should keep more of the
original logic.

Steps (each step = one model call, logged as its own row in results.csv):
  1. helpers        ini_rstrip, ini_lskip, ini_find_chars_or_comment, ini_strncpy0
  2. parse_stream   ini_parse_stream (the core parser, default settings from ini.h)
  3. file_api       ini_parse_file, ini_parse
  4. test_driver    dumper(), parse(), main() from tests/unittest.c
Then the pieces are assembled into one main.rs, built and parity-checked.
With --fix-iters > 0, compiler errors / output diffs of the assembled program
are fed back (same loop as harness.py feedback mode).

Usage
  python3 harness_fbf.py --model qwen2.5-coder:7b --name q7b_fbf_1
  python3 harness_fbf.py --model qwen2.5-coder:7b --name q7b_fbf_fix_1 --fix-iters 2
"""

import argparse
import re
import sys
import time
from pathlib import Path

import harness as H   # reuse chat(), extract_rust(), scan(), build(), parity(), log_row()

UNITS = [
    ("helpers", "ini.c", ["ini_rstrip", "ini_lskip", "ini_find_chars_or_comment", "ini_strncpy0"]),
    ("parse_stream", "ini.c", ["ini_parse_stream"]),
    ("file_api", "ini.c", ["ini_parse_file", "ini_parse"]),
    ("test_driver", "tests/unittest.c", ["dumper", "parse", "main"]),
]

SYSTEM = (
    "You are an expert C and Rust engineer migrating legacy C code to safe, idiomatic Rust, "
    "one piece at a time. Preserve behaviour exactly. Use only the Rust standard library. "
    "Avoid `unsafe`. Translate EVERY line of logic you are given; do not simplify, skip "
    "features or invent values. Reply with Rust code in a single ```rust block and nothing else."
)

STEP = """We are translating the C INI parser "inih" to Rust step by step, using the DEFAULT
settings from ini.h (no extra -D flags).

=== ini.h (settings and types, for reference) ===
{ini_h}

=== Rust code written so far (do NOT repeat it; your code must work with it) ===
{rust_so_far}

=== C code to translate in THIS step ===
{c_unit}

Translate ONLY the C code of this step into Rust: the function(s) {names}, plus any
`use` lines, types or constants they need that do not exist yet. {extra}
Reply with only the new Rust code in one ```rust block."""

EXTRA = {
    "test_driver": "The program's output must be byte-for-byte identical to the C test program: "
                   "same files in the same order, same printf formats, same counter values. "
                   "Global C variables may become fields of a struct or mutable locals.",
}

# --- prompt v2: targets the q7b_fbf_2 error categories -----------------------------
# (a) E0425 on C macros  -> the harness defines them itself, before step 1
# (b) raw-pointer / cast errors (E0599, E0606) -> explicit "no raw pointers" rule
# (c) cross-step type/argument mismatches (E0308, E0061) -> list exact existing signatures
PRELUDE_V2 = """// constants from ini.h / ini.c (DEFAULT settings), defined by the harness
const INI_HANDLER_LINENO: bool = false;
const INI_ALLOW_MULTILINE: bool = true;
const INI_ALLOW_BOM: bool = true;
const INI_START_COMMENT_PREFIXES: &str = ";#";
const INI_ALLOW_INLINE_COMMENTS: bool = true;
const INI_INLINE_COMMENT_PREFIXES: &str = ";";
const INI_MAX_LINE: usize = 200;
const INI_MAX_SECTION: usize = 50;
const INI_MAX_NAME: usize = 50;
const INI_STOP_ON_FIRST_ERROR: bool = false;
const INI_CALL_HANDLER_ON_NEW_SECTION: bool = false;
const INI_ALLOW_NO_VALUE: bool = false;"""

RULES_V2 = """
Rules for this step:
- C strings (`char*`, `const char*`, char arrays) become `&str`, `String`, or `&[u8]` / `Vec<u8>`
  with integer indices. NEVER use raw pointers (`*const`, `*mut`) and never cast pointers with `as`.
- The constants and functions listed under "Already defined" EXIST. Do not define them again.
  Call them with exactly the signatures shown.
- Already defined:
{signatures}"""

ITEM_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe|extern\s+\"C\")\s+)*"
                     r"(fn|const|static(?:\s+mut)?|struct|enum|type|trait|union|mod)\s+(\w+)")


def split_items(code):
    """Split Rust source into top-level items (comments/attributes stay attached to the item
    that follows). An item ends at ';' or a closing brace at bracket depth 0. Strings, char
    literals and comments are skipped so their brackets do not count."""
    items, start, depth, i, n = [], 0, 0, 0, len(code)
    while i < n:
        c = code[i]
        if code.startswith("//", i):
            j = code.find("\n", i)
            i = n if j < 0 else j
            continue
        if code.startswith("/*", i):
            j = code.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        if c == '"':
            i += 1
            while i < n and code[i] != '"':
                i += 2 if code[i] == "\\" else 1
            i += 1
            continue
        if c == "'":
            m = re.match(r"'(?:\\.|\\u\{[0-9a-fA-F]+\}|[^\\'])'", code[i:])
            i += m.end() if m else 1          # char literal, or a lifetime like 'a
            continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0 and c == "}":
                items.append(code[start:i + 1])
                start = i + 1
        elif c == ";" and depth == 0:
            items.append(code[start:i + 1])
            start = i + 1
        i += 1
    if code[start:].strip():
        items.append(code[start:])
    return items


def item_key(item):
    """('fn', 'ini_rstrip') for named items, None for impl blocks, macros and anything unknown."""
    body = "\n".join(ln for ln in item.splitlines()
                     if ln.strip() and not ln.strip().startswith(("//", "#[", "#!")))
    m = ITEM_RE.match(body)
    if not m:
        return None
    kind = "static" if m.group(1).startswith("static") else m.group(1)
    return kind, m.group(2)


def signatures(pieces):
    """One line per top-level fn/const/static/struct/enum/type already written."""
    out, seen = [], set()
    for _, code in pieces:
        for it in split_items(code):
            key = item_key(it)
            if not key or key in seen:
                continue
            seen.add(key)
            body = "\n".join(ln for ln in it.splitlines()
                             if ln.strip() and not ln.strip().startswith(("//", "#[")))
            head = body.split("{")[0] if key[0] in ("fn", "struct", "enum", "trait") else body
            out.append("  " + " ".join(head.split()).rstrip(" =;"))
    return "\n".join(out) or "  (nothing yet)"


def owner_of(name):
    """Which step is responsible for a Rust name (by its C name, with or without the ini_ prefix)."""
    for step, _, c_names in UNITS:
        for c in c_names:
            if name in (c, c.removeprefix("ini_")):
                return step
    return None


def dedup(pieces):
    """pieces = [(step, code)]. For names defined more than once keep ONE definition:
    the one from the step that owns the name (helpers own ini_rstrip, test_driver owns main),
    otherwise the first one. Returns (pieces, list of removed 'kind name (step)')."""
    seen = {}                                       # key -> step whose definition is kept
    for step, code in pieces:
        for it in split_items(code):
            key = item_key(it)
            if key and key not in seen:
                seen[key] = step
            elif key and owner_of(key[1]) == step:
                seen[key] = step                     # owner wins over an earlier copy
    kept_once, removed, out = set(), [], []
    for step, code in pieces:
        parts = []
        for it in split_items(code):
            key = item_key(it)
            if key and (seen[key] != step or key in kept_once):
                removed.append(f"{key[0]} {key[1]} ({step})")
                continue
            if key:
                kept_once.add(key)
            parts.append(it)
        out.append((step, "".join(parts)))
    return out, removed

def extract_c_functions(text, names):
    """Return the source of the named top-level C functions, in file order."""
    lines = text.splitlines()
    found = []
    for i, line in enumerate(lines):
        m = re.match(r"^(?:static\s+)?[A-Za-z_][\w\s\*]*?\b(\w+)\s*\(", line)
        if not m or m.group(1) not in names or line.rstrip().endswith(";"):
            continue
        # include preprocessor lines directly above (e.g. #if INI_HANDLER_LINENO variants)
        start = i
        while start > 0 and lines[start - 1].startswith("#"):
            start -= 1
        depth, seen_open, j = 0, False, i
        while j < len(lines):
            depth += lines[j].count("{") - lines[j].count("}")
            seen_open = seen_open or "{" in lines[j]
            if seen_open and depth == 0:
                break
            j += 1
        found.append("\n".join(lines[start:j + 1]))
    return "\n\n".join(found)


def error_summary(stderr):
    """Count compiler errors per code, e.g. '  26 E0428 the name `x` is defined multiple times'."""
    counts = {}
    for ln in stderr.splitlines():
        m = re.match(r"^error(?:\[(E\d+)\])?: (.*)", ln)
        if m and "aborting due to" not in ln and "could not compile" not in ln:
            k = m.group(1) or "-----"
            counts.setdefault(k, [0, re.sub(r"`[^`]*`", "`_`", m.group(2))])[0] += 1
    return "\n".join(f"  {n:4d} {k} {msg}" for k, (n, msg) in
                     sorted(counts.items(), key=lambda kv: -kv[1][0]))


def assemble(pieces):
    """Join the step outputs [(step, code)] into one file. Single-line `use` statements are
    hoisted and de-duplicated (each step may repeat the same imports); everything else is kept."""
    uses, bodies = [], []
    for step, piece in pieces:
        piece = f"// ===== step: {step} =====\n{piece}"
        kept = []
        for line in piece.splitlines():
            if re.match(r"^\s*use\s+[^;{]+;\s*$", line):
                if line.strip() not in uses:
                    uses.append(line.strip())
            else:
                kept.append(line)
        bodies.append("\n".join(kept).strip())
    return "\n".join(uses) + "\n\n" + "\n\n".join(bodies) + "\n"


def preamble(text):
    """Everything in a C file before its first function body (includes, #defines, types)."""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith("{") or re.match(r"^(?:static\s+)?\w[\w\s\*]*\(.*\)\s*\{?\s*$", line) and not line.rstrip().endswith(";"):
            j = i
            while j > 0 and lines[j - 1].startswith("#"):
                j -= 1
            return "\n".join(lines[:j]).strip()
    return ""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--model", required=True)
    ap.add_argument("--name", required=True)
    ap.add_argument("--fix-iters", type=int, default=0, help="feedback rounds on the assembled program")
    ap.add_argument("--timeout", type=int, default=7200, help="seconds per model call")
    ap.add_argument("--prompt", choices=["v1", "v2"], default="v1",
                    help="v2 = harness-defined constants + no-raw-pointer rule + existing signatures")
    ap.add_argument("--dedup", action="store_true",
                    help="drop duplicate top-level definitions before building")
    args = ap.parse_args()
    variant = "fbf" + ("-v2" if args.prompt == "v2" else "") + ("-dedup" if args.dedup else "")
    sys.stdout.reconfigure(line_buffering=True)

    ini_h = (H.INIH / "ini.h").read_text()
    project = H.OUT_DIR / args.name
    (project / "src").mkdir(parents=True, exist_ok=True)
    crate = re.sub(r"[^a-z0-9_]", "_", args.name.lower())
    (project / "Cargo.toml").write_text(H.CARGO_TOML.format(name=crate))
    pieces, tot_in, tot_out = [], 0, 0              # pieces = [(step, rust code)]
    if args.prompt == "v2":
        pieces.append(("prelude", PRELUDE_V2))

    def record(step, it, ok_build, errors, ok_par, n_diff, code, t_in, t_out, secs, reason):
        unsafe_n, crates = H.scan(code)
        H.log_row({
            "time": time.strftime("%Y-%m-%d %H:%M"), "name": args.name, "model": args.model,
            "mode": f"{variant}:{step}", "iteration": it, "compiled": ok_build, "compile_errors": len(errors),
            "parity_ok": ok_par, "lines_differ": n_diff if ok_build else "",
            "unsafe_count": unsafe_n, "external_crates": " ".join(crates),
            "prompt_tokens": t_in, "output_tokens": t_out, "cum_tokens": tot_in + tot_out,
            "seconds": round(secs, 1), "truncated": reason == "length", "code_lines": code.count("\n"),
        })

    # --- 1. translate unit by unit -------------------------------------------------
    for step, src_file, names in UNITS:
        text = (H.INIH / src_file).read_text()
        if step == "test_driver":
            c_unit = text  # small file; its globals (User, Prev_section) matter
        else:
            # file preamble (#defines such as MAX_SECTION) + the functions of this step
            c_unit = preamble(text) + "\n\n" + extract_c_functions(text, set(names))
        if not c_unit.strip():
            print(f"[{args.name}] could not find {names} in {src_file}; aborting")
            return
        extra = EXTRA.get(step, "")
        if args.prompt == "v2":
            extra += RULES_V2.format(signatures=signatures(pieces))
        prompt = STEP.format(ini_h=ini_h, rust_so_far=assemble(pieces).strip() or "(nothing yet)",
                             c_unit=c_unit, names=", ".join(names), extra=extra)
        print(f"[{args.name}] step {step}: {len(c_unit.splitlines())} C lines -> asking {args.model} ...")
        try:
            reply, t_in, t_out, secs, reason = H.chat(args.model, [{"role": "system", "content": SYSTEM},
                                                                   {"role": "user", "content": prompt}], args.timeout)
        except Exception as exc:
            print(f"  model call failed: {exc}")
            return
        tot_in += t_in
        tot_out += t_out
        code = H.extract_rust(reply)
        (project / f"step_{step}.md").write_text(reply)
        pieces.append((step, code.strip()))
        record(step, 1, "", [], "", "", code, t_in, t_out, secs, reason)
        print(f"  {code.count(chr(10))} Rust lines | tokens in={t_in} out={t_out} | {secs:.0f}s"
              f"{' | ANSWER CUT OFF' if reason == 'length' else ''}")

    # --- 2. assemble, build, check; optional fix loop -----------------------------
    def evaluate(code):
        (project / "src" / "main.rs").write_text(code)
        ok_build, errors, stderr = H.build(project)
        ok_par, n_diff, par_out = False, 0, ""
        if ok_build:
            ok_par, n_diff, par_out = H.parity(project / "target" / "release" / crate)
        status = ("PARITY OK" if ok_par else f"built, {n_diff} lines differ") if ok_build \
            else f"build failed, {len(errors)} errors"
        return ok_build, errors, stderr, ok_par, n_diff, par_out, status

    if args.dedup:
        pieces, removed = dedup(pieces)
        print(f"[{args.name}] dedup removed {len(removed)} duplicate definitions: {', '.join(removed) or '-'}")
    code = assemble(pieces)
    ok_build, errors, stderr, ok_par, n_diff, par_out, status = evaluate(code)
    record("assembled", 0, ok_build, errors, ok_par, n_diff, code, 0, 0, 0, "")
    print(f"[{args.name}] assembled: {status} | {code.count(chr(10))} Rust lines")
    print(error_summary(stderr))

    messages = [{"role": "system", "content": H.SYSTEM},
                {"role": "user", "content": "Here is a Rust translation of the C INI parser inih "
                 "and its test program:\n```rust\n" + code + "```"},
                {"role": "assistant", "content": "```rust\n" + code + "```"}]
    for k in range(1, args.fix_iters + 1):
        if ok_par:
            break
        messages.append({"role": "user", "content":
                         H.FIX_BUILD.format(errors="\n".join(stderr.splitlines()[:80])) if not ok_build
                         else H.FIX_PARITY.format(diff="\n".join(par_out.splitlines()[1:41]))})
        try:
            reply, t_in, t_out, secs, reason = H.chat(args.model, messages, args.timeout)
        except Exception as exc:
            print(f"  model call failed: {exc}")
            break
        tot_in += t_in
        tot_out += t_out
        messages.append({"role": "assistant", "content": reply})
        new = H.extract_rust(reply)
        same = new.strip() == code.strip()
        code = new or code
        ok_build, errors, stderr, ok_par, n_diff, par_out, status = evaluate(code)
        record("fix", k, ok_build, errors, ok_par, n_diff, code, t_in, t_out, secs, reason)
        print(f"  fix {k}: {status} | tokens in={t_in} out={t_out} | {secs:.0f}s"
              f"{' | IDENTICAL CODE RETURNED' if same else ''}")

    print(f"done. total tokens: {tot_in + tot_out} (in {tot_in}, out {tot_out}). Code in {project}")


if __name__ == "__main__":
    main()
