use std::fs::File;
use std::io;

#[derive(Debug, Default, PartialEq)]
struct CliOptions {
    min_mb: Option<f64>,
    quiet: bool,
}

fn usage() -> &'static str {
    "Usage: writeback [--min-mb <MB>] [--quiet]\n\n  --min-mb <MB>  Exit with status 0 only when writeback is >= MB\n  --quiet        Suppress normal output (useful with --min-mb)\n  -h, --help     Show this help"
}

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
                println!("{}", usage());
                std::process::exit(0);
            }
            _ => {
                return Err(format!("unknown argument: {arg}"));
            }
        }
    }

    Ok(options)
}

fn parse_writeback_kb<R: std::io::Read>(mut reader: R) -> io::Result<u64> {
    let mut buffer = [0u8; 4096];
    let bytes_read = reader.read(&mut buffer)?;

    if bytes_read == buffer.len() {
        return Err(io::Error::new(
            io::ErrorKind::OutOfMemory,
            "/proc/meminfo exceeded the 4KB stack buffer ceiling",
        ));
    }

    let data = &buffer[..bytes_read];

    if let Some(idx) = data.windows(10).position(|w| w == b"Writeback:") {
        let mut pos = idx + 10;

        while pos < data.len() && data[pos] == b' ' {
            pos += 1;
        }

        let mut value = 0u64;
        let mut found_digit = false;

        while pos < data.len() && data[pos].is_ascii_digit() {
            value = value * 10 + (data[pos] - b'0') as u64;
            pos += 1;
            found_digit = true;
        }

        if found_digit {
            return Ok(value);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Writeback entry not found",
    ))
}

fn main() {
    let options = match parse_cli_args(std::env::args().skip(1)) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{err}\n\n{}", usage());
            std::process::exit(2);
        }
    };

    let file = File::open("/proc/meminfo").unwrap_or_else(|e| {
        eprintln!("Error reading file: {e}");
        std::process::exit(1);
    });

    // Pass the raw File descriptor directly, bypassing the BufReader layer entirely
    let size_kb = match parse_writeback_kb(file) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error parsing meminfo: {}", e);
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
