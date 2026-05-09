//! Raw volume dump for Windows.
//!
//! The Mini Pearl 512A writes `C.KKD` with a malformed FAT cluster chain, so
//! Windows refuses to copy it (error 0x80070570). The bytes are present on the
//! stick — we just have to read the volume at the block level and let a later
//! pass carve `C.KKD` out of the resulting image.
//!
//! On Windows, opening the path `\\.\X:` with read access (and admin rights)
//! gives a handle to the raw volume. `std::fs::File` handles that path fine;
//! reads must be sector-aligned, which a fixed-size buffer naturally satisfies.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

const SECTOR: usize = 512;
const CHUNK_SECTORS: usize = 2048; // 1 MiB per read
const CHUNK: usize = SECTOR * CHUNK_SECTORS;

pub fn dump_volume(drive_letter: &str, out_path: &Path) -> io::Result<u64> {
    let letter = parse_drive_letter(drive_letter)?;
    if letter == 'C' {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to dump C: — pass the USB drive letter",
        ));
    }

    let device = format!(r"\\.\{letter}:");
    println!("Opening {device} (raw volume read; requires Administrator)...");

    let mut src = OpenOptions::new().read(true).open(&device)?;
    let mut dst = File::create(out_path)?;

    let mut buf = vec![0u8; CHUNK];
    let mut total: u64 = 0;
    let started = Instant::now();
    let mut last_print = started;

    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        total += n as u64;

        let now = Instant::now();
        if now.duration_since(last_print).as_millis() >= 250 {
            print_progress(total, started);
            last_print = now;
        }
    }

    dst.flush()?;
    print_progress(total, started);
    Ok(total)
}

fn parse_drive_letter(s: &str) -> io::Result<char> {
    let trimmed = s.trim_end_matches(':').trim_end_matches('\\');
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Ok(c.to_ascii_uppercase()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected a single drive letter, got {s:?}"),
        )),
    }
}

fn print_progress(total: u64, started: Instant) {
    let secs = started.elapsed().as_secs_f64().max(0.001);
    let mib = total as f64 / (1024.0 * 1024.0);
    let rate = mib / secs;
    print!("\r  {mib:8.2} MiB  ({rate:6.1} MiB/s)");
    let _ = io::stdout().flush();
}
