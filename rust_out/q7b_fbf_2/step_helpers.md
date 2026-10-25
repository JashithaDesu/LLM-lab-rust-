```rust
use std::ffi::CStr;
use std::os::raw::c_char;

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
```