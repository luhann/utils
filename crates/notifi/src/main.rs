//! # notifi
//!
//! An efficient, event-driven utility to monitor [dunst](https://github.com/dunst-project/dunst) status.
//!
//! Prints dunst status instantly on state changes using DBus monitoring.

use std::borrow::Cow;

use clap::{Parser, ValueEnum};
use futures_util::StreamExt;
use serde_json::{Value, json};
use zbus::{Connection, MatchRule, Proxy, fdo::DBusProxy, message::Type as MessageType};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum OutputMode {
    Json,
    Plain,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Monitor Dunst status for Waybar", long_about = None)]
struct Args {
    /// Output format for dunstctl status
    #[arg(short, long, value_enum, default_value_t = OutputMode::Json)]
    mode: OutputMode,

    /// Run continuously as an event-driven daemon
    #[arg(short, long)]
    daemon: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let conn = Connection::session().await?;

    let dunst_proxy = Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.dunstproject.cmd0",
    )
    .await?;

    if !args.daemon {
        println!("{}", get_status(&dunst_proxy, args.mode).await?);
        return Ok(());
    }

    let dbus_proxy = DBusProxy::new(&conn).await?;

    // Match rule: Wake up ONLY when Dunst properties change (pauseLevel, waitingLength, historyLength)
    let rule_props = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();

    // Register dbus rule
    dbus_proxy.add_match_rule(rule_props).await?;

    let mut stream = zbus::MessageStream::from(&conn);

    // Track the last printed line to eliminate the cascading pause double-print
    let mut last_output = get_status(&dunst_proxy, args.mode).await?;
    println!("{}", last_output);

    while stream.next().await.is_some() {
        let new_output = get_status(&dunst_proxy, args.mode).await?;
        // If Dunst spams multiple property updates for the same event,
        // this boundary blocks the duplicate text from hitting Waybar/Polybar.
        if new_output != last_output {
            // Use a block to control the lifetime of the lock
            {
                use std::io::Write;
                let mut out = std::io::stdout().lock();

                let _ = writeln!(out, "{}", new_output);
                let _ = out.flush();
            }

            last_output = new_output;
        }
    }

    Ok(())
}

/// Fetches current dunst state via DBus and returns the formatted output string
async fn get_status(dunst_proxy: &Proxy<'_>, output_mode: OutputMode) -> Result<Value> {
    // Check for "paused" (bool). If that fails, fallback to "pauseLevel" (i32).
    let is_paused = if let Ok(paused) = dunst_proxy.get_property::<bool>("paused").await {
        paused
    } else {
        // 2. Fallback to "pauseLevel" if "paused" fails
        dunst_proxy
            .get_property::<i32>("pauseLevel")
            .await
            .is_ok_and(|level| level > 0)
    };

    // Fetch waitingLength natively
    let waiting_count = dunst_proxy
        .get_property::<u32>("waitingLength")
        .await
        .unwrap_or_default();

    // Fetch historyLength natively
    let num_notifications = dunst_proxy
        .get_property::<u32>("historyLength")
        .await
        .unwrap_or_default();

    if output_mode == OutputMode::Plain {
        // Waybar custom modules can also accept a raw string text line if you want just the icon
        return Ok(json!(if is_paused { "" } else { "" }));
    }

    let (text, class, alt): (Cow<'_, str>, &str, Option<&str>) = if is_paused {
        let txt = if waiting_count > 0 {
            Cow::Owned(format!(" {waiting_count}"))
        } else {
            Cow::Borrowed("") // No allocation!
        };
        (txt, "paused", None)
    } else {
        (Cow::Borrowed(""), "active", Some("active"))
    };

    let output = json!({
        "text": text,
        "class": class,
        "alt": alt,
        "tooltip": format!("{num_notifications} notifications in history"),
    });

    Ok(output)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_defaults() {
        let args = Args::try_parse_from(["notifi"]).unwrap();
        assert_eq!(args.mode, OutputMode::Json);
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
