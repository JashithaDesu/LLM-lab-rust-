```rust
use std::str::Chars;

fn ini_rstrip(s: &mut str) -> &mut str {
    let mut end = s.len();
    while end > 0 && s.chars().nth(end - 1).unwrap().is_whitespace() {
        end -= 1;
    }
    s.truncate(end);
    s
}

fn ini_lskip(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_whitespace())
}

fn ini_find_chars_or_comment(s: &str, chars: Option<&str>) -> &str {
    let mut chars = chars.unwrap_or("");
    let mut was_space = false;
    let mut chars_iter = chars.chars();
    let mut s_iter = s.chars();
    while let Some(c) = s_iter.next() {
        if chars.contains(c) || (was_space && chars_iter.any(|cc| cc == *c)) {
            break;
        }
        was_space = c.is_whitespace();
    }
    &s[..s_iter.as_str().chars().count()]
}

fn ini_strncpy0(dest: &mut str, src: &str, size: usize) -> &mut str {
    let mut i = 0;
    for c in src.chars().take(size - 1) {
        dest.chars().nth_mut(i).unwrap().replace(c);
        i += 1;
    }
    dest.chars().nth_mut(i).unwrap().replace('\0');
    dest
}
```