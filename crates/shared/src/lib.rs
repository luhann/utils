use std::env;

/// Checks if a command executable exists in the system's PATH.
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
