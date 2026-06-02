//! # luksctl
//!
//! A utility to control encrypted luks_file containers.
//!
//! # Examples
//!
//! ```fish
//! luksctl open shadow
//!
//! # or
//!
//! luksctl status shadow
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Args as ClapArgs, Parser, Subcommand};
use shared::{command_exists, home_dir};

/// Common arguments for all subcommands
#[derive(ClapArgs, Debug, Clone)]
struct ContainerArgs {
    /// Path to a LUKS container file
    luks_file: PathBuf,

    /// Optional custom mount point (default: ~/container_name)
    mount_point: Option<PathBuf>,
}

/// Sub-command enum
#[derive(Subcommand, Debug)]
enum Action {
    /// Open and mount a LUKS container
    Open(ContainerArgs),
    /// Unmount and close a LUKS container
    Close(ContainerArgs),
    /// Show status of a LUKS container
    Status(ContainerArgs),
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Open, close, or inspect LUKS container files", long_about = None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

/// Privilege Escalation Runner
enum PrivilegeRunner {
    Sudo,
    Run0,
    None,
}

/// Main entry point for binary
fn main() -> Result<()> {
    let args = Args::parse();
    let privilege_runner = detect_privilege_runner();

    // Extract common args regardless of action
    let container_args = match &args.action {
        Action::Open(c) | Action::Close(c) | Action::Status(c) => c,
    };

    // Perform shared validation and setup
    ensure_luks_file_exists(&container_args.luks_file)?;
    let luks_name = luks_name_from_path(&container_args.luks_file)?;
    let mount_point = resolve_mount_point(&luks_name, container_args.mount_point.clone())?;

    // Execute the specific action
    match args.action {
        Action::Status(_) => status(&container_args.luks_file, &luks_name, &mount_point),
        Action::Open(_) => open_luks(
            &container_args.luks_file,
            &luks_name,
            &mount_point,
            &privilege_runner,
        ),
        Action::Close(_) => close_luks(
            &container_args.luks_file,
            &luks_name,
            &mount_point,
            &privilege_runner,
        ),
    }
}

fn ensure_luks_file_exists(luks_file: &Path) -> Result<()> {
    if !luks_file.is_file() {
        bail!("LUKS file '{}' does not exist", luks_file.display());
    }
    Ok(())
}

fn resolve_mount_point(luks_name: &str, mount_point: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = mount_point {
        Ok(path)
    } else {
        Ok(home_dir()
            .context("Failed to locate home directory")?
            .join(luks_name))
    }
}

/// Check which privilege escalation command to use
fn detect_privilege_runner() -> PrivilegeRunner {
    if command_exists("sudo") {
        PrivilegeRunner::Sudo
    } else if command_exists("run0") {
        PrivilegeRunner::Run0
    } else {
        PrivilegeRunner::None
    }
}

/// Construct privileged_command
fn privileged_command(privilege_runner: &PrivilegeRunner, program: &str) -> Command {
    let mut cmd;
    match privilege_runner {
        PrivilegeRunner::Sudo => {
            cmd = Command::new("sudo");
            cmd.arg(program);
        }
        PrivilegeRunner::Run0 => {
            cmd = Command::new("run0");
            cmd.arg(program);
        }
        PrivilegeRunner::None => {
            cmd = Command::new(program);
        }
    }
    cmd
}

/// Generate a name for the given luks file
fn luks_name_from_path(luks_file: &Path) -> Result<String> {
    let file_name = luks_file
        .file_name()
        .and_then(|name| name.to_str())
        .context("Unable to derive LUKS name from file path")?;

    Ok(file_name
        .strip_suffix(".luks")
        .unwrap_or(file_name)
        .to_owned())
}

/// Check status of given luks container
fn status(luks_file: &Path, luks_name: &str, mount_point: &Path) -> Result<()> {
    println!("LUKS Container: {}", luks_file.display());
    println!("Device Name: {luks_name}");
    println!("Mount Point: {}", mount_point.display());

    if is_device_open(luks_name) {
        println!("Status: Device is OPEN");
        if is_mounted(mount_point) {
            println!("Mount Status: MOUNTED at {}", mount_point.display());
            println!("Available Space:");
            if let Some(line) = df_last_line(mount_point)? {
                println!("{line}");
            }
        } else {
            println!("Mount Status: NOT MOUNTED");
        }
    } else {
        println!("Status: Device is CLOSED");
    }

    Ok(())
}

/// Open a luks container
fn open_luks(
    luks_file: &Path,
    luks_name: &str,
    mount_point: &Path,
    privilege_runner: &PrivilegeRunner,
) -> Result<()> {
    let mut opened_in_this_run = false;

    if is_device_open(luks_name) {
        if is_mounted(mount_point) {
            bail!(
                "LUKS device '{luks_name}' is already open and mounted at '{}'",
                mount_point.display()
            );
        }
        println!("Device is open but not mounted. Attempting to mount...");
    } else {
        if !is_luks_encrypted(luks_file)? {
            bail!(
                "File '{}' is not a valid LUKS container",
                luks_file.display()
            );
        }

        println!("Opening LUKS container...");
        let open_status = privileged_command(privilege_runner, "cryptsetup")
            .args(["open", "--type", "luks"])
            .arg(luks_file)
            .arg(luks_name)
            .status()?;

        if !open_status.success() {
            bail!("Failed to open LUKS device. Check password and try again.");
        }
        opened_in_this_run = true;
    }

    if let Err(err) = fs::create_dir_all(mount_point) {
        if opened_in_this_run {
            let _ = privileged_command(privilege_runner, "cryptsetup")
                .args(["close", luks_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        bail!(
            "Failed to create mount point '{}': {err}",
            mount_point.display()
        );
    }

    println!("Mounting filesystem...");
    let mount_status = privileged_command(privilege_runner, "mount")
        .arg(format!("/dev/mapper/{luks_name}"))
        .arg(mount_point)
        .status()?;

    if !mount_status.success() {
        if opened_in_this_run {
            let _ = privileged_command(privilege_runner, "cryptsetup")
                .args(["close", luks_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        bail!("Failed to mount filesystem. Device may be corrupted.");
    }

    println!(
        "LUKS container '{}' opened and mounted at '{}'",
        luks_file.display(),
        mount_point.display()
    );

    println!("Available space:");
    if let Some(line) = df_last_line(mount_point)? {
        println!("{line}");
    }

    Ok(())
}

/// Close the given luks container
fn close_luks(
    luks_file: &Path,
    luks_name: &str,
    mount_point: &Path,
    privilege_runner: &PrivilegeRunner,
) -> Result<()> {
    let mut cleanup_needed = false;

    if is_mounted(mount_point) {
        println!("Unmounting filesystem...");
        let umount_status = privileged_command(privilege_runner, "umount")
            .arg(mount_point)
            .status()?;

        if !umount_status.success() {
            bail!("Failed to unmount filesystem. Files may be in use.");
        }
        cleanup_needed = true;
    }

    if is_device_open(luks_name) {
        println!("Closing LUKS device...");
        let close_status = privileged_command(privilege_runner, "cryptsetup")
            .args(["close", luks_name])
            .status()?;

        if !close_status.success() {
            bail!("Failed to close LUKS device.");
        }
        cleanup_needed = true;
    }

    if cleanup_needed && mount_point.is_dir() {
        let _ = fs::remove_dir(mount_point);
    }

    if cleanup_needed {
        println!(
            "LUKS container '{}' closed successfully",
            luks_file.display()
        );
    } else {
        println!("LUKS container '{luks_name}' was not open");
    }

    Ok(())
}

/// Check if a file is luks encrypted
fn is_luks_encrypted(device: &Path) -> Result<bool> {
    let status = Command::new("cryptsetup")
        .arg("isLuks")
        .arg(device)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Ok(status.success())
}

/// Check if the luks file is open
fn is_device_open(luks_name: &str) -> bool {
    Path::new("/dev/mapper").join(luks_name).exists()
}

/// Check if the luks file is mounted
fn is_mounted(mount_point: &Path) -> bool {
    let Ok(mounts) = fs::read_to_string("/proc/mounts") else {
        return false;
    };

    let mount_str = mount_point.to_string_lossy();

    mounts.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next();
        parts.next() == Some(&mount_str)
    })
}

/// Get available space in the given `mount_point`
fn df_last_line(mount_point: &Path) -> Result<Option<String>> {
    let output = Command::new("df")
        .arg("-h")
        .arg(mount_point)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    // Safer parsing: Use from_utf8_lossy to avoid panics on weird characters
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(ToOwned::to_owned))
}

// Note: Test suite needs minor adjustments to reflect `Action::Open(args)` struct syntax instead of named fields.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::*;

    #[test]
    fn luks_name_from_path_strips_suffix() {
        let path = PathBuf::from("/tmp/secret.luks");
        assert_eq!(luks_name_from_path(&path).unwrap(), "secret");
    }

    #[test]
    fn luks_name_from_path_no_suffix() {
        let path = PathBuf::from("/tmp/secretfile");
        assert_eq!(luks_name_from_path(&path).unwrap(), "secretfile");
    }

    #[test]
    fn luks_name_from_path_errors_on_invalid() {
        let path = PathBuf::from("");
        assert!(luks_name_from_path(&path).is_err());
    }

    #[test]
    fn parses_open_with_required_args() {
        let args = Args::try_parse_from(["luksctl", "open", "/tmp/secret.luks"]).unwrap();
        let container_args = match &args.action {
            Action::Open(c) | Action::Close(c) | Action::Status(c) => c,
        };

        match args.action {
            Action::Open(_) => {
                assert_eq!(container_args.luks_file, PathBuf::from("/tmp/secret.luks"));
                assert!(container_args.mount_point.is_none());
            }
            _ => panic!("expected open action"),
        }
    }

    #[test]
    fn parses_close_with_custom_mount_point() {
        let args =
            Args::try_parse_from(["luksctl", "close", "/tmp/secret.luks", "/mnt/secret"]).unwrap();
        let container_args = match &args.action {
            Action::Open(c) | Action::Close(c) | Action::Status(c) => c,
        };

        match args.action {
            Action::Close(_) => {
                assert_eq!(container_args.luks_file, PathBuf::from("/tmp/secret.luks"));
                assert_eq!(
                    container_args.mount_point,
                    Some(PathBuf::from("/mnt/secret"))
                );
            }
            _ => panic!("expected close action"),
        }
    }

    #[test]
    fn errors_on_invalid_subcommand() {
        assert!(Args::try_parse_from(["luksctl", "unlock", "/tmp/secret.luks"]).is_err());
    }
}
