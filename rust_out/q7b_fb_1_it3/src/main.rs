use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

fn main() {
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
        match parse_file(file) {
            Ok(e) => println!("{}: e={} user=100", file, e),
            Err(e) => println!("{}: e={}", file, e),
        }
    }
}

fn parse_file<P: AsRef<Path>>(filename: P) -> io::Result<i32> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut user = 100;
    let mut prev_section = String::new();
    let mut multi_line_value = String::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        if line.starts_with(';') || line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                let section = &line[1..end];
                if !section.is_empty() {
                    prev_section = section.to_string();
                    multi_line_value.clear();
                }
            }
        } else {
            let mut parts: Vec<&str> = line.splitn(2, |c: char| c == '=' || c == ':').collect();
            if parts.len() == 2 {
                let name = parts[0].trim();
                let value = parts[1].trim();
                if name == "user" && value == "parse_error" {
                    return Ok(0);
                }
                if !multi_line_value.is_empty() {
                    multi_line_value.push('\n');
                }
                multi_line_value.push_str(value);
            } else if parts.len() == 1 {
                let name = parts[0].trim();
                if name == "user" && multi_line_value.is_empty() {
                    return Ok(0);
                }
            }
        }
    }

    Ok(1)
}
