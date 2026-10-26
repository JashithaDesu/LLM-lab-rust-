```rust
fn ini_rstrip(s: &mut str) -> &mut str {
    let end = s.as_bytes().iter().rposition(|&c| !c.is_ascii_whitespace()).map_or(s.len(), |pos| pos + 1);
    &mut s[..end]
}

fn ini_lskip(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_ascii_whitespace())
}

fn ini_find_chars_or_comment(s: &str, chars: Option<&str>) -> &str {
    if INI_ALLOW_INLINE_COMMENTS {
        let mut was_space = false;
        let mut chars = chars.unwrap_or("");
        for (i, c) in s.char_indices() {
            if chars.contains(c) || (was_space && INI_INLINE_COMMENT_PREFIXES.contains(c)) {
                return &s[..i];
            }
            was_space = c.is_ascii_whitespace();
        }
        s
    } else {
        s.find(chars).map_or(s, |pos| &s[..pos])
    }
}

fn ini_strncpy0(dest: &mut str, src: &str, size: usize) -> &mut str {
    let mut i = 0;
    for (j, c) in src.char_indices() {
        if i < size - 1 {
            dest.push(c);
            i += 1;
        } else {
            break;
        }
    }
    dest.push('\0');
    dest
}
```