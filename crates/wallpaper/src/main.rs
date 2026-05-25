use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let no_xinerama = env::args().skip(1).any(|arg| arg == "--no-xinerama");
    let wallpaper_dir = home_dir()?.join("onedrive/wallpapers");
    let wallpaper = pick_random_wallpaper(&wallpaper_dir)?
        .ok_or_else(|| format!("No wallpaper files found in {}", wallpaper_dir.display()))?;

    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();

    if session_type != "wayland" {
        let mut command = Command::new("feh");
        command.arg("--no-fehbg");
        if no_xinerama {
            command.arg("--no-xinerama");
        }
        command.arg("--bg-fill").arg(&wallpaper);

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

        println!("Set X11 wallpaper to: {}", wallpaper.display());
    } else {
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
            .arg(&wallpaper)
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

        println!("Set Wayland wallpaper to: {}", wallpaper.display());
    }

    Ok(())
}

fn home_dir() -> Result<PathBuf, Box<dyn Error>> {
    let home = env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home))
}

// Reservoir sampling - Algorithm R
fn pick_random_wallpaper(dir: &Path) -> Result<Option<PathBuf>, Box<dyn Error>> {
    // I investigated algorithm L, but for selecting only 1 element algorithm L is actually
    // less efficient than algorithm R (because of CPU floating point operations not inherently),
    // hence we keep algorithm R here.
    // If I ever wanted to select more than 1 file at a time then algorithm L is more efficient.
    // Likely algorithm R will be more efficient until the k I want to select is in the thousands
    // and the n I'm selecting from is in the 10s of thousands
    let candidates = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        // Filter using the borrowed DirEntry to avoid early allocations
        .filter(|entry| entry.file_type().is_file() && is_supported_image(entry.path()));

    let mut chosen = None;

    // Reservoir sampling for a single item
    for (i, entry) in candidates.enumerate() {
        if fastrand::usize(..=i) == 0 {
            // Allocate the PathBuf ONLY when a file wins the reservoir slot
            chosen = Some(entry.into_path());
        }
    }

    Ok(chosen)
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            ext.eq_ignore_ascii_case("jpg")
                || ext.eq_ignore_ascii_case("jpeg")
                || ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("webp")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supports_common_image_extensions() {
        assert!(is_supported_image(Path::new("foo.jpg")));
        assert!(is_supported_image(Path::new("foo.jpeg")));
        assert!(is_supported_image(Path::new("foo.png")));
        assert!(is_supported_image(Path::new("foo.webp")));
    }

    #[test]
    fn rejects_unsupported_extensions() {
        assert!(!is_supported_image(Path::new("foo.txt")));
        assert!(!is_supported_image(Path::new("foo")));
    }
}
