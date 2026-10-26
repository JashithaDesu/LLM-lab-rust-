// constants from ini.h / ini.c (DEFAULT settings), defined by the harness
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
const INI_ALLOW_NO_VALUE: bool = false;
const INI_USE_STACK: bool = true;
const INI_ALLOW_REALLOC: bool = false;
const INI_INITIAL_ALLOC: usize = 200;
const INI_CUSTOM_ALLOCATOR: bool = false;

fn ini_rstrip(s: &str) -> &str { s.trim_end_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b') }
fn ini_lskip(s: &str) -> &str { s.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '\x0b') }
fn ini_find_chars_or_comment(s: &str, chars: Option<&str>) -> usize {
    let mut was_space = false;
    for (i, c) in s.char_indices() {
        if chars.map_or(false, |cs| cs.contains(c)) { return i; }
        if INI_ALLOW_INLINE_COMMENTS && was_space && INI_INLINE_COMMENT_PREFIXES.contains(c) { return i; }
        was_space = c.is_ascii_whitespace() || c == '\x0b';
    }
    s.len()
}
fn ini_strncpy0(src: &str, size: usize) -> String { src.chars().take(size - 1).collect() }

fn ini_parse_stream<R: std::io::BufRead>(mut reader: R, handler: &mut dyn FnMut(&str, &str, &str) -> bool) -> i32 {
    let (mut section, mut prev_name) = (String::new(), String::new());
    let (mut lineno, mut error) = (0i32, 0i32);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if reader.read_until(b'\n', &mut raw).unwrap_or(0) == 0 { break; }
        lineno += 1;
        if raw.len() > INI_MAX_LINE - 1 && !(raw.len() == INI_MAX_LINE - 1) {
            if raw.len() > INI_MAX_LINE - 1 && !(raw.len() == INI_MAX_LINE && raw[INI_MAX_LINE - 1] == b'\n') {
                raw.truncate(INI_MAX_LINE - 1);
                if error == 0 { error = lineno; }
            }
        }
        let mut bytes: &[u8] = &raw;
        if INI_ALLOW_BOM && lineno == 1 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { bytes = &bytes[3..]; }
        let line = String::from_utf8_lossy(bytes).into_owned();
        let start = ini_rstrip(ini_lskip(&line));
        let first = start.chars().next();
        if start.is_empty() || first.map_or(false, |c| INI_START_COMMENT_PREFIXES.contains(c)) {
        } else if INI_ALLOW_MULTILINE && !prev_name.is_empty() && line.len() != ini_lskip(&line).len() {
            let v = ini_rstrip(&start[..ini_find_chars_or_comment(start, None)]);
            if !handler(&section, &prev_name, v) && error == 0 { error = lineno; }
        } else if first == Some('[') {
            let rest = &start[1..];
            let end = ini_find_chars_or_comment(rest, Some("]"));
            if rest[end..].starts_with(']') { section = ini_strncpy0(&rest[..end], INI_MAX_SECTION); prev_name.clear(); }
            else if error == 0 { error = lineno; }
        } else {
            let end = ini_find_chars_or_comment(start, Some("=:"));
            if end < start.len() && (start[end..].starts_with('=') || start[end..].starts_with(':')) {
                let name = ini_rstrip(&start[..end]);
                let after = &start[end + 1..];
                let value = ini_rstrip(ini_lskip(&after[..ini_find_chars_or_comment(after, None)]));
                prev_name = ini_strncpy0(name, INI_MAX_NAME);
                if !handler(&section, name, value) && error == 0 { error = lineno; }
            } else if error == 0 { error = lineno; }
        }
    }
    error
}
fn ini_parse_file(file: std::fs::File, handler: &mut dyn FnMut(&str, &str, &str) -> bool) -> i32 {
    ini_parse_stream(std::io::BufReader::new(file), handler)
}
fn ini_parse(filename: &str, handler: &mut dyn FnMut(&str, &str, &str) -> bool) -> i32 {
    match std::fs::File::open(filename) { Ok(f) => ini_parse_file(f, handler), Err(_) => -1 }
}
fn main() {
    let mut user_shown = 0; let mut u = 100;
    for file in ["no_file.ini", "normal.ini", "bad_section.ini", "bad_comment.ini", "user_error.ini", "multi_line.ini",
                 "bad_multi.ini", "bom.ini", "duplicate_sections.ini", "no_value.ini", "long_section.ini", "long_line.ini",
                 "name_only_after_error.ini"] {
        let mut prev_section = String::new();
        let e = ini_parse(file, &mut |section: &str, name: &str, value: &str| {
            user_shown = u;
            if section != prev_section { println!("... [{}]", section); prev_section = ini_strncpy0(section, 50); }
            println!("... {}={};", name, value);
            !(name == "user" && value == "parse_error")
        });
        println!("{}: e={} user={}", file, e, user_shown);
        u += 1;
    }
}
