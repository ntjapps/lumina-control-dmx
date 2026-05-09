//! Byte-diff two carved `.KKD` files and print contiguous differing runs.
//!
//! When two saves differ by a single controlled change on the console, the
//! changed bytes cluster in a small number of regions. Each region's offset
//! and content is a clue toward decoding the file format.

use std::fs;
use std::io;
use std::path::Path;

const GAP: usize = 8; // merge runs separated by ≤ GAP equal bytes
const CONTEXT: usize = 8;

pub fn diff(a_path: &Path, b_path: &Path) -> io::Result<()> {
    let a = fs::read(a_path)?;
    let b = fs::read(b_path)?;
    if a.len() != b.len() {
        println!(
            "size differs: {} = {}, {} = {}",
            a_path.display(),
            a.len(),
            b_path.display(),
            b.len()
        );
    }
    let n = a.len().min(b.len());

    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        if a[i] != b[i] {
            let start = i;
            let mut last_diff = i;
            i += 1;
            while i < n {
                if a[i] != b[i] {
                    last_diff = i;
                    i += 1;
                } else {
                    // peek for another diff within GAP
                    let mut j = i;
                    let mut found = false;
                    while j < n && j - i < GAP {
                        if a[j] != b[j] {
                            found = true;
                            break;
                        }
                        j += 1;
                    }
                    if found {
                        i = j;
                    } else {
                        break;
                    }
                }
            }
            runs.push((start, last_diff + 1));
        } else {
            i += 1;
        }
    }

    let total_diff: usize = runs.iter().map(|(s, e)| e - s).sum();
    println!("file size: {} bytes", n);
    println!("differing runs: {}  (total {} bytes differ)", runs.len(), total_diff);
    println!();

    for (idx, (s, e)) in runs.iter().enumerate() {
        let len = e - s;
        let ctx_start = s.saturating_sub(CONTEXT);
        let ctx_end = (e + CONTEXT).min(n);
        println!(
            "── run {:>2}: offset 0x{:08X} ({:>10})  len {:>5}  ────────────────",
            idx + 1,
            s,
            s,
            len
        );
        print_hex_pair(&a[ctx_start..ctx_end], &b[ctx_start..ctx_end], ctx_start, *s, *e);
        println!();
    }

    Ok(())
}

fn print_hex_pair(a: &[u8], b: &[u8], base: usize, hl_start: usize, hl_end: usize) {
    let line_w = 16;
    for line in 0..(a.len() + line_w - 1) / line_w {
        let off = base + line * line_w;
        let lo = line * line_w;
        let hi = (lo + line_w).min(a.len());
        print!("  A {:08X}  ", off);
        emit_hex_line(&a[lo..hi], off, hl_start, hl_end);
        println!();
        print!("  B {:08X}  ", off);
        emit_hex_line(&b[lo..hi], off, hl_start, hl_end);
        println!();
    }
}

fn emit_hex_line(buf: &[u8], base: usize, hl_start: usize, hl_end: usize) {
    let mut hex = String::new();
    let mut ascii = String::new();
    for (i, &byte) in buf.iter().enumerate() {
        let off = base + i;
        let in_hl = off >= hl_start && off < hl_end;
        if in_hl {
            hex.push_str(&format!("[{:02X}]", byte));
        } else {
            hex.push_str(&format!(" {:02X} ", byte));
        }
        ascii.push(if (0x20..0x7F).contains(&byte) {
            byte as char
        } else {
            '.'
        });
    }
    print!("{:<64}  {}", hex, ascii);
}
