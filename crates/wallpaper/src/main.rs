//! # wallpaper
//!
//! A utility to set a random wallpaper on X11 or wayland, using `feh` or `awww` respectively.
//!
//! ## Exit Codes
//! * `0`: Success (wallpaper was changed successfully).
//! * `1`: A system error occurred, either no wallpapers could be found, or the wallpaper utility
//!   could not be found.
//!
//! ## Examples
//! ```fish
//! # Change current wallpaper to a random wallpaper in the wallpaper directory.
//! wallpaper
//!
//! ```

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use shared::home_dir;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "Set a random wallpaper on X11 or Wayland", long_about = None)]
struct Args {
    /// Pass --no-xinerama through to feh in X11 sessions
    #[arg(long)]
    no_xinerama: bool,

    /// Directory to pick a random wallpaper from
    #[arg(long, default_value_os_t = default_wallpaper_dir())]
    wallpaper_dir: PathBuf,
}

/// Main entry point for binary.
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let wallpaper = pick_random_wallpaper(&args.wallpaper_dir)?.ok_or_else(|| {
        format!(
            "No wallpaper files found in {}",
            args.wallpaper_dir.display()
        )
    })?;

    let is_wayland = env::var_os("XDG_SESSION_TYPE")
        .map(|val| val == "wayland")
        .unwrap_or(false);

    if is_wayland {
        let status = match Command::new("awww")
            .args([
                "img",
                "--transition-fps",
                "255",
                "--transition-type",
                "wave",
                "--transition-wave",
                "50,25",
                "--transition-angle",
                "135",
            ])
            .arg(wallpaper.path())
            .status()
        {
            Ok(status) => status,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err("awww not found on your system. Is the daemon running?".into());
            }
            Err(err) => return Err(err.into()),
        };

        if !status.success() {
            return Err("awww failed".into());
        }

        println!("Set Wayland wallpaper to: {}", wallpaper.path().display());
    } else {
        let mut command = Command::new("feh");
        command.arg("--no-fehbg");
        if args.no_xinerama {
            command.arg("--no-xinerama");
        }
        command.arg("--bg-fill").arg(wallpaper.path());

        let status = match command.status() {
            Ok(command) => command,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err("Feh not found on your system".to_string().into());
            }
            Err(err) => return Err(err.into()),
        };

        if !status.success() {
            return Err("feh failed".into());
        }

        println!("Set X11 wallpaper to: {}", wallpaper.path().display());
    }

    Ok(())
}

/// Return a random wallpaper path from the given directory.
///
/// This function makes use of [Reservoir sampling - Algorithm
/// L](https://en.wikipedia.org/wiki/Reservoir_sampling#Simple:_Algorithm_L).
///
///
/// # Examples
///
/// ```
///    let wallpaper_dir = Path::new("~/onedrive/wallpapers");
///    let wallpaper = pick_random_wallpaper(wallpaper_dir)?;
///    assert_eq!(Option<walkdir::DirEntry>, wallpaper);
/// ```
fn pick_random_wallpaper(dir: &Path) -> Result<Option<walkdir::DirEntry>, Box<dyn Error>> {
    let mut candidates = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        // Filter using the borrowed DirEntry to avoid early allocations
        .filter(|entry| entry.file_type().is_file() && is_supported_image(entry));

    let mut chosen: Option<walkdir::DirEntry> = candidates.next();
    let mut rng = fastrand::Rng::new();
    let mut w: f64 = rng.f64();

    // Reservoir sampling for a single item
    loop {
        // Calculate skip distance using floating-point logarithms
        let skip = (rng.f64().ln() / (1.0 - w).ln()) as usize;

        // Simulate iterator `.nth()` stepping overhead
        if let Some(item) = candidates.nth(skip) {
            chosen = Some(item);
            w *= rng.f64();
        } else {
            break;
        }
    }

    Ok(chosen)
}

/// Checks if the provided path is one of the supported image formats.
///
/// Current supported image formats:
///   - jpg/jpeg
///   - png
///   - webp
///
fn is_supported_image(entry: &walkdir::DirEntry) -> bool {
    matches!(
        entry.path().extension().and_then(|e| e.to_str()),
        Some(ext) if matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "webp")
    )
}

fn default_wallpaper_dir() -> PathBuf {
    home_dir()
        .map(|h| h.join("onedrive/wallpapers"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_no_xinerama_flag() {
        let args = Args::try_parse_from(["wallpaper", "--no-xinerama"]).unwrap();
        assert!(args.no_xinerama);
    }

    #[test]
    fn defaults_no_xinerama_to_false() {
        let args = Args::try_parse_from(["wallpaper"]).unwrap();
        assert!(!args.no_xinerama);
    }
}
