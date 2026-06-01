//! # Shared Utils
//!
//! A collection of common utilities for path manipulation and environment
//! checking. This crate provides safe wrappers around system calls.
//!
//! ## Usage
//! Add this to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! shared = "0.1.0"
//! ```

use std::{env, error::Error, path::PathBuf};

/// Check if an OS command exists on the system.
///
/// # Examples
///
/// ```
/// use shared::command_exists;
///
/// if !command_exists("dunstctl") {
///     println!("dunstctl not available");
/// }
/// ```
pub fn command_exists(cmd: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    for mut path in env::split_paths(&paths) {
        // --- Linux / macOS / Unix compilation branch ---
        #[cfg(unix)]
        {
            path.push(cmd);
            if path.exists() {
                return true;
            }
        }

        // --- Windows compilation branch ---
        #[cfg(windows)]
        {
            if cmd.ends_with(".exe") {
                path.push(cmd);
                if path.exists() {
                    return true;
                }
            } else {
                path.push(cmd);
                if path.exists() {
                    return true;
                }
                path.set_extension("exe");
                if path.exists() {
                    return true;
                }
            }
        }
    }

    false
}

/// Returns the path to the current user's home directory.
///
/// This function looks up the `HOME` environment variable.
///
/// # Errors
///
/// Returns an error if:
/// * The `HOME` environment variable is not set.
/// * The value contains invalid Unicode (if using `var` instead of `var_os`).
///
/// # Examples
///
/// ```
/// use shared::home_dir;
/// use std::path::PathBuf;
///
/// // Note: This test might fail in environments without HOME set
/// if let Ok(path) = home_dir() {
///     println!("My home is at: {}", path.display());
/// }
/// ```
pub fn home_dir() -> Result<PathBuf, Box<dyn Error>> {
    let home = env::var_os("HOME").ok_or("HOME is not set")?;

    Ok(PathBuf::from(home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_false_for_nonexistent_command() {
        assert!(!command_exists("definitely_not_a_real_command_12345"));
    }

    #[test]
    fn returns_true_for_likely_existing_command() {
        // This test may fail on very minimal systems, but "sh" is almost always present.
        assert!(command_exists("sh"));
    }
}
