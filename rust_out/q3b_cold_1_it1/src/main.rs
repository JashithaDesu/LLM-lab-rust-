use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug)]
enum IniError {
    Io(io::Error),
    Parse(String),
}

fn parse_ini_file(path: &Path, handler: &dyn Fn(&str, &str, &str) -> Result<(), IniError>) -> Result<(), IniError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut section = String::new();
    let mut prev_name = String::new();

    for (lineno, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                section = line[1..end].to_string();
                prev_name.clear();
            } else {
                return Err(IniError::Parse(format!("Invalid section line: {}", line)));
            }
        } else {
            let mut parts = line.splitn(2, '=');
            let name = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            if name.is_empty() {
                if let Some(end) = line.find(':') {
                    name = line[..end].trim();
                    value = line[end + 1..].trim();
                } else {
                    return Err(IniError::Parse(format!("Invalid name-only line: {}", line)));
                }
            }

            if !value.is_empty() {
                handler(&section, name, value)?;
            } else if handler(&section, name, value).is_err() {
                return Err(IniError::Parse(format!("Error handling line: {}", line)));
            }

            prev_name = name.to_string();
        }
    }

    Ok(())
}

fn main() {
    let tests = vec![
        ("no_file.ini", |_, _, _| Ok(())),
        ("normal.ini", |section, name, value| {
            assert_eq!(section, "section1");
            assert_eq!(name, "key1");
            assert_eq!(value, "value1");
            Ok(())
        }),
        ("bad_section.ini", |_, _, _| Err(IniError::Parse("Invalid section line".to_string()))),
        ("bad_comment.ini", |_, _, _| Err(IniError::Parse("Invalid comment line".to_string()))),
        ("user_error.ini", |_, _, _| Err(IniError::Parse("User error".to_string()))),
        ("multi_line.ini", |section, name, value| {
            assert_eq!(section, "section1");
            assert_eq!(name, "key1");
            assert_eq!(value, "value1\nvalue2");
            Ok(())
        }),
        ("bad_multi.ini", |_, _, _| Err(IniError::Parse("Invalid multi-line value".to_string()))),
        ("bom.ini", |_, _, _| Ok(())),
        ("duplicate_sections.ini", |_, _, _| Ok(())),
        ("no_value.ini", |_, _, _| Ok(())),
        ("long_section.ini", |section, name, value| {
            assert_eq!(section, "section1");
            assert_eq!(name, "key1");
            assert_eq!(value, "value1");
            Ok(())
        }),
        ("long_line.ini", |section, name, value| {
            assert_eq!(section, "section1");
            assert_eq!(name, "key1");
            assert_eq!(value, "value1");
            Ok(())
        }),
        ("name_only_after_error.ini", |_, _, _| Ok(())),
    ];

    for (test_file, handler) in tests {
        let path = Path::new(test_file);
        match parse_ini_file(path, handler) {
            Ok(_) => println!("{}: Success", test_file),
            Err(e) => println!("{}: Error: {}", test_file, e),
        }
    }
}
