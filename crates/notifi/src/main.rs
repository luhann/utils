//! # notifi
//!
//! An efficient, event-driven utility to monitor [dunst](https://github.com/dunst-project/dunst) status.
//!
//! Prints dunst status instantly on state changes using DBus monitoring.

use std::{
    env,
    error::Error,
    io::{self, Write},
    process::Command,
};

use clap::{Parser, ValueEnum};
use serde_json::json;
use shared::command_exists;
use zbus::{
    MatchRule, MessageType,
    blocking::{Connection, fdo::DBusProxy},
};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum OutputMode {
    Json,
    Plain,
    Auto,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Monitor Dunst status for Waybar/Polybar", long_about = None)]
struct Args {
    /// Output format for dunstctl status
    #[arg(short, long, value_enum, default_value_t = OutputMode::Auto)]
    mode: OutputMode,

    /// Run continuously as an event-driven daemon
    #[arg(short, long)]
    daemon: bool,
}

/// Main entry point for binary
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if !command_exists("dunstctl") {
        return Err("dunstctl not available".into());
    }

    if !args.daemon {
        print_status(&args.mode)?;
        return Ok(());
    }

    let conn = Connection::session()?;

    let dbus_proxy = DBusProxy::new(&conn)?;

    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .path("/org/freedesktop/Notifications")?
        .interface("org.freedesktop.DBus.Properties")?
        .build();

    dbus_proxy.add_match_rule(rule)?;

    let mut iterator = zbus::blocking::MessageIterator::from(&conn);

    print_status(&args.mode)?;

    while let Some(Ok(_message)) = iterator.next() {
        if let Err(e) = print_status(&args.mode) {
            eprintln!("Error updating dunst status: {e}");
        }
    }

    Ok(())
}

/// Fetches current dunst state and prints it to stdout
fn print_status(output_mode: &OutputMode) -> Result<(), Box<dyn Error>> {
    let is_paused = run_dunstctl(&["is-paused"])?.trim() == "true";
    let waiting_count: u64 = run_dunstctl(&["count", "waiting"])?
        .trim()
        .parse()
        .map_err(|e| format!("Failed to parse waiting count: {e}"))?;

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

        let output = json!({
            "text": text,
            "class": class,
            "alt": alt,
            "tooltip": format!("{num_notifications} notifications in history"),
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
    let history_json: serde_json::Value = serde_json::from_str(&history_raw)?;

    let count = history_json["data"][0]
        .as_array()
        .map(|notifications| notifications.len())
        .ok_or("Unexpected dunst history format")?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_defaults() {
        let args = Args::try_parse_from(["notifi"]).unwrap();
        assert_eq!(args.mode, OutputMode::Auto);
        assert!(!args.daemon);
    }

    #[test]
    fn parses_explicit_mode_and_daemon() {
        let args = Args::try_parse_from(["notifi", "--mode", "plain", "--daemon"]).unwrap();
        assert_eq!(args.mode, OutputMode::Plain);
        assert!(args.daemon);
    }

    #[test]
    fn errors_on_invalid_mode() {
        assert!(Args::try_parse_from(["notifi", "--mode", "yaml"]).is_err());
    }
}
