# LLM-lab-rust

Measuring how well local LLMs can migrate legacy C code to Rust while preserving behaviour.

**Source:** [inih](https://github.com/benhoyt/inih), a small C INI-file parser, plus its own test program.
**Parity check:** a translation counts as correct only if its output matches inih's version-controlled
test baseline (`baseline_multi.txt`, 81 lines) exactly. The checker is verified to accept the original C
and to reject a deliberately broken version.

**Measured per attempt:** compiles or not, number of compile errors, output parity, `unsafe` blocks,
invented crates, prompt/output tokens and time; plain prompting vs a compiler-and-test feedback loop.

Status: work in progress, results in progress. First observations are in [NOTES.md](NOTES.md).
