use std::str::FromStr;
use std::fs::File;
use std::path::Path;

// ===== step: prelude =====
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

// ===== step: helpers =====
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

// ===== step: parse_stream =====
use std::io::{self, Read};

type IniHandler = dyn FnMut(&str, &str, &str) -> bool;

fn ini_parse_stream<R: Read>(
    reader: R,
    handler: &mut IniHandler,
    user: &mut dyn FnMut(&str, &str, &str),
) -> io::Result<usize> {
    let mut line = String::with_capacity(INI_MAX_LINE);
    let mut max_line = INI_MAX_LINE;
    let mut section = String::with_capacity(INI_MAX_SECTION);
    let mut prev_name = String::with_capacity(INI_MAX_NAME);
    let mut offset = 0;
    let mut start = "";
    let mut end = "";
    let mut name = "";
    let mut value = "";
    let mut lineno = 0;
    let mut error = 0;
    let mut abyss = vec![0u8; 16];
    let mut abyss_len = 0;

    let mut reader = reader.bytes();

    while let Some(byte) = reader.next() {
        let byte = byte?;
        line.push(byte as char);
        offset += 1;

        if offset == max_line - 1 && line.ends_with('\n') {
            break;
        }

        if offset == max_line - 1 && !line.ends_with('\n') {
            while let Some(byte) = reader.next() {
                let byte = byte?;
                if byte == b'\n' {
                    break;
                }
                abyss[abyss_len] = byte;
                abyss_len += 1;
            }
        }

        lineno += 1;

        if line.starts_with(&[0xEF, 0xBB, 0xBF]) && lineno == 1 {
            line.drain(..3);
        }

        start = &line[0..offset];
        start = ini_rstrip(ini_lskip(start), &line);

        if INI_START_COMMENT_PREFIXES.contains(start.chars().next().unwrap_or(' ')) {
            // Start-of-line comment
        } else if !prev_name.is_empty() && !start.is_empty() && start > &line {
            if INI_ALLOW_INLINE_COMMENTS {
                end = ini_find_chars_or_comment(start, None);
                line.drain(end.len()..);
                ini_rstrip(start, end);
            }
            if !handler(user, &section, &prev_name, start) && error == 0 {
                error = lineno;
            }
        } else if start.starts_with('[') {
            end = ini_find_chars_or_comment(&start[1..], Some("]"));
            if end.starts_with(']') {
                *end = '\0';
                section.clear();
                section.push_str(&start[1..end.len() - 1]);
                prev_name.clear();
                if INI_CALL_HANDLER_ON_NEW_SECTION {
                    if !handler(user, &section, "", "") && error == 0 {
                        error = lineno;
                    }
                }
            } else if error == 0 {
                error = lineno;
            }
        } else if !start.is_empty() {
            end = ini_find_chars_or_comment(start, Some("=:"));
            if end.starts_with('=') || end.starts_with(':') {
                *end = '\0';
                name = ini_rstrip(start, end);
                value = &end[1..];
                if INI_ALLOW_INLINE_COMMENTS {
                    end = ini_find_chars_or_comment(value, None);
                    *end = '\0';
                }
                value = ini_lskip(value);
                ini_rstrip(value, end);

                prev_name.clear();
                prev_name.push_str(name);
                if !handler(user, &section, name, value) && error == 0 {
                    error = lineno;
                }
            } else {
                if INI_ALLOW_NO_VALUE {
                    *end = '\0';
                    name = ini_rstrip(start, end);
                    if !handler(user, &section, name, "") && error == 0 {
                        error = lineno;
                    }
                } else if error == 0 {
                    error = lineno;
                }
            }
        }

        if INI_STOP_ON_FIRST_ERROR && error != 0 {
            break;
        }
    }

    Ok(error)
}

// ===== step: file_api =====
use std::io::{self, BufRead, BufReader, Read};

fn ini_parse_file<R: Read>(
    file: R,
    handler: &mut IniHandler,
    user: &mut dyn FnMut(&str, &str, &str),
) -> io::Result<usize> {
    ini_parse_stream(file, handler, user)
}

fn ini_parse(filename: &str, handler: &mut IniHandler, user: &mut dyn FnMut(&str, &str, &str)) -> io::Result<usize> {
    let file = File::open(filename)?;
    ini_parse_file(BufReader::new(file), handler, user)
}

// ===== step: test_driver =====
use std::io::{self, BufRead, BufReader, Read};

// Global variables
static mut User: i32 = 100;
static mut Prev_section: String = String::new();

fn dumper(user: &mut i32, section: &str, name: &str, value: &str) -> bool {
    *user = 100; // Reset User for each call
    if name.is_empty() || section != Prev_section {
        println!("... [{}]", section);
        Prev_section.clear();
        Prev_section.push_str(section);
    }
    if name.is_empty() {
        return true;
    }

    println!("... {}{}{};", name, if value.is_empty() { "" } else { "=" }, value);

    if value.is_empty() {
        return true;
    }

    *user += 1;
    *user == 100
}

fn parse(fname: &str) -> io::Result<()> {
    let mut user = 100;
    *Prev_section = String::new();
    let e = ini_parse(fname, &mut dumper, &mut |section, name, value| {
        println!("{}: e={} user={}", fname, e, user);
        user += 1;
        Ok(())
    });
    Ok(())
}

fn main() -> io::Result<()> {
    parse("no_file.ini")?;
    parse("normal.ini")?;
    parse("bad_section.ini")?;
    parse("bad_comment.ini")?;
    parse("user_error.ini")?;
    parse("multi_line.ini")?;
    parse("bad_multi.ini")?;
    parse("bom.ini")?;
    parse("duplicate_sections.ini")?;
    parse("no_value.ini")?;
    parse("long_section.ini")?;
    parse("long_line.ini")?;
    parse("name_only_after_error.ini")?;
    Ok(())
}
