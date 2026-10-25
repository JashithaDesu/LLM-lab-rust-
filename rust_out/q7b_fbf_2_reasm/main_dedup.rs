use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null_mut;
use std::fs::File;
use std::io::BufRead;

// ===== step: helpers =====

fn ini_rstrip(s: &mut CStr, end: &mut *const c_char) -> &mut CStr {
    while *end > s.as_ptr() && s[*end as usize - 1].is_ascii_whitespace() {
        unsafe {
            *end = *end.offset(-1);
        }
    }
    s
}

fn ini_lskip(s: *const c_char) -> *const c_char {
    let mut s = s;
    while *s != 0 && s.as_char().unwrap().is_ascii_whitespace() {
        s = unsafe { s.offset(1) };
    }
    s
}

fn ini_find_chars_or_comment(s: *const c_char, chars: Option<&CStr>) -> *const c_char {
    let mut s = s;
    while *s != 0 {
        if let Some(chars) = chars {
            if chars.to_bytes().contains(&*s as u8) {
                break;
            }
        }
        if INI_ALLOW_INLINE_COMMENTS == 1 {
            let was_space = s.as_char().unwrap().is_ascii_whitespace();
            if was_space && INI_INLINE_COMMENT_PREFIXES.to_bytes().contains(&*s as u8) {
                break;
            }
        }
        s = unsafe { s.offset(1) };
    }
    s
}

fn ini_strncpy0(dest: &mut [c_char], src: *const c_char, size: usize) -> &mut [c_char] {
    let mut i = 0;
    while i < size - 1 && unsafe { *src.add(i) } != 0 {
        dest[i] = unsafe { *src.add(i) };
        i += 1;
    }
    dest[i] = 0;
    dest
}

// ===== step: parse_stream =====

const INI_MAX_LINE: usize = 200;
const INI_MAX_SECTION: usize = 50;
const INI_MAX_NAME: usize = 50;
const INI_ALLOW_MULTILINE: usize = 1;
const INI_ALLOW_INLINE_COMMENTS: usize = 1;
const INI_START_COMMENT_PREFIXES: &str = ";#";
const INI_STOP_ON_FIRST_ERROR: usize = 0;

fn ini_parse_stream(
    reader: ini_reader,
    stream: *mut std::ffi::c_void,
    handler: ini_handler,
    user: *mut std::ffi::c_void,
) -> isize {
    let mut line = vec![0 as c_char; INI_MAX_LINE];
    let mut max_line = INI_MAX_LINE;
    let mut section = [0 as c_char; INI_MAX_SECTION];
    let mut prev_name = [0 as c_char; INI_MAX_NAME];
    let mut offset: usize = 0;
    let mut start: *const c_char = null_mut();
    let mut end: *const c_char = null_mut();
    let mut name: *const c_char = null_mut();
    let mut value: *const c_char = null_mut();
    let mut lineno: isize = 0;
    let mut error: isize = 0;
    let mut abyss = [0 as c_char; 16];
    let mut abyss_len: usize = 0;

    assert!(reader.is_some());
    assert!(stream.is_some());
    assert!(handler.is_some());

    while unsafe { reader.unwrap()(line.as_mut_ptr(), max_line as i32, stream) } != null_mut() {
        offset = line.len() - 1;

        if INI_ALLOW_REALLOC == 1 && !INI_USE_STACK == 1 {
            while max_line < INI_MAX_LINE
                && offset == max_line - 1
                && line[offset - 1] != '\n' as c_char
            {
                max_line *= 2;
                if max_line > INI_MAX_LINE {
                    max_line = INI_MAX_LINE;
                }
                let new_line = unsafe { std::alloc::realloc(line.as_mut_ptr() as *mut u8, max_line) };
                if new_line.is_null() {
                    unsafe { std::alloc::free(line.as_mut_ptr() as *mut u8) };
                    return -2;
                }
                line = unsafe { std::slice::from_raw_parts_mut(new_line as *mut c_char, max_line) };
                if unsafe { reader.unwrap()(line.as_mut_ptr() + offset, (max_line - offset) as i32, stream) } == null_mut() {
                    break;
                }
                offset += line.len() - 1;
            }
        }

        lineno += 1;

        if offset == max_line - 1 && line[offset - 1] != '\n' as c_char {
            while unsafe { reader.unwrap()(abyss.as_mut_ptr(), abyss.len() as i32, stream) } != null_mut() {
                if error == 0 {
                    error = lineno;
                }
                abyss_len = abyss.len() - 1;
                if abyss_len > 0 && abyss[abyss_len - 1] == '\n' as c_char {
                    break;
                }
            }
        }

        start = line.as_ptr();
        if INI_ALLOW_BOM == 1 && lineno == 1 {
            if unsafe { *start as u8 } == 0xEF && unsafe { *(start.add(1)) as u8 } == 0xBB && unsafe { *(start.add(2)) as u8 } == 0xBF {
                start = unsafe { start.add(3) };
            }
        }
        start = ini_rstrip(ini_lskip(start), line.as_mut_ptr().add(offset));

        if unsafe { *start as c_char } == ';' as c_char || unsafe { *start as c_char } == '#' as c_char {
            // Start-of-line comment
        } else if *prev_name != 0 as c_char && *start != 0 as c_char && start > line.as_ptr() {
            if INI_ALLOW_INLINE_COMMENTS == 1 {
                end = ini_find_chars_or_comment(start, None);
                unsafe { *end = '\0' as c_char };
                ini_rstrip(start, end);
            }
            // Non-blank line with leading whitespace, treat as continuation
            // of previous name's value (as per Python configparser).
            if unsafe { handler.unwrap()(user, section.as_ptr(), prev_name.as_ptr(), start) } == 0 && error == 0 {
                error = lineno;
            }
        } else if unsafe { *start as c_char } == '[' as c_char {
            // A "[section]" line
            end = ini_find_chars_or_comment(start.add(1), Some(CStr::from_bytes_with_nul(b"]\0").unwrap()));
            if unsafe { *end as c_char } == ']' as c_char {
                unsafe { *end = '\0' as c_char };
                ini_strncpy0(section.as_mut_ptr(), start.add(1), INI_MAX_SECTION);
                if INI_ALLOW_MULTILINE == 1 {
                    *prev_name.as_mut_ptr() = 0 as c_char;
                }
                if INI_CALL_HANDLER_ON_NEW_SECTION == 1 {
                    if unsafe { handler.unwrap()(user, section.as_ptr(), null_mut(), null_mut()) } == 0 && error == 0 {
                        error = lineno;
                    }
                }
            } else if error == 0 {
                // No ']' found on section line
                error = lineno;
            }
        } else if *start != 0 as c_char {
            // Not a comment, must be a name[=:]value pair
            end = ini_find_chars_or_comment(start, Some(CStr::from_bytes_with_nul(b"=:\0").unwrap()));
            if unsafe { *end as c_char } == '=' as c_char || unsafe { *end as c_char } == ':' as c_char {
                unsafe { *end = '\0' as c_char };
                name = ini_rstrip(start, end);
                value = end.add(1);
                if INI_ALLOW_INLINE_COMMENTS == 1 {
                    end = ini_find_chars_or_comment(value, None);
                    unsafe { *end = '\0' as c_char };
                }
                value = ini_lskip(value);
                ini_rstrip(value, end);

                if INI_ALLOW_MULTILINE == 1 {
                    ini_strncpy0(prev_name.as_mut_ptr(), name, INI_MAX_NAME);
                }
                // Valid name[=:]value pair found, call handler
                if unsafe { handler.unwrap()(user, section.as_ptr(), name, value) } == 0 && error == 0 {
                    error = lineno;
                }
            } else {
                // No '=' or ':' found on name[=:]value line
                if INI_ALLOW_NO_VALUE == 1 {
                    unsafe { *end = '\0' as c_char };
                    name = ini_rstrip(start, end);
                    if unsafe { handler.unwrap()(user, section.as_ptr(), name, null_mut()) } == 0 && error == 0 {
                        error = lineno;
                    }
                } else if error == 0 {
                    error = lineno;
                }
            }
        }

        if INI_STOP_ON_FIRST_ERROR == 1 && error != 0 {
            break;
        }
    }

    if !INI_USE_STACK == 1 {
        unsafe { std::alloc::free(line.as_mut_ptr() as *mut u8) };
    }

    error
}

// ===== step: file_api =====

fn ini_parse_file(
    file: *mut std::ffi::c_void,
    handler: ini_handler,
    user: *mut std::ffi::c_void,
) -> isize {
    ini_parse_stream((ini_reader)fgets, file, handler, user)
}

fn ini_parse(
    filename: *const c_char,
    handler: ini_handler,
    user: *mut std::ffi::c_void,
) -> isize {
    let file = unsafe { std::fs::File::open(CStr::from_ptr(filename).to_str().unwrap()) };
    match file {
        Ok(file) => {
            let file_ptr = Box::into_raw(Box::new(file));
            let error = ini_parse_file(file_ptr as *mut std::ffi::c_void, handler, user);
            unsafe { Box::from_raw(file_ptr) };
            error
        }
        Err(_) => -1,
    }
}

// ===== step: test_driver =====
use std::io::{self, Read, Write};

fn dumper(user: *mut c_char, section: *const c_char, name: *const c_char, value: *const c_char, lineno: Option<i32>) -> i32 {
    let user = unsafe { &mut *user };
    if name.is_null() || section != Prev_section.as_ptr() {
        println!("... [{}]", CStr::from_ptr(section).to_str().unwrap());
        Prev_section.copy_from_slice(CStr::from_ptr(section).to_bytes());
        Prev_section[INI_MAX_SECTION - 1] = 0;
    }
    if name.is_null() {
        return 1;
    }

    let line_number = match lineno {
        Some(lineno) => format!(";  line {}", lineno),
        None => String::new(),
    };

    println!("... {}{}{}{}", CStr::from_ptr(name).to_str().unwrap(), if value.is_null() { "" } else { "=" }, if value.is_null() { "" } else { CStr::from_ptr(value).to_str().unwrap() }, line_number);

    if value.is_null() {
        return 1;
    }

    if CStr::from_ptr(name).to_str().unwrap() == "user" && CStr::from_ptr(value).to_str().unwrap() == "parse_error" {
        return 0;
    }

    1
}

fn parse(fname: *const c_char, user: *mut c_char) {
    static mut u: i32 = 100;
    let mut e: i32 = 0;

    unsafe {
        *Prev_section = 0;
        e = ini_parse(fname, dumper, user);
        println!("{}: e={} user={}", CStr::from_ptr(fname).to_str().unwrap(), e, *user);
        *user += 1;
    }
}

fn main() {
    unsafe {
        parse(CStr::from_bytes_with_nul(b"no_file.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"normal.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"bad_section.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"bad_comment.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"user_error.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"multi_line.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"bad_multi.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"bom.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"duplicate_sections.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"no_value.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"long_section.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"long_line.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
        parse(CStr::from_bytes_with_nul(b"name_only_after_error.ini\0").unwrap().as_ptr(), &mut User as *mut c_char);
    }
}
