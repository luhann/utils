use std::env;
use std::error::Error;
use std::process::Command;

const MIN_BRIGHTNESS: u8 = 0;
const MAX_BRIGHTNESS: u8 = 100;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [b1] => set_brightness(b1, b1),
        [b1, b2] => set_brightness(b1, b2),
        _ => {
            eprintln!(
                "Usage:\n  backlight <brightness 0-100>\n  backlight <brightness1 0-100> <brightness2 0-100>"
            );
            Err("invalid argument count".into())
        }
    }
}

fn set_brightness(b1: &str, b2: &str) -> Result<(), Box<dyn Error>> {
    let brightness_display_1 = parse_brightness(b1, "brightness")?.to_string();
    let brightness_display_2 = parse_brightness(b2, "brightness")?.to_string();

    let mut display_1 = spawn_ddcutil("1", &brightness_display_1)?;
    let mut display_2 = spawn_ddcutil("2", &brightness_display_2)?;

    let status_1 = display_1.wait()?;
    let status_2 = display_2.wait()?;

    if !status_1.success() || !status_2.success() {
        return Err("one or more ddcutil commands failed".into());
    }
    Ok(())
}

fn spawn_ddcutil(display: &str, brightness: &str) -> Result<std::process::Child, Box<dyn Error>> {
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

fn parse_brightness(raw: &str, arg_name: &str) -> Result<u8, Box<dyn Error>> {
    let parsed: u8 = raw.parse().map_err(|_| {
        format!("{arg_name} must be an integer between {MIN_BRIGHTNESS} and {MAX_BRIGHTNESS}")
    })?;

    if !(MIN_BRIGHTNESS..=MAX_BRIGHTNESS).contains(&parsed) {
        return Err(
            format!("{arg_name} must be between {MIN_BRIGHTNESS} and {MAX_BRIGHTNESS}").into(),
        );
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_brightness() {
        assert_eq!(parse_brightness("42", "test").unwrap(), 42);
    }

    #[test]
    fn errors_on_non_numeric() {
        assert!(parse_brightness("abc", "test").is_err());
    }

    #[test]
    fn errors_on_too_low() {
        assert!(parse_brightness("-1", "test").is_err());
    }

    #[test]
    fn errors_on_too_high() {
        assert!(parse_brightness("101", "test").is_err());
    }
}
