//! # backlight
//!
//! A utility to control external monitor brightness through `ddcutil`.
//!
//! # Examples
//!
//! ```fish
//! backlight 10
//!
//! # or
//!
//! backlight 10 25
//! ```
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Parser;

/// Minimum screen brightness.
const MIN_BRIGHTNESS: i64 = 0;
/// Maximum screen brightness.
const MAX_BRIGHTNESS: i64 = 100;

#[derive(Parser, Debug)]
#[command(author, version, about = "A utility to control external monitor brightness through `ddcutil`.", long_about = None)]
struct Args {
    /// Brightness value for display 1 (or both displays when brightness2 is omitted)
    #[arg(value_parser = clap::value_parser!(u8).range(MIN_BRIGHTNESS..=MAX_BRIGHTNESS))]
    brightness1: u8,

    /// Optional brightness value for display 2
    #[arg(value_parser = clap::value_parser!(u8).range(MIN_BRIGHTNESS..=MAX_BRIGHTNESS))]
    brightness2: Option<u8>,
}

/// Main entry point for binary.
fn main() -> Result<()> {
    let args = Args::parse();

    let brightness_display_1 = args.brightness1;
    let brightness_display_2 = args.brightness2.unwrap_or(args.brightness1);

    set_brightness(brightness_display_1, brightness_display_2)
}

/// Set brightness on two monitors.
///
/// **Note:** Currently this function assumes that you are using a two monitor setup.
///
fn set_brightness(b1: u8, b2: u8) -> Result<()> {
    let brightness_display_1 = b1.to_string();
    let brightness_display_2 = b2.to_string();

    let mut display_1 =
        spawn_ddcutil("1", &brightness_display_1).context("spawn ddcutil failed")?;
    let mut display_2 =
        spawn_ddcutil("2", &brightness_display_2).context("spawn ddcutil failed")?;

    let status_1 = display_1.wait()?;
    let status_2 = display_2.wait()?;

    if !status_1.success() || !status_2.success() {
        bail!("one or more ddcutil commands failed");
    }
    Ok(())
}

/// Spawn ddcutil
///
/// Change the brightness on the provided display with the given brightness value.
/// `--skip-ddc-checks` and --enable-dynamic-sleep` are options that experimentally appear to
/// provide the best balance between speed and correctness.
///
/// I thought about using the [ddc_hi](https://docs.rs/ddc-hi/latest/ddc_hi/) crate to replace
/// this, but `ddcutil` does a lot under the hood to speed up i2c connections and ensure
/// correctness. I'm not convinced the Rust crate will be faster here for a oneshot CLI util.
fn spawn_ddcutil(display: &str, brightness: &str) -> Result<std::process::Child> {
    let child = Command::new("ddcutil")
        .args([
            "--display",
            display,
            "--skip-ddc-checks",
            "--enable-dynamic-sleep",
            "setvcp",
            "10",
            brightness,
        ])
        .spawn()?;

    Ok(child)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn errors_on_invalid_brightness_values() {
        assert!(Args::try_parse_from(["backlight", "abc"]).is_err());
        assert!(Args::try_parse_from(["backlight", "-1"]).is_err());
        assert!(Args::try_parse_from(["backlight", "101"]).is_err());
    }

    #[test]
    fn parses_single_brightness_argument() {
        let args = Args::try_parse_from(["backlight", "10"]).unwrap();
        assert_eq!(args.brightness1, 10);
        assert!(args.brightness2.is_none());
    }

    #[test]
    fn parses_two_brightness_arguments() {
        let args = Args::try_parse_from(["backlight", "10", "25"]).unwrap();
        assert_eq!(args.brightness1, 10);
        assert_eq!(args.brightness2, Some(25));
    }

    #[test]
    fn errors_when_brightness_argument_missing() {
        assert!(Args::try_parse_from(["backlight"]).is_err());
    }
}
