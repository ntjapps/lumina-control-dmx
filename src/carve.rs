//! Carve `C.KKD` out of a raw FAT16 volume image.
//!
//! The console writes the file with a deliberately broken FAT cluster chain
//! (Windows reports 0x80070570 on copy), but the directory entry still gives a
//! valid start cluster and file size. We trust those two fields and read the
//! file contiguously from the start cluster — the console writes its save as
//! one continuous run, which is why a chain walk isn't needed.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug)]
struct Bpb {
    bytes_per_sector: u32,
    sectors_per_cluster: u32,
    reserved_sectors: u32,
    num_fats: u32,
    root_entries: u32,
    sectors_per_fat: u32,
}

impl Bpb {
    fn parse(boot: &[u8]) -> io::Result<Self> {
        if boot.len() < 64 || boot[510] != 0x55 || boot[511] != 0xAA {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "boot sector signature 0x55AA missing — not a FAT volume?",
            ));
        }
        let u16le = |o: usize| u16::from_le_bytes([boot[o], boot[o + 1]]) as u32;
        Ok(Self {
            bytes_per_sector: u16le(11),
            sectors_per_cluster: boot[13] as u32,
            reserved_sectors: u16le(14),
            num_fats: boot[16] as u32,
            root_entries: u16le(17),
            sectors_per_fat: u16le(22),
        })
    }

    fn root_dir_offset(&self) -> u64 {
        let root_start_sector = self.reserved_sectors + self.num_fats * self.sectors_per_fat;
        root_start_sector as u64 * self.bytes_per_sector as u64
    }

    fn root_dir_bytes(&self) -> u64 {
        self.root_entries as u64 * 32
    }

    fn data_region_offset(&self) -> u64 {
        self.root_dir_offset() + self.root_dir_bytes()
    }

    fn cluster_size(&self) -> u64 {
        self.bytes_per_sector as u64 * self.sectors_per_cluster as u64
    }

    fn cluster_offset(&self, cluster: u32) -> u64 {
        // Clusters are numbered starting at 2.
        self.data_region_offset() + (cluster as u64 - 2) * self.cluster_size()
    }
}

#[derive(Debug)]
struct DirEntry {
    name: String,
    start_cluster: u32,
    size: u32,
    attr: u8,
}

fn parse_dir_entries(buf: &[u8]) -> Vec<DirEntry> {
    let mut out = Vec::new();
    for chunk in buf.chunks_exact(32) {
        let first = chunk[0];
        if first == 0x00 {
            break; // end of directory
        }
        if first == 0xE5 {
            continue; // deleted
        }
        let attr = chunk[11];
        if attr == 0x0F {
            continue; // LFN entry
        }
        if attr & 0x08 != 0 && attr & 0x10 == 0 {
            // volume label only — skip
            continue;
        }
        let name_raw = &chunk[0..8];
        let ext_raw = &chunk[8..11];
        let name = String::from_utf8_lossy(name_raw).trim_end().to_string();
        let ext = String::from_utf8_lossy(ext_raw).trim_end().to_string();
        let full = if ext.is_empty() {
            name
        } else {
            format!("{name}.{ext}")
        };
        let start_hi = u16::from_le_bytes([chunk[20], chunk[21]]) as u32; // FAT32 only
        let start_lo = u16::from_le_bytes([chunk[26], chunk[27]]) as u32;
        let start_cluster = (start_hi << 16) | start_lo;
        let size = u32::from_le_bytes([chunk[28], chunk[29], chunk[30], chunk[31]]);
        out.push(DirEntry {
            name: full,
            start_cluster,
            size,
            attr,
        });
    }
    out
}

pub fn carve(image: &Path, target_name: &str, out: &Path) -> io::Result<u64> {
    let mut f = File::open(image)?;

    let mut boot = [0u8; 512];
    f.read_exact(&mut boot)?;
    let bpb = Bpb::parse(&boot)?;
    println!("BPB: {bpb:?}");
    println!("  root dir offset: 0x{:X}", bpb.root_dir_offset());
    println!("  data region offset: 0x{:X}", bpb.data_region_offset());
    println!("  cluster size: {} bytes", bpb.cluster_size());

    f.seek(SeekFrom::Start(bpb.root_dir_offset()))?;
    let mut root = vec![0u8; bpb.root_dir_bytes() as usize];
    f.read_exact(&mut root)?;

    let entries = parse_dir_entries(&root);
    println!("\nRoot directory entries:");
    for e in &entries {
        println!(
            "  {:<14}  attr=0x{:02X}  cluster={:>6}  size={:>10}",
            e.name, e.attr, e.start_cluster, e.size
        );
    }

    let target_upper = target_name.to_ascii_uppercase();
    let entry = entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(&target_upper))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{target_name} not found in root directory"),
            )
        })?;

    if entry.size == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has size 0 in directory entry", entry.name),
        ));
    }

    let start_offset = bpb.cluster_offset(entry.start_cluster);
    println!(
        "\nCarving {} ({} bytes) from offset 0x{:X} (cluster {})",
        entry.name, entry.size, start_offset, entry.start_cluster
    );

    f.seek(SeekFrom::Start(start_offset))?;
    let mut payload = vec![0u8; entry.size as usize];
    f.read_exact(&mut payload)?;

    let mut dst = File::create(out)?;
    dst.write_all(&payload)?;
    dst.flush()?;
    Ok(entry.size as u64)
}
