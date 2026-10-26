```rust
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

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
```