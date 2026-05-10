use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

mod carve;
mod diff;
mod dump;
mod find;
mod inspect;
mod peek;

fn usage() -> ExitCode {
    eprintln!(
        "lumina_control_dmx — Mini Pearl 512A toolkit\n\n\
         USAGE:\n  \
           lumina_control_dmx dump  <drive-letter> <output.img>\n  \
           lumina_control_dmx carve   <image>  [filename]  [output]\n  \
           lumina_control_dmx diff    <a.kkd>  <b.kkd>\n  \
           lumina_control_dmx inspect <kkd> [kkd ...]\n  \
           lumina_control_dmx peek    <file> <offset_hex> <len_dec>\n\n\
         EXAMPLES:\n  \
           lumina_control_dmx dump    E dump.img\n  \
           lumina_control_dmx carve   dumps/dump.imp A.KKD dumps/A.KKD\n  \
           lumina_control_dmx diff    dumps/A.KKD dumps/B.KKD\n  \
           lumina_control_dmx inspect dumps/BKP.KKD dumps/WP1.KKD dumps/WP2.KKD\n\n\
         NOTES:\n  \
           * `dump` raw-reads \\\\.\\X: on Windows. Run the terminal as Administrator.\n  \
           * `carve` parses FAT, locates the file's directory entry, and reads\n    \
             `size` bytes contiguously from the start cluster — bypassing the\n    \
             malformed cluster chain that makes Windows refuse to copy.\n  \
           * `dump` refuses C: as a safety check.\n"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return usage();
    }
    match args[1].as_str() {
        "dump" => {
            if args.len() != 4 {
                return usage();
            }
            let drive = &args[2];
            let out = PathBuf::from(&args[3]);
            match dump::dump_volume(drive, &out) {
                Ok(bytes) => {
                    println!("\nDone. Wrote {bytes} bytes to {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dump failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "find" => {
            if args.len() != 4 {
                return usage();
            }
            let file = PathBuf::from(&args[2]);
            let pattern = &args[3];
            match find::find(&file, pattern) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("find failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "diff" => {
            if args.len() != 4 {
                return usage();
            }
            let a = PathBuf::from(&args[2]);
            let b = PathBuf::from(&args[3]);
            match diff::diff(&a, &b) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("diff failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "carve" => {
            if args.len() < 3 || args.len() > 5 {
                return usage();
            }
            let image = PathBuf::from(&args[2]);
            let target = args.get(3).cloned().unwrap_or_else(|| "C.KKD".to_string());
            let out = args
                .get(4)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(&target));
            match carve::carve(&image, &target, &out) {
                Ok(bytes) => {
                    println!("\nWrote {bytes} bytes to {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("carve failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "peek" => {
            if args.len() != 5 {
                return usage();
            }
            let file = PathBuf::from(&args[2]);
            let offset = match u64::from_str_radix(args[3].trim_start_matches("0x"), 16) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("invalid hex offset: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let len: usize = match args[4].parse() {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("invalid length: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match peek::peek(&file, offset, len) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("peek failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "inspect" => {
            if args.len() < 3 {
                return usage();
            }
            let files: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            match inspect::inspect(&files) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("inspect failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}
