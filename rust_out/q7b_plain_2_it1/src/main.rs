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
        match File::open(file) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut user = 100;
                let mut prev_section = String::new();
                let mut error = 0;

                for (line_number, line) in reader.lines().enumerate() {
                    let line = line.unwrap();
                    let line = line.trim();

                    if line.starts_with(';') || line.is_empty() {
                        continue;
                    }

                    if line.starts_with('[') {
                        if let Some(end) = line.find(']') {
                            let section = &line[1..end];
                            if section != prev_section {
                                println!("... [{}]", section);
                                prev_section = section.to_string();
                            }
                        }
                    } else {
                        let mut parts = line.splitn(2, '=');
                        let name = parts.next().unwrap().trim();
                        let value = parts.next().map_or("", |v| v.trim());

                        if name.is_empty() {
                            continue;
                        }

                        if name == "user" && value == "parse_error" {
                            error = line_number + 1;
                        }

                        println!("... {}{}{};  line {}", name, if value.is_empty() { "" } else { "=" }, value, line_number + 1);
                    }
                }

                println!("{}: e={} user={}", file, error, user);
                user += 1;
            }
            Err(_) => {
                println!("{}: e=-1 user=100", file);
            }
        }
    }
}
