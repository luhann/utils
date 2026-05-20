use serde_json::Value;
use std::env;
use std::error::Error;
use std::process::Command;

enum OutputMode {
    Json,
    Plain,
    Auto,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let output_mode = parse_output_mode()?;

    if which::which("dunstctl").is_err() {
        return Err("dunstctl not available".into());
    }

    let is_paused = run_dunstctl(["is-paused"])?.trim() == "true";
    let waiting_count: u64 = run_dunstctl(["count", "waiting"])?
        .trim()
        .parse()
        .unwrap_or(0);
    let num_notifications = history_count()?;

    let is_wayland = env::var("XDG_SESSION_TYPE")
        .map(|value| value == "wayland")
        .unwrap_or(false);

    let emit_json = match output_mode {
        OutputMode::Json => true,
        OutputMode::Plain => false,
        OutputMode::Auto => is_wayland,
    };

    if emit_json {
        let enabled_icon = "";
        let disabled_icon = "";

        if is_paused {
            let output_text = if waiting_count > 0 {
                format!("{disabled_icon} {waiting_count}")
            } else {
                disabled_icon.to_owned()
            };

            let output = serde_json::json!({
                "text": output_text,
                "class": "paused",
                "tooltip": num_notifications.to_string(),
            });
            println!("{}", output);
        } else {
            let output = serde_json::json!({
                "text": enabled_icon,
                "alt": "active",
                "tooltip": num_notifications.to_string(),
            });
            println!("{}", output);
        }
    } else if is_paused {
        println!("%{{F#821717}} %{{F-}}");
    } else {
        println!("");
    }

    Ok(())
}

fn run_dunstctl<const N: usize>(args: [&str; N]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("dunstctl").args(args).output()?;
    if !output.status.success() {
        return Err(format!("dunstctl command failed: {}", args.join(" ")).into());
    }

    Ok(String::from_utf8(output.stdout)?)
}

fn history_count() -> Result<usize, Box<dyn Error>> {
    let history_raw = run_dunstctl(["history"])?;
    let history_json: Value = serde_json::from_str(&history_raw)?;

    let count = history_json
        .get("data")
        .and_then(Value::as_array)
        .and_then(|data| data.first())
        .and_then(Value::as_array)
        .map(|notifications| notifications.len())
        .unwrap_or(0);

    Ok(count)
}

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
                print_help();
                std::process::exit(0);
            }
            _ => {
                return Err(format!("unknown argument: {arg}").into());
            }
        }
    }

    Ok(output_mode)
}

fn print_help() {
    println!("Usage: notifi [--json|--plain]");
    println!();
    println!("  --json   Force Waybar JSON output");
    println!("  --plain  Force Polybar plain-text output");
}
