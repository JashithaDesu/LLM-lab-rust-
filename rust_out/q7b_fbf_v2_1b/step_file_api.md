```rust
use std::fs::File;
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
```