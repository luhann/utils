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
use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use memchr::memmem;

/// Minimal CLI options for binary.
#[derive(Debug, PartialEq)]
struct CliOptions {
    min_mb: Option<f64>,
    quiet: bool,
    daemon: bool,
    interval: u64,
}

impl Default for CliOptions {
    fn default() -> Self {
        CliOptions {
            min_mb: None,
            quiet: false,
            daemon: false,
            interval: 2, // Default polling interval of 2 seconds
        }
    }
}

/// Display command help options.
const USAGE: &str = concat!(
    "Usage: writeback [--min-mb <MB>] [--quiet] [--daemon] [--interval <seconds>]\n\n",
    "  --min-mb <MB>      Exit with status 0 (or print nothing in daemon mode) only when writeback is >= MB\n",
    "  --quiet, -q        Suppress normal output (useful with --min-mb)\n",
    "  --daemon, -d       Run continuously as a daemon (ideal for Waybar streaming)\n",
    "  --interval, -i <S> Polling interval in seconds for daemon mode (default: 2)\n",
    "  -h, --help         Show this help"
);

/// Main entry point for binary
fn main() {
    let options = match parse_cli_args(std::env::args_os().skip(1)) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Error: {err}\n\n{}", USAGE);
            std::process::exit(2);
        }
    };

    loop {
        let file = match File::open("/proc/meminfo") {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error reading file: {e}");
                if options.daemon {
                    thread::sleep(Duration::from_secs(options.interval));
                    continue;
                } else {
                    std::process::exit(1);
                }
            }
        };

        // Pass the raw File descriptor directly.
        let size_kb = match parse_writeback_kb(file) {
            Ok(value) => value,
            Err(e) => {
                let msg = match e.kind() {
                    io::ErrorKind::OutOfMemory => "/proc/meminfo exceeded the 2KB buffer ceiling",
                    io::ErrorKind::NotFound => "Writeback entry not found",
                    _ => "Unknown I/O error",
                };
                eprintln!("Error parsing meminfo: {msg}");
                if options.daemon {
                    thread::sleep(Duration::from_secs(options.interval));
                    continue;
                } else {
                    std::process::exit(1);
                }
            }
        };

        let mut threshold_met = true;
        if let Some(min_mb) = options.min_mb {
            let size_mb = size_kb as f64 / 1024.0;

            if size_mb < min_mb {
                if options.daemon {
                    threshold_met = false;
                    // Print an empty line to clear the Waybar module layout
                    println!();
                    let _ = io::stdout().flush();
                } else {
                    std::process::exit(1);
                }
            }
        }

        if threshold_met && !options.quiet {
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
        if !options.daemon {
            break;
        }

        thread::sleep(Duration::from_secs(options.interval));
    }
}

/// Parses command-line arguments into structured [`CliOptions`].
fn parse_cli_args<I>(args: I) -> Result<CliOptions, std::borrow::Cow<'static, str>>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut options = CliOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg_os) = iter.next() {
        // Convert to &str cleanly without allocating.
        // Non-UTF8 arguments are safely treated as unknown arguments.
        let arg = arg_os.to_str().unwrap_or("");

        match arg {
            "--min-mb" => {
                let Some(value_os) = iter.next() else {
                    return Err("--min-mb requires a numeric value".into());
                };
                let value = value_os.to_str().unwrap_or("");

                let parsed = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid --min-mb value: {value}"))?;

                if parsed.is_sign_negative() {
                    return Err("--min-mb must be non-negative".into());
                }

                options.min_mb = Some(parsed);
            }
            "--quiet" | "-q" => {
                options.quiet = true;
            }
            "--daemon" | "-d" => {
                options.daemon = true;
            }
            "--interval" | "-i" => {
                let Some(value_os) = iter.next() else {
                    return Err("--interval requires a numeric value".into());
                };
                let value = value_os.to_str().unwrap_or("");

                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --interval value: {value}"))?;

                options.interval = parsed;
            }
            "--help" | "-h" => {
                println!("{}", USAGE);
                std::process::exit(0);
            }
            _ => {
                // Safely lossy-decode unknown arguments for the error message
                return Err(format!("unknown argument: {}", arg_os.to_string_lossy()).into());
            }
        }
    }

    Ok(options)
}

/// Extracts the `Writeback` value from a `/proc/meminfo` stream.
///
/// This implementation performs a single-pass scan over a 2KB stack buffer to avoid 
/// heap allocations and `BufReader` overhead. 
///
/// # Performance
/// - **Memory**: Uses a fixed `2048` byte array on the stack.
/// - **Time**: O(n) where `n` is the bytes read (capped at 2KB).
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
    let mut buffer = [0u8; 2048];
    let bytes_read = reader.read(&mut buffer)?;
    let data = &buffer[..bytes_read];

    let needle = b"Writeback:";
    if let Some(idx) = memmem::find(data, needle) {
        let rest = &data[idx + needle.len()..];
        let start_of_digits = rest.iter().position(|&b| b != b' ').unwrap_or(rest.len());
        let digit_data = &rest[start_of_digits..];

        let mut value = 0u64;
        let mut found_digit = false;

        for &b in digit_data {
            if b.is_ascii_digit() {
                value = value * 10 + (b - b'0') as u64;
                found_digit = true;
            } else {
                break;
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
        // Map native strings into OsString for the new zero-copy signature
        let args = vec!["--min-mb", "50", "--quiet"]
            .into_iter()
            .map(std::ffi::OsString::from);

        let result = parse_cli_args(args).unwrap();

        assert_eq!(
            result,
            CliOptions {
                min_mb: Some(50.0),
                quiet: true,
                daemon: false,
                interval: 2,
            }
        );
    }

    #[test]
    fn parses_daemon_and_interval_args() {
        let args = vec!["--daemon", "--interval", "5"]
            .into_iter()
            .map(std::ffi::OsString::from);

        let result = parse_cli_args(args).unwrap();

        assert_eq!(
            result,
            CliOptions {
                min_mb: None,
                quiet: false,
                daemon: true,
                interval: 5,
            }
        );
    }

    #[test]
    fn errors_on_invalid_min_mb() {
        let args = vec!["--min-mb", "abc"]
            .into_iter()
            .map(std::ffi::OsString::from);

        let result = parse_cli_args(args);
        assert!(result.is_err());
    }
}
