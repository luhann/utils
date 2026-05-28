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
//! dunst
//!
//! # Print dunst status using a single icon.
//! dunst --plain
//! ```
use serde_json::Value;
use shared::command_exists;
use std::env;
use std::error::Error;
use std::process::Command;

/// Output format for dunstctl status
enum OutputMode {
    Json,
    Plain,
    Auto,
}

/// Main entry point for binary
fn main() -> Result<(), Box<dyn Error>> {
    let output_mode = parse_output_mode()?;

    if !command_exists("dunstctl") {
        return Err("dunstctl not available".into());
    }

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

/// Parse the dunst output to supported formats
///
/// Currently only `json` and `plain text` output are supported.
fn parse_output_mode() -> Result<OutputMode, Box<dyn Error>> {
    let mut output_mode = OutputMode::Auto;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--json" => {
                if matches!(output_mode, OutputMode::Plain) {
                    return Err("cannot combine --json and --plain".into());
                }
                output_mode = OutputMode::Json;
            }
            "--plain" => {
                if matches!(output_mode, OutputMode::Json) {
                    return Err("cannot combine --json and --plain".into());
                }
                output_mode = OutputMode::Plain;
            }
            "-h" | "--help" => {
                println!(
                    "Usage: notifi [--json|--plain]\n\n  --json   Force Waybar JSON output\n  --plain  Force Polybar plain-text output"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    Ok(output_mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_flag() {
        assert!(matches!(
            parse_output_mode_from_args(["--json"]).unwrap(),
            OutputMode::Json
        ));
    }

    #[test]
    fn parses_plain_flag() {
        assert!(matches!(
            parse_output_mode_from_args(["--plain"]).unwrap(),
            OutputMode::Plain
        ));
    }

    #[test]
    fn errors_on_both_flags() {
        assert!(parse_output_mode_from_args(["--json", "--plain"]).is_err());
        assert!(parse_output_mode_from_args(["--plain", "--json"]).is_err());
    }

    #[test]
    fn errors_on_unknown_flag() {
        assert!(parse_output_mode_from_args(["--foo"]).is_err());
    }

    // Helper for testing: mimic parse_output_mode but take args as slice
    fn parse_output_mode_from_args<const N: usize>(
        args: [&str; N],
    ) -> Result<OutputMode, Box<dyn std::error::Error>> {
        let mut output_mode = OutputMode::Auto;
        for arg in args.iter() {
            match *arg {
                "--json" => {
                    if matches!(output_mode, OutputMode::Plain) {
                        return Err("cannot combine --json and --plain".into());
                    }
                    output_mode = OutputMode::Json;
                }
                "--plain" => {
                    if matches!(output_mode, OutputMode::Json) {
                        return Err("cannot combine --json and --plain".into());
                    }
                    output_mode = OutputMode::Plain;
                }
                "-h" | "--help" => {
                    return Ok(output_mode);
                }
                _ => {
                    return Err(format!("unknown argument: {arg}").into());
                }
            }
        }
        Ok(output_mode)
    }
}
