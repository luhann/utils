use rand::seq::IteratorRandom;
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
        let status = Command::new("swww")
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
            .status()?;

        if !status.success() {
            return Err("swww failed".into());
        }

        println!("Set Wayland wallpaper to: {}", wallpaper.display());
    }

    Ok(())
}

fn home_dir() -> Result<PathBuf, Box<dyn Error>> {
    let home = env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home))
}

fn pick_random_wallpaper(dir: &Path) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let candidates = WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file())
        .filter(|path| is_supported_image(path));

    Ok(candidates.choose(&mut rng))
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("webp")
    )
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
