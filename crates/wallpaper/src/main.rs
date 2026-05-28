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

use std::env;
use std::error::Error;
use std::path::Path;
use std::process::Command;

use walkdir::WalkDir;

use shared::home_dir;

/// Main entry point for binary.
fn main() -> Result<(), Box<dyn Error>> {
    let no_xinerama = env::args().skip(1).any(|arg| arg == "--no-xinerama");
    let wallpaper_dir = home_dir()?.join("onedrive/wallpapers");
    let wallpaper = pick_random_wallpaper(&wallpaper_dir)?
        .ok_or_else(|| format!("No wallpaper files found in {}", wallpaper_dir.display()))?;

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
        if no_xinerama {
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
/// R](https://en.wikipedia.org/wiki/Reservoir_sampling#Simple:_Algorithm_R).
///
/// I investigated algorithm L, but for selecting only 1 element algorithm L is actually
/// less efficient than algorithm R (because of CPU floating point operations not inherently),
/// hence we keep algorithm R here.
/// If I ever wanted to select more than 1 file at a time then algorithm L is likely more efficient
/// (depending on the size of the wallpaper directory).
/// Likely algorithm R will be more efficient until the k I want to select is in the thousands
/// and the n I'm selecting from is in the 10s of thousands
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
    let mut w: f64 = fastrand::f64();

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
    let name = entry.file_name();
    let bytes = name.as_encoded_bytes();
    
    // Scan backwards from the end of the filename bytes for the dot
    if let Some(dot_idx) = bytes.iter().rposition(|&b| b == b'.') {
        let ext = &bytes[dot_idx + 1..];
        return ext.eq_ignore_ascii_case(b"jpg")
            || ext.eq_ignore_ascii_case(b"jpeg")
            || ext.eq_ignore_ascii_case(b"png")
            || ext.eq_ignore_ascii_case(b"webp");
    }
    false
}
