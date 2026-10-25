```rust
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null_mut;

const INI_MAX_LINE: usize = 200;
const INI_MAX_SECTION: usize = 50;
const INI_MAX_NAME: usize = 50;
const INI_ALLOW_MULTILINE: usize = 1;
const INI_ALLOW_INLINE_COMMENTS: usize = 1;
const INI_START_COMMENT_PREFIXES: &str = ";#";
const INI_STOP_ON_FIRST_ERROR: usize = 0;

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
```