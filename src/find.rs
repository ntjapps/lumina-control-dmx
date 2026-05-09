use std::fs;
use std::io;
use std::path::Path;

pub fn find(file: &Path, hex_pattern: &str) -> io::Result<()> {
    let pattern: Vec<u8> = hex_pattern
        .split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).expect("invalid hex byte"))
        .collect();
    if pattern.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty pattern",
        ));
    }
    let data = fs::read(file)?;
    let mut hits = 0usize;
    let mut i = 0;
    while i + pattern.len() <= data.len() {
        if data[i..i + pattern.len()] == pattern[..] {
            println!("  hit at 0x{:08X} ({})", i, i);
            hits += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    println!("({} hits)", hits);
    Ok(())
}
