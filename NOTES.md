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
