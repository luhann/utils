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
//! # Only exit with error if writeback is less than 500MB, otherwise stay silent
//! writeback --min-mb 500 --quiet
//! ```
use std::fs::File;
use std::io;

use memchr::memmem;

/// Minimal CLI options for binary.
#[derive(Debug, Default, PartialEq)]
struct CliOptions {
    min_mb: Option<f64>,
    quiet: bool,
}

/// Display command help options.
const USAGE: &str = concat!(
    "Usage: writeback [--min-mb <MB>] [--quiet]\n\n",
    "  --min-mb <MB>   Exit with status 0 only when writeback is >= MB\n",
    "  --quiet         Suppress normal output (useful with --min-mb)\n",
    "  -h, --help      Show this help"
);

/// Main entry point for binary
fn main() {
    let options = match parse_cli_args(std::env::args().skip(1)) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{err}\n\n{}", USAGE);
            std::process::exit(2);
        }
    };

    let file = File::open("/proc/meminfo").unwrap_or_else(|e| {
        eprintln!("Error reading file: {e}");
        std::process::exit(1);
    });

    // Pass the raw File descriptor directly.
    //
    // We bypass [`std::io::BufReader`] because `/proc` files are pseudo-files;
    // the kernel generates the content on `read()`. A single 4KB read is
    // atomic and sufficient for `meminfo`, making additional buffering redundant.
    let size_kb = match parse_writeback_kb(file) {
        Ok(value) => value,
        Err(e) => {
            let msg = match e.kind() {
                io::ErrorKind::OutOfMemory => "/proc/meminfo exceeded the 2KB buffer ceiling",
                io::ErrorKind::NotFound => "Writeback entry not found",
                _ => "Unknown I/O error",
            };
            eprintln!("Error parsing meminfo: {msg}");
            std::process::exit(1);
        }
    };

    if let Some(min_mb) = options.min_mb {
        let size_mb = size_kb as f64 / 1024.0;

        if size_mb < min_mb {
            std::process::exit(1);
        }
    }

    if options.quiet {
        return;
    }

    if size_kb >= 1048576 {
        println!("{:.1} GB", size_kb as f64 / 1048576.0);
    } else if size_kb >= 1024 {
        println!("{:.1} MB", size_kb as f64 / 1024.0);
    } else {
        println!("{} kB", size_kb);
    }
}

/// Parses command-line arguments into structured [`CliOptions`].
/// 
/// # Errors
/// Returns an `Err` if an unknown flag is passed or if `--min-mb` is given
/// a non-numeric or negative value.
fn parse_cli_args<I>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut options = CliOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--min-mb" => {
                let Some(value) = iter.next() else {
                    return Err("--min-mb requires a numeric value".to_string());
                };

                let parsed = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid --min-mb value: {value}"))?;

                if parsed.is_sign_negative() {
                    return Err("--min-mb must be non-negative".to_string());
                }

                options.min_mb = Some(parsed);
            }
            "--quiet" | "-q" => {
                options.quiet = true;
            }
            "--help" | "-h" => {
                println!("{}", USAGE);
                std::process::exit(0);
            }
            _ => {
                return Err(format!("unknown argument: {arg}"));
            }
        }
    }

    Ok(options)
}

/// Extracts the `Writeback` value from a `/proc/meminfo` stream.
///
/// This implementation performs a single-pass scan over a 4KB stack buffer to avoid 
/// heap allocations and `BufReader` overhead. 
///
/// # Performance
/// - **Memory**: Uses a fixed `4096` byte array on the stack.
/// - **Time**: O(n) where `n` is the bytes read (capped at 4KB).
///
/// # Errors
/// - `io::ErrorKind::NotFound`: If the "Writeback:" key isn't found in the first 4KB.
/// - `io::ErrorKind::OutOfMemory`: If the buffer is filled without finding the key.
/// # Example
/// ```
/// let input = b"Dirty: 0 kB\nWriteback: 1024 kB\nAnonPages: 0 kB";
/// let val = parse_writeback_kb(&input[..]).unwrap();
/// assert_eq!(val, 1024);
/// ```
fn parse_writeback_kb<R: io::Read>(mut reader: R) -> io::Result<u64> {
    // [0u8; 2048] forces the CPU to zero out 4KB of stack memory on every function call.
    let mut buffer = [0u8; 2048]; 
    let bytes_read = reader.read(&mut buffer)?;
    let data = &buffer[..bytes_read];

    let needle = b"Writeback:";
    if let Some(idx) = memmem::find(data, needle) {
        // Slice the data to right after "Writeback:"
        let rest = &data[idx + needle.len()..];

        // Vectorized space skipping.
        // Using `.position()` allows LLVM to heavily optimize or even vectorize 
        // the scan for the first non-space character.
        let start_of_digits = rest.iter().position(|&b| b != b' ').unwrap_or(rest.len());
        let digit_data = &rest[start_of_digits..];

        let mut value = 0u64;
        let mut found_digit = false;

        // Zero-cost iteration without bounds checking.
        // Iterating over a direct sub-slice slice (`&b in digit_data`) completely
        // removes the need for an index variable and strips out all internal bounds checks.
        for &b in digit_data {
            if b.is_ascii_digit() {
                value = value * 10 + (b - b'0') as u64;
                found_digit = true;
            } else {
                break; // Met a non-digit (e.g., the space before "kB"), stop parsing.
            }
        }

        if found_digit {
            return Ok(value);
        }
    }

    if bytes_read == buffer.len() {
        return Err(io::ErrorKind::OutOfMemory.into());
    }

    Err(io::ErrorKind::NotFound.into())
}


#[cfg(test)]
mod tests {
    use super::{CliOptions, parse_cli_args, parse_writeback_kb};
    use std::io::Cursor;

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
    fn parses_min_mb_and_quiet_args() {
        let args = vec![
            "--min-mb".to_string(),
            "50".to_string(),
            "--quiet".to_string(),
        ];
        let result = parse_cli_args(args).unwrap();

        assert_eq!(
            result,
            CliOptions {
                min_mb: Some(50.0),
                quiet: true,
            }
        );
    }

    #[test]
    fn errors_on_invalid_min_mb() {
        let args = vec!["--min-mb".to_string(), "abc".to_string()];
        let result = parse_cli_args(args);

        assert!(result.is_err());
    }
}
