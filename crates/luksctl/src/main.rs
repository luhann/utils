use shared::{command_exists, home_dir};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Sub-command enum
enum Action {
    Open,
    Close,
    Status,
}

/// Privilege Escalation Runner
enum PrivilegeRunner {
    Sudo,
    Run0,
    None,
}

/// Help menu
const USAGE: &str = concat!(
    "Usage: {program} (open|close|status) /path/to/luks_file [mount_point]\n",
    "\n",
    "Commands:\n",
    "  open    - Open and mount LUKS container\n",
    "  close   - Unmount and close LUKS container\n",
    "  status  - Show status of LUKS container\n",
    "\n",
    "Arguments:\n",
    "  luks_file    - Path to LUKS container file\n",
    "  mount_point  - Optional custom mount point (default: ~/container_name)\n",
);

/// Main entry point for binary
fn main()  -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 || args[1] == "-h" || args[1] == "--help" {
        println!("{}", USAGE);
        return Ok(());
    }

    if args.len() < 3 {
        println!("{}", USAGE);
        return Err("Insufficient arguments".into());
    }

    let action = parse_action(&args[1])?;
    let luks_file = PathBuf::from(&args[2]);

    if !luks_file.is_file() {
        return Err(format!("LUKS file '{}' does not exist", luks_file.display()).into());
    }

    let luks_name = luks_name_from_path(&luks_file)?;
    let privilege_runner = detect_privilege_runner();
    let mount_point = if let Some(custom_mount_point) = args.get(3) {
        PathBuf::from(custom_mount_point)
    } else {
        home_dir()?.join(&luks_name)
    };

    match action {
        Action::Status => status(&luks_file, &luks_name, &mount_point),
        Action::Open => open_luks(&luks_file, &luks_name, &mount_point, &privilege_runner),
        Action::Close => close_luks(&luks_file, &luks_name, &mount_point, &privilege_runner),
    }
}

/// Check which privilege escalation command to use
///
/// Prefer `sudo` if it is installed otherwise use `run0`.
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
    match privilege_runner {
        PrivilegeRunner::Sudo => {
            let mut command = Command::new("sudo");
            command.arg(program);
            command
        }
        PrivilegeRunner::Run0 => {
            let mut command = Command::new("run0");
            command.arg(program);
            command
        }
        PrivilegeRunner::None => Command::new(program),
    }
}

/// Parse arguments to determine action
fn parse_action(action: &str) -> Result<Action, Box<dyn Error>> {
    match action {
        "open" => Ok(Action::Open),
        "close" => Ok(Action::Close),
        "status" => Ok(Action::Status),
        _ => Err(format!("Invalid action '{action}'. Use 'open', 'close', or 'status'.").into()),
    }
}

/// Generate a name for the given luks file
fn luks_name_from_path(luks_file: &Path) -> Result<String, Box<dyn Error>> {
    let file_name = luks_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Unable to derive LUKS name from file path")?;

    Ok(file_name
        .strip_suffix(".luks")
        .unwrap_or(file_name)
        .to_owned())
}

/// Check status of given luks container
fn status(luks_file: &Path, luks_name: &str, mount_point: &Path) -> Result<(), Box<dyn Error>> {
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
///
/// Mount the given luks container at the given `mount_point` by default it mounts the container in
/// the user's home_dir.
fn open_luks(
    luks_file: &Path,
    luks_name: &str,
    mount_point: &Path,
    privilege_runner: &PrivilegeRunner,
) -> Result<(), Box<dyn Error>> {
    let mut opened_in_this_run = false;

    if is_device_open(luks_name) {
        if is_mounted(mount_point) {
            return Err(format!(
                "LUKS device '{luks_name}' is already open and mounted at '{}'",
                mount_point.display()
            )
            .into());
        }

        println!("Device is open but not mounted. Attempting to mount...");
    } else {
        if !is_luks_encrypted(luks_file)? {
            return Err(format!(
                "File '{}' is not a valid LUKS container",
                luks_file.display()
            )
            .into());
        }

        println!("Opening LUKS container...");
        let open_status = privileged_command(privilege_runner, "cryptsetup")
            .args(["open", "--type", "luks"])
            .arg(luks_file)
            .arg(luks_name)
            .status()?;

        if !open_status.success() {
            return Err("Failed to open LUKS device. Check password and try again.".into());
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
        return Err(format!(
            "Failed to create mount point '{}': {err}",
            mount_point.display()
        )
        .into());
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
        return Err("Failed to mount filesystem. Device may be corrupted.".into());
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
///
/// Also removes the `mount_point` after closing the container.
fn close_luks(
    luks_file: &Path,
    luks_name: &str,
    mount_point: &Path,
    privilege_runner: &PrivilegeRunner,
) -> Result<(), Box<dyn Error>> {
    let mut cleanup_needed = false;

    if is_mounted(mount_point) {
        println!("Unmounting filesystem...");
        let umount_status = privileged_command(privilege_runner, "umount")
            .arg(mount_point)
            .status()?;
        if !umount_status.success() {
            return Err("Failed to unmount filesystem. Files may be in use.".into());
        }
        cleanup_needed = true;
    }

    if is_device_open(luks_name) {
        println!("Closing LUKS device...");
        let close_status = privileged_command(privilege_runner, "cryptsetup")
            .args(["close", luks_name])
            .status()?;
        if !close_status.success() {
            return Err("Failed to close LUKS device.".into());
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
fn is_luks_encrypted(device: &Path) -> Result<bool, Box<dyn Error>> {
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

    // Standardize the path to a string to match against /proc/mounts text
    let mount_str = mount_point.to_string_lossy();

    // Each line looks like: /dev/mapper/luks_name /home/user/mount_point ext4 rw...
    // We check if our mount point is listed as the second item on any line
    mounts.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next();
        parts.next() == Some(&mount_str) // Check the mount target
    })
}

/// Get available space in the given `mount_point`
fn df_last_line(mount_point: &Path) -> Result<Option<String>, Box<dyn Error>> {
    let output = Command::new("df")
        .arg("-h")
        .arg(mount_point)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(ToOwned::to_owned))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
