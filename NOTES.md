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
- Repetition validity: q7b_fbf_2 and _3 give identical step outputs (48/206/82 lines): temperature 0 is greedy,
  so the fixed seed has no effect and warm repeats are copies. Only the cold run (fbf_1) differed.
  Real repetitions need --temperature > 0 with different --seed values. Added both options (defaults unchanged).
- q7b_fbf_3: identical to q7b_fbf_2 through file_api; aborted at test_driver by a reboot (21:15). q3b_fbf_1/2 never ran.
- v2_1 helpers (cold): 38 lines (v1: 48), all 4 functions present, raw pointers 4 -> 0, libc 0.
  Input tokens 3155 vs 2537 (+24% for prelude+rules). Time 898 s (v1 cold 857 s).
- Timing caveat: kind cluster (simple-faas) + Firefox were running during runs until 23:00 25 Oct; stopped mid v2_1 parse_stream. Step times are not a clean comparison; outputs unaffected.
- Timing caveat: kind cluster (simple-faas) + Firefox were running during runs until 23:00 25 Oct; stopped mid v2_1 parse_stream. Step times are not a clean comparison; outputs unaffected.
- v2_1 (prompt v2): parse_stream TIMED OUT at 7200 s (v1: ~2800 s). No partial output saved (pre-streaming harness).
  Hypotheses: repetition loop until num_predict, or very long output. Rerun as v2_1b with streaming,
  --timeout 10800 (so num_predict hits first -> 'ANSWER CUT OFF' = runaway), --keep-alive -1.
- v2_1b started 23:45 with streaming, --timeout 10800, --keep-alive 24h; model still loaded from v2_1 -> helpers runs WARM (v2_1 helpers was cold).
- Cold vs warm, same prompt (v2_1 vs v2_1b helpers): same 38 lines, 309 vs 307 tokens, but DIFFERENT code.
  Cold invented APIs (&str::truncate, chars().nth_mut); warm used INI_ALLOW_INLINE_COMMENTS/PREFIXES like the C code.
  Both push '\0' into a String (C terminator copied literally).
  => Line/token counts are NOT evidence of identical output. Earlier "fbf_2 == fbf_3" claim must be checked by diff.
- CONFIRMED by cmp: q7b_fbf_2 and _3 step files byte-identical (helpers, parse_stream, file_api). Both warm.
  At temperature 0 the only variation source observed is load state (cold vs warm). Independent repeats need temperature > 0.
## v2_1b (prompt v2 + dedup, warm, streaming, keep-alive 24h) - 26 Oct
- Assembled: 37 errors, 254 lines (v1: 109/455; v1+dedup: 56/301). Errors per 100 lines 24 -> 19 -> 15.
- v2 fixed: duplicate definitions 26 -> 0 (signature list worked where "do not repeat" failed),
  undefined names 32 -> 1, raw-pointer/cast errors 17 -> 0.
- Remaining: 10 argument-count mismatches (parse_stream takes `user` as a second closure), 6 import clashes
  (E0252, assemble() only dedups identical use lines), 3 static mut despite explicit rule, 18 types/traits/lifetimes.
- CAUTION: code shrank (parse_stream 206 -> 119, file_api 82 -> 15, test_driver 119 -> 56): check for dropped logic.
- parse_stream 1925 s warm vs >7200 s timeout cold (v2_1), but v2_1 also had kind cluster load: not a clean comparison.
- reassemble (fixed to include v2 prelude) reproduces v2_1b exactly: 37 errors / 254 lines.
  + merge-uses (all `use` imports expanded, de-duplicated): 37 -> 31, E0252 6 -> 0.
- Chain: v1 109 -> dedup 56 -> v2 prompt 37 -> merge-uses 31. Of the 78 removed errors, 59 came from
  deterministic tool fixes, not from the model.
- Prediction for q7b_fbf_v2_2 (warm, temp 0, same prompt): steps byte-identical to v2_1b -> assembled = 31 errors.
- v3 = v2 + interface-first contract (fixed Rust signatures) + behaviour checklist per step, hand-written from
  ini.c/unittest.c. Validated: contract_ref (implements ONLY the contract, written with AI help) -> PARITY OK 81/81.
  Threat: v3 measures model + human-written spec, not autonomous migration.
- Side finding: a line of exactly 199 chars + newline is an error in C but not in contract_ref; baseline still
  matches -> inih tests do not cover this boundary.
- CORRECTION: v2_1b did NOT drop multi-line values. Its parse_stream implements the continuation logic
  (prev_name non-empty + leading whitespace) without using the word "multiline"; the keyword grep (3 -> 0)
  measured vocabulary, not behaviour. Only output parity tests behaviour.
- Prediction confirmed: v2_2 steps byte-identical to v2_1b (helpers, parse_stream). Warm temp-0 repeats identical again.
- Ollama log shows llama.cpp prompt-cache reuse between calls (lcp / f_keep lines), e.g. file_api 143 s vs 651 s.
- v2_2 assembled: 30 errors / 254 lines (prediction from v2_1b: 31). Steps identical up to parse_stream;
  file_api differs (142 vs 147 tokens) and ran 143 s vs 651 s = prompt-cache hit (Ollama log: f_keep 0.655).
  test_driver then differs too (no static mut: E0133 3 -> 0).
  => temp-0 output depends on prompt-cache state; cold vs warm is one case of this.
- v2_2 fix 1 removed ONLY the 2 E0106 (missing lifetime); all 10 argument-count errors (E0061/E0057/E0593),
  7 E0277 and 6 E0308 unchanged. Compiler feedback repairs local errors, not cross-step interface mismatches
  (the message does not say which side - caller or callee - is wrong). Motivation for v3 interface contract.
- v2_2 fix 2: IDENTICAL CODE RETURNED (28 errors, 4355 s, 2012 output tokens wasted).
  Fix loop total: 30 -> 28 for 4024 output tokens / ~2.4 h. Run total 35,796 tokens.
  Workflow rule suggested: stop feedback loop when a round returns identical code or no error reduction.
