- Correction: E0308 errors come from ONE closure returning bool while the model's own
  signature requires Result<(), IniError> (not from several closures).

- Semantic bugs that would survive compilation (only parity testing catches these):
  1. hard-coded "user=101" instead of the C counter (fabricated output)
  2. format argument order wrong: prints "onevalue=" instead of "one=value"
  3. section header printed for every key (Prev_section logic dropped)
  4. inverted stop condition for user=parse_error
  5. missing file aborts program via `?` instead of printing e=-1
- Verified: q7b_fb_1 iterations 1 and 3 are byte-identical (diff empty). The 7b model did not
  change its code at all after receiving the output diff.
## Repeat runs (2026-10-25), all conditions
- Parity: 0 of 15 attempts (3b plain 0/6, 7b plain 0/5, 3b feedback 0/2, 7b feedback 0/2).
- 7b compiles in 3/5 first answers; 3b never compiles (0/6).
- Feedback reproduced exactly: 3b stuck at 5 errors (both runs, ~21.06k tokens);
  7b returned byte-identical code each round (both runs, 94 lines differ, ~18.93k tokens).
- Outputs are bimodal, not random: 3b = {103 lines/16 errors, 77 lines/6 errors};
  7b = {71 lines/1 error, 75 lines/compiled}.
- Hypothesis: first call after model load gives answer A, later calls answer B
  (fits all 11 runs). Testing with forced cold starts (q3b_cold_1/2).
- CONFIRMED (2 forced cold starts, `ollama stop` before each): both gave 16 errors / 859 tokens,
  identical to the earlier cold run. Cold calls 3/3 -> answer A, warm calls 4/4 -> answer B.
  Output at temperature 0 depends on model load state, not on randomness.
  Implication: benchmark runs must control (or report) cold vs warm state.
## Function-by-function (fbf), qwen2.5-coder:7b
- q7b_fbf_1: file_api step timed out (>3600 s); prompt grows with all Rust written so far.
- q7b_fbf_2: 455 Rust lines (vs 71-75 whole-file): logic kept, but 109 compile errors:
  26 duplicate definitions across steps (E0428) despite "do not repeat" instruction;
  35 undefined values/types/functions (C macros/globals never defined);
  18 raw-pointer methods/invalid casts/derefs: char* translated literally to raw pointers;
  4 missing lifetimes; ~23 type/argument mismatches between steps.
- Step times (7b): helpers 8-14 min, parse_stream ~47 min, file_api 23 min, test_driver 40 min.
- Finding: decomposition fixes OMISSION but creates INTEGRATION and TRANSLITERATION errors.
## Deterministic dedup (reassemble.py, no model call), q7b_fbf_2
- as-is reproduced: 109 errors, 455 lines. dedup: 56 errors, 301 lines (26 definitions removed).
- 7b ignored "do NOT repeat" in 3/3 later steps: all 4 helpers re-written each time, constants re-declared.
- Repeated copies were WORSE: removing them also removed 27 non-duplicate errors
  (raw-pointer 18->6, E0308 18->12, lifetimes/indexing 8->2).
- Remaining 56: 32 E0425 (cannot find value), 12 E0308, 6 raw-pointer/cast, 6 other.
- Takeaway: ~half of fbf build errors came from naive assembly, not from translation.
- Note: q7b_fbf_1/2/3 and q3b_fbf_* were produced by harness_fbf v1 (not in git); v2 without --prompt/--dedup is equivalent apart from the default timeout (3600 -> 7200 s).
- The 32 E0425 after dedup = names used without bringing their definition across from C:
  17 C globals (User 13, Prev_section 4), 7 INI_* constants, 5 C typedefs (ini_handler, ini_reader),
  3 C library calls (free, fgets).
- Prompt v2 targets all four. Threat to validity: v2 partly prescribes the design (closure handler,
  struct/local for globals), so it measures model + engineered context, not raw model ability.
