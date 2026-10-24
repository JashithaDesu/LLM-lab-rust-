- Correction: E0308 errors come from ONE closure returning bool while the model's own
  signature requires Result<(), IniError> (not from several closures).

- Semantic bugs that would survive compilation (only parity testing catches these):
  1. hard-coded "user=101" instead of the C counter (fabricated output)
  2. format argument order wrong: prints "onevalue=" instead of "one=value"
  3. section header printed for every key (Prev_section logic dropped)
  4. inverted stop condition for user=parse_error
  5. missing file aborts program via `?` instead of printing e=-1
