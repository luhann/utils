//! # writeback
//!
//! A high-performance utility to monitor the system's "Writeback" memory state via `/proc/meminfo`.
//!
//! ## Exit Codes
//! * `0`: Success (and if `--min-mb` is provided, writeback is above the threshold).
//! * `1`: Writeback memory is less than the `--min-mb` threshold, or a system error occurred.
//! * `2`: CLI argument parsing error.
//!
//! ## Examples
//! ```bash
//! # Print current writeback size in human-readable format
//! writeback
//!
//! # Run continuously as a daemon updating every 1 second (Perfect for Waybar)
//! writeback --daemon --interval 1
//! ```
use std::{
    fs::File,
    io::{self, Seek, Write},
    thread,
    time::Duration,
};

use clap::Parser;
use memchr::memmem;

#[derive(Parser, Debug)]
#[command(author, version, about = "A utility to monitor the system's 'Writeback' memory state via `/proc/meminfo`.", long_about = None)]
struct Args {
    /// Run continuously as a daemon
    #[arg(short, long, default_value_t = false)]
    daemon: bool,

    /// Polling interval in seconds when running in daemon mode
    #[arg(short, long, default_value_t = 2)]
    interval: u64,

    /// Suppress output while still using exit status behavior
    #[arg(short, long, default_value_t = false)]
    quiet: bool,

    /// Exit with code 1 if writeback is below this threshold (MiB)
    #[arg(short, long)]
    min_mb: Option<f64>,
}

/// Main entry point for binary
fn main() {
    let args = Args::parse();
    let delay = Duration::from_secs(args.interval);

    let mut file = match File::open("/proc/meminfo") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening /proc/meminfo: {e}");
            std::process::exit(1);
        }
    };

    loop {
        let _ = file.seek(io::SeekFrom::Start(0));

        // Pass the raw File descriptor directly.
        let size_kb = match parse_writeback_kb(&file) {
            Ok(value) => value,
            Err(e) => {
                let msg = match e.kind() {
                    io::ErrorKind::UnexpectedEof => "/proc/meminfo exceeded the 2KB buffer ceiling",
                    io::ErrorKind::NotFound => "Writeback entry not found",
                    _ => "Unknown I/O error",
                };
                eprintln!("Error parsing meminfo: {msg}");
                if args.daemon {
                    thread::sleep(delay);
                    continue;
                } else {
                    std::process::exit(1);
                }
            }
        };

        let mut threshold_met = true;
        if let Some(min_mb) = args.min_mb {
            let size_mb = size_kb as f64 / 1024.0;

            if size_mb < min_mb {
                if args.daemon {
                    threshold_met = false;
                    // Print an empty line to clear the Waybar module layout
                    println!();
                    let _ = io::stdout().flush();
                } else {
                    std::process::exit(1);
                }
            }
        }

        if threshold_met && !args.quiet {
            if size_kb >= 1048576 {
                println!("{:.1} GB", size_kb as f64 / 1048576.0);
            } else if size_kb >= 1024 {
                println!("{:.1} MB", size_kb as f64 / 1024.0);
            } else {
                println!("{} kB", size_kb);
            }
            // Vital for Waybar modules to receive updates instantly over pipes
            let _ = io::stdout().flush();
        }

        // If daemon mode is disabled, exit the loop immediately (Exit Status 0)
        if !args.daemon {
            break;
        }

        thread::sleep(delay);
    }
}

/// Extracts the `Writeback` value from a `/proc/meminfo` stream.
///
/// This implementation performs a single-pass scan over a 2KB stack buffer to avoid
/// heap allocations and `BufReader` overhead.
///
/// Given `/proc/meminfo` is a kernel generated virtual file We take it on faith that the "Writeback"
/// key will existin the first 2KB `/proc/meminfo`.
///
/// # Performance
/// - **Memory**: Uses a fixed `2048` byte array on the stack.
/// - **Time**: O(n) where `n` is the bytes read (capped at 2KB).
///
/// # Errors
/// - `io::ErrorKind::NotFound`: If the "Writeback:" key isn't found in the first 2KB.
/// # Example
/// ```
/// let input = b"Dirty: 0 kB\nWriteback: 1024 kB\nAnonPages: 0 kB";
/// let val = parse_writeback_kb(&input[..]).unwrap();
/// assert_eq!(val, 1024);
/// ```
fn parse_writeback_kb<R: io::Read>(mut reader: R) -> io::Result<u64> {
    let mut buffer = [0u8; 2048];
    let bytes_read = reader.read(&mut buffer)?;
    let data = &buffer[..bytes_read];

    let needle = b"Writeback:";
    let idx = memmem::find(data, needle).ok_or(io::ErrorKind::NotFound)?;

    let rest = &data[idx + needle.len()..];
    let start_of_digits = rest.iter().position(|&b| b != b' ').unwrap_or(rest.len());
    let digit_data = &rest[start_of_digits..];

    let value = digit_data
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .fold(0u64, |acc, b| acc * 10 + (b - b'0') as u64);

    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use clap::Parser;

    use crate::{Args, parse_writeback_kb};

    #[test]
    fn parses_writeback_value() {
        let data = "MemTotal: 16384 kB\nWriteback: 2048 kB\n";
        let result = parse_writeback_kb(Cursor::new(data)).unwrap();
        assert_eq!(result, 2048);
    }

    #[test]
    fn errors_when_writeback_missing() {
        let data = "MemTotal: 16384 kB\n";
        let result = parse_writeback_kb(Cursor::new(data));
        assert!(result.is_err());
    }

    #[test]
    fn parses_default_cli_args() {
        let args = Args::try_parse_from(["writeback"]).unwrap();
        assert!(!args.daemon);
        assert_eq!(args.interval, 2);
        assert!(!args.quiet);
        assert!(args.min_mb.is_none());
    }

    #[test]
    fn parses_all_cli_options() {
        let args = Args::try_parse_from([
            "writeback",
            "--daemon",
            "--interval",
            "5",
            "--quiet",
            "--min-mb",
            "10.5",
        ])
        .unwrap();

        assert!(args.daemon);
        assert_eq!(args.interval, 5);
        assert!(args.quiet);
        assert_eq!(args.min_mb, Some(10.5));
    }
}
