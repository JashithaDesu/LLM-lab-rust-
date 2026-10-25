use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug)]
enum IniError {
    Io(io::Error),
    Parse(String),
}

impl From<io::Error> for IniError {
    fn from(e: io::Error) -> Self {
        IniError::Io(e)
    }
}

fn parse_ini_file(file: &mut BufReader<File>, handler: &dyn Fn(&str, &str, &str) -> bool) -> Result<(), IniError> {
    let mut section = String::new();
    let mut prev_name = String::new();

    for line in file.lines() {
        let line = line?;
        let mut line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                section = line[1..end].to_string();
                prev_name.clear();
            } else {
                return Err(IniError::Parse(format!("Invalid section: {}", line)));
            }
        } else {
            let mut parts = line.splitn(2, '=');
            let name = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            if handler(&section, name, value) {
                return Err(IniError::Parse(format!("Handler error: {}={}", name, value)));
            }

            prev_name = name.to_string();
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files = [
        "no_file.ini",
        "normal.ini",
        "bad_section.ini",
        "bad_comment.ini",
        "user_error.ini",
        "multi_line.ini",
        "bad_multi.ini",
        "bom.ini",
        "duplicate_sections.ini",
        "no_value.ini",
        "long_section.ini",
        "long_line.ini",
        "name_only_after_error.ini",
    ];

    for file in files {
        let path = Path::new(file);
        let file = File::open(path).map_err(|e| IniError::Io(e))?;
        let reader = BufReader::new(file);

        match parse_ini_file(&reader, &|section, name, value| {
            println!("... [{section}]");
            println!("... {}{}{};", name, value, if value.is_empty() { "" } else { "=" });
            name == "user" && value == "parse_error"
        }) {
            Ok(_) => println!("{}: e=0 user=101", file),
            Err(e) => println!("{}: e={} user=101", file, e),
        }
    }

    Ok(())
}
