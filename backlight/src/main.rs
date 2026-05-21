use std::env;
use std::error::Error;
use std::path::Path;
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
    let program = env::args()
        .next()
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "backlight".to_owned());

    if args.is_empty() || args.len() > 2 {
        eprintln!("Usage:");
        eprintln!(
            "  {program} <brightness 0-100>                 # Set both displays to same brightness"
        );
        eprintln!(
            "  {program} <brightness1 0-100> <brightness2 0-100>  # Set each display individually"
        );
        return Err("invalid argument count".into());
    }

    let brightness_display_1 = parse_brightness(&args[0], "brightness")?;
    let brightness_display_2 = if args.len() == 2 {
        parse_brightness(&args[1], "brightness")?
    } else {
        brightness_display_1
    };

    let brightness_display_1 = brightness_display_1.to_string();
    let brightness_display_2 = brightness_display_2.to_string();

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
