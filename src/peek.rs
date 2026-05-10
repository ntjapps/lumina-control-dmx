//! Read N bytes at a hex offset from a file and print as hex+ASCII rows.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub fn peek(file: &Path, offset: u64, len: usize) -> io::Result<()> {
    let mut f = File::open(file)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    print_hex(&buf, offset);
    if n < len {
        println!("(EOF after {n} bytes; requested {len})");
    }
    Ok(())
}

fn print_hex(buf: &[u8], base: u64) {
    let w = 16;
    for (i, chunk) in buf.chunks(w).enumerate() {
        let off = base + (i * w) as u64;
        let hex: String = chunk.iter().map(|b| format!("{:02X} ", b)).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' })
            .collect();
        println!("  {:08X}  {:<48}  {}", off, hex.trim_end(), ascii);
    }
}
