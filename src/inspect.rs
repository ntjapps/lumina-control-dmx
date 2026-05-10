//! Inspect well-known fields across one or more `.KKD` files side-by-side.
//!
//! Reads the file(s) into memory and prints decoded values for the offsets
//! catalogued in `maps.md`. Useful for cross-checking diff hypotheses without
//! reaching for an external scripting language.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

struct Loaded {
    label: String,
    bytes: Vec<u8>,
}

fn label_for(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string()
}

fn ascii_or_dot(b: u8) -> char {
    if (0x20..0x7F).contains(&b) {
        b as char
    } else {
        '.'
    }
}

fn slice(buf: &[u8], off: usize, len: usize) -> Option<&[u8]> {
    if off + len <= buf.len() {
        Some(&buf[off..off + len])
    } else {
        None
    }
}

fn hex_ascii(buf: &[u8]) -> String {
    let h: String = buf.iter().map(|b| format!("{:02X} ", b)).collect();
    let a: String = buf.iter().map(|&b| ascii_or_dot(b)).collect();
    format!("{:<48}  \"{}\"", h.trim_end(), a)
}

fn print_field(loaded: &[Loaded], title: &str, off: usize, len: usize) {
    println!("── {title}  @ 0x{off:X}  ({len} B) ──");
    let label_w = loaded.iter().map(|l| l.label.len()).max().unwrap_or(3);
    for l in loaded {
        match slice(&l.bytes, off, len) {
            Some(s) => println!("  {:<w$}  {}", l.label, hex_ascii(s), w = label_w),
            None => println!(
                "  {:<w$}  (out of range — file size 0x{:X})",
                l.label,
                l.bytes.len(),
                w = label_w
            ),
        }
    }
    println!();
}

pub fn inspect(files: &[PathBuf]) -> io::Result<()> {
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inspect: provide at least one .KKD file",
        ));
    }

    let loaded: Vec<Loaded> = files
        .iter()
        .map(|p| -> io::Result<Loaded> {
            Ok(Loaded {
                label: label_for(p),
                bytes: fs::read(p)?,
            })
        })
        .collect::<io::Result<_>>()?;

    println!("=== file sizes & trailer offset ===");
    let label_w = loaded.iter().map(|l| l.label.len()).max().unwrap_or(3);
    for l in &loaded {
        let sz = l.bytes.len();
        let trailer = sz.saturating_sub(0x800);
        let trailer_head = slice(&l.bytes, trailer, 24)
            .map(hex_ascii)
            .unwrap_or_else(|| "(file too small)".into());
        println!(
            "  {:<w$}  size=0x{:X} ({})  trailer@0x{:X}  {}",
            l.label,
            sz,
            sz,
            trailer,
            trailer_head,
            w = label_w
        );
    }
    println!();

    // Header + show-name (10 ASCII bytes after "SHOWDATA" magic).
    print_field(&loaded, "SHOWDATA header + show-name", 0x00, 0x1E);

    // DMX-address table head (u16 LE per fixture_id, base ~0x1E).
    print_field(&loaded, "DMX-address table head (0x1E)", 0x1E, 32);

    // Personality counter array + mirror.
    print_field(&loaded, "Personality counters @ 0x9C", 0x9C, 6);
    print_field(&loaded, "Personality counters mirror @ 0x60840", 0x60840, 6);

    // Sorted fixture_id list head.
    print_field(&loaded, "Sorted fixture_id list head @ 0x498", 0x498, 32);

    // Per-fixture record-pointer table head.
    print_field(&loaded, "Fixture record-pointer table @ 0x4028", 0x4028, 32);

    // Canonical fixture->DMX patch table (60 × u16 LE).
    print_field(&loaded, "Fixture→DMX patch table @ 0x60878", 0x60878, 32);

    // Personality library — first slot's name.
    print_field(&loaded, "Personality slot 0 name @ 0x61000", 0x61000, 16);

    Ok(())
}
