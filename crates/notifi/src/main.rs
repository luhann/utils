//! # notifi
//!
//! A utility to monitor [dunst](https://github.com/dunst-project/dunst) status.
//!
//! By default it will return dunst status as json. Using the `--plain` option will return an icon
//! depending on current dunst state.
//!
//! ## Examples
//!
//! ```fish
//! # Print current dunst status.
//! notifi
//!
//! # Print dunst status using a single icon.
//! notifi --plain
//!
//! # Run as a daemon for Waybar
//! notifi --daemon
//! ```
use serde_json::Value;
use shared::command_exists;
use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Output format for dunstctl status
#[derive(Debug, Clone, Copy, PartialEq)]
enum OutputMode {
    Json,
    Plain,
    Auto,
}

/// Configuration parsed from CLI arguments
#[derive(Debug, PartialEq)]
struct Config {
    output_mode: OutputMode,
    daemon: bool,
    interval: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output_mode: OutputMode::Auto,
            daemon: false,
            interval: 2, // Default polling interval of 2 seconds
        }
    }
}

/// Main entry point for binary
fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;

    if !command_exists("dunstctl") {
        return Err("dunstctl not available".into());
    }

    if config.daemon {
        loop {
            if let Err(e) = print_status(&config.output_mode) {
                eprintln!("Error fetching dunst status: {e}");
            }
            // Use the parsed interval here!
            thread::sleep(Duration::from_secs(config.interval as u64));
        }
    } else {
        print_status(&config.output_mode)?;
    }

    Ok(())
}

/// Fetches current dunst state and prints it to stdout
fn print_status(output_mode: &OutputMode) -> Result<(), Box<dyn Error>> {
    let is_paused = run_dunstctl(&["is-paused"])?.trim() == "true";
    let waiting_count: u64 = run_dunstctl(&["count", "waiting"])?
        .trim()
        .parse()
        .unwrap_or(0);

    let num_notifications = history_count()?;
    let is_wayland = env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland");

    let emit_json = match output_mode {
        OutputMode::Json => true,
        OutputMode::Plain => false,
        OutputMode::Auto => is_wayland,
    };

    if emit_json {
        let (text, class, alt) = if is_paused {
            let icon = "";
            let txt = if waiting_count > 0 {
                format!("{icon} {waiting_count}")
            } else {
                icon.to_owned()
            };
            (txt, "paused", None)
        } else {
            ("".to_owned(), "active", Some("active"))
        };

        let output = serde_json::json!({
            "text": text,
            "class": class,
            "alt": alt,
            "tooltip": num_notifications.to_string(),
        });
        println!("{}", output);
    } else {
        // Polybar formatting
        let text = if is_paused {
            "%{F#821717} %{F-}"
        } else {
            ""
        };
        println!("{}", text);
    }

    // CRITICAL: Force Rust to push the line through the pipe to Waybar immediately
    io::stdout().flush()?;
    Ok(())
}

/// Run dunstctl with the given args
fn run_dunstctl(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("dunstctl").args(args).output()?;
    if !output.status.success() {
        return Err(format!("dunstctl command failed: {}", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Get number of notifications in history
fn history_count() -> Result<usize, Box<dyn Error>> {
    let history_raw = run_dunstctl(&["history"])?;
    let history_json: Value = serde_json::from_str(&history_raw)?;

    let count = history_json["data"][0]
        .as_array()
        .map(|notifications| notifications.len())
        .unwrap_or(0);

    Ok(count)
}

/// Parse arguments into a Config struct
fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut config = Config::default();

    // We use into_iter() and a while loop so we can consume extra arguments
    // when we hit a flag that requires a value (like --interval)
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {
                if matches!(config.output_mode, OutputMode::Plain) {
                    return Err("cannot combine --json and --plain".into());
                }
                config.output_mode = OutputMode::Json;
            }
            "--plain" => {
                if matches!(config.output_mode, OutputMode::Json) {
                    return Err("cannot combine --json and --plain".into());
                }
                config.output_mode = OutputMode::Plain;
            }
            "-d" | "--daemon" => {
                config.daemon = true;
            }
            "-i" | "--interval" => {
                let next_arg = args.next().ok_or("expected value for --interval")?;
                let interval: usize = next_arg
                    .parse()
                    .map_err(|_| "interval must be a positive integer")?;
                config.interval = interval;
            }
            "-h" | "--help" => {
                println!(
                    "Usage: notifi [OPTIONS]\n\nOptions:\n  --json             Force Waybar JSON output\n  --plain            Force Polybar plain-text output\n  -d, --daemon       Run continuously as a daemon\n  -i, --interval <N> Polling interval in seconds (default: 2)"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_flag() {
        let config = parse_args_from_args(&["--json"]).unwrap();
        assert_eq!(config.output_mode, OutputMode::Json);
        assert!(!config.daemon);
    }

    #[test]
    fn parses_plain_flag() {
        let config = parse_args_from_args(&["--plain"]).unwrap();
        assert_eq!(config.output_mode, OutputMode::Plain);
        assert!(!config.daemon);
    }

    #[test]
    fn parses_daemon_flag() {
        let config = parse_args_from_args(&["--daemon"]).unwrap();
        assert!(config.daemon);
    }

    #[test]
    fn parses_interval() {
        let config = parse_args_from_args(&["--interval", "5"]).unwrap();
        assert_eq!(config.interval, 5);
    }

    #[test]
    fn errors_on_both_flags() {
        assert!(parse_args_from_args(&["--json", "--plain"]).is_err());
        assert!(parse_args_from_args(&["--plain", "--json"]).is_err());
    }

    #[test]
    fn errors_on_missing_interval_value() {
        assert!(parse_args_from_args(&["--interval"]).is_err());
    }

    #[test]
    fn errors_on_invalid_interval_value() {
        assert!(parse_args_from_args(&["--interval", "foo"]).is_err());
    }

    #[test]
    fn errors_on_unknown_flag() {
        assert!(parse_args_from_args(&["--foo"]).is_err());
    }

    // Helper for testing: mimic parse_args but take args as a slice
    fn parse_args_from_args(args: &[&str]) -> Result<Config, Box<dyn std::error::Error>> {
        let mut config = Config::default();
        let mut args_iter = args.iter();

        while let Some(arg) = args_iter.next() {
            match *arg {
                "--json" => {
                    if matches!(config.output_mode, OutputMode::Plain) {
                        return Err("cannot combine --json and --plain".into());
                    }
                    config.output_mode = OutputMode::Json;
                }
                "--plain" => {
                    if matches!(config.output_mode, OutputMode::Json) {
                        return Err("cannot combine --json and --plain".into());
                    }
                    config.output_mode = OutputMode::Plain;
                }
                "-d" | "--daemon" => {
                    config.daemon = true;
                }
                "-i" | "--interval" => {
                    let next_arg = args_iter.next().ok_or("expected value for --interval")?;
                    let interval: usize = next_arg
                        .parse()
                        .map_err(|_| "interval must be a positive integer")?;
                    config.interval = interval;
                }
                "-h" | "--help" => {
                    return Ok(config);
                }
                _ => return Err(format!("unknown argument: {arg}").into()),
            }
        }
        Ok(config)
    }
}
