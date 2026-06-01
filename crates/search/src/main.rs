//! # search
//!
//! A utility to search the web using `rofi` input.
//!
//! Notably this search utility was designed to work in the same way as [DuckDuckgo bangs](https://duckduckgo.com/bangs),
//! so `search term` will search google, but `!gh search term` will search Github.
//!
//! By default no help is shown, pressing `Alt + h` will close `search` and reopen with all
//! bangs/engines showing.
//!
//! # Examples
//!
//! ```bash
//! # Search using rofi
//! search
//! ```

use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

use clap::Parser;
use urlencoding::encode;

#[derive(Parser, Debug)]
#[command(author, version, about = "A utility to search the web using rofi input.", long_about = None)]
struct Args {
    /// Start with engine help entries shown in rofi
    #[arg(long, default_value_t = false)]
    help_menu: bool,
}

/// Search engine struct
#[derive(Clone, Copy, Debug)]
struct Engine {
    key: &'static str,
    search_url: &'static str,
    home_url: &'static str,
    description: &'static str,
}

/// Default search engine
///
/// Currently [google.com](https://google.com).
const DEFAULT_ENGINE: Engine = Engine {
    key: "default",
    search_url: "https://www.google.com/search?q=",
    home_url: "https://www.google.com",
    description: "google",
};

/// All supported search engines
const ENGINES: &[Engine] = &[
    Engine {
        key: "gh",
        search_url: "https://github.com/search?q=",
        home_url: "https://github.com",
        description: "github",
    },
    Engine {
        key: "rd",
        search_url: "https://www.rdocumentation.org/search?q=",
        home_url: "https://www.rdocumentation.org",
        description: "rdocumentation",
    },
    Engine {
        key: "rs",
        search_url: "https://docs.rs/releases/search?query=",
        home_url: "https://docs.rs",
        description: "rust docs",
    },
    Engine {
        key: "r",
        search_url: "https://reddit.com/search?q=",
        home_url: "https://reddit.com",
        description: "reddit",
    },
    Engine {
        key: "yt",
        search_url: "https://www.youtube.com/results?search_query=",
        home_url: "https://www.youtube.com",
        description: "youtube",
    },
    Engine {
        key: "t",
        search_url: "https://twitch.tv/search?term=",
        home_url: "https://twitch.tv",
        description: "twitch",
    },
    Engine {
        key: "sp",
        search_url: "https://open.spotify.com/search/",
        home_url: "https://open.spotify.com",
        description: "spotify",
    },
    Engine {
        key: "ddg",
        search_url: "https://duckduckgo.com/?q=",
        home_url: "https://duckduckgo.com",
        description: "duckduckgo",
    },
    Engine {
        key: "w",
        search_url: "https://www.wikipedia.org/wiki/",
        home_url: "https://www.wikipedia.org",
        description: "wikipedia",
    },
];

/// Result of rofi window close
enum MenuResult {
    Selected(String),
    ToggleHelp,
    Cancelled,
}

/// Main entry point for binary
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let full_menu_text = build_menu_text();
    let mut show_help = args.help_menu;

    let selection = loop {
        let menu_text = if show_help { &full_menu_text } else { "" };

        match run_menu(menu_text)? {
            MenuResult::Selected(sel) => break sel,
            MenuResult::ToggleHelp => {
                show_help = !show_help;
                continue;
            }
            MenuResult::Cancelled => return Ok(()),
        }
    };

    let (engine_key, search_terms) = parse_selection(&selection);

    let target = if engine_key == "default" && is_direct_url(search_terms) {
        // If it looks like a URL, bypass the search engine entirely.
        if search_terms.starts_with("http") {
            search_terms.to_owned()
        } else {
            format!("https://{}", search_terms)
        }
    } else {
        // Otherwise, proceed with the normal search engine logic.
        let engine = find_engine(engine_key).unwrap_or(&DEFAULT_ENGINE);
        if search_terms.is_empty() {
            engine.home_url.to_owned()
        } else {
            format!(
                "{}{}",
                engine.search_url,
                encode(search_terms).replace("%20", "+")
            )
        }
    };

    Command::new("xdg-open")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

/// Construct menu text
///
/// This takes the default engine, plus all additional engines and converts it to a [`String`] to be
/// used in constructing the rofi help menu.
///
fn build_menu_text() -> String {
    let mut capacity = DEFAULT_ENGINE.description.len() + 10;
    for e in ENGINES {
        capacity += e.key.len() + e.description.len() + 4;
    }

    let mut output = String::with_capacity(capacity);
    output.push_str("default: ");
    output.push_str(DEFAULT_ENGINE.description);

    for engine in ENGINES {
        output.push_str("\n!");
        output.push_str(engine.key);
        output.push_str(": ");
        output.push_str(engine.description);
    }

    output
}

/// Rofi menu loop
fn run_menu(menu_text: &str) -> Result<MenuResult, Box<dyn Error>> {
    // Rofi arguments, alt-h is used to exit and switch to help mode
    let args = vec!["-dmenu", "-sync", "-p", "search:", "-kb-custom-1", "Alt+h"];

    let mut child = match Command::new("rofi")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err("Launcher 'rofi' not found on your system".into());
        }
        Err(err) => return Err(err.into()),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(menu_text.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let status_code = output.status.code().unwrap_or(0);

    // Rofi custom keybinds start at exit code 10
    if status_code == 10 {
        return Ok(MenuResult::ToggleHelp);
    }

    // Standard cancellation (Escape key or clicking away)
    if !output.status.success() {
        return Ok(MenuResult::Cancelled);
    }

    let selection = String::from_utf8(output.stdout)?;
    let trimmed = selection.trim();

    if trimmed.is_empty() {
        Ok(MenuResult::Cancelled)
    } else {
        Ok(MenuResult::Selected(trimmed.to_owned()))
    }
}

/// Return the engine associated with a given key
fn find_engine(key: &str) -> Option<&'static Engine> {
    ENGINES.iter().find(|engine| engine.key == key)
}

/// Parse the generated selection.
///
/// This function strips out the bang `!gh` and returns a tuple with the engine key `gh` and the
/// `search` term. If no bang is found in the search term, return the key for the [`DEFAULT_ENGINE`].
///
/// Returns string slices to avoid heap allocation.
fn parse_selection(selection: &str) -> (&str, &str) {
    let selection = selection.trim();

    if let Some(content) = selection.strip_prefix('!') {
        let (bang, rest) = content.split_once(' ').unwrap_or((content, ""));
        return (bang, rest.trim());
    }

    if selection.starts_with("Default") {
        // Handle your "Default: description" logic if still needed,
        // otherwise just treat as a standard search.
        let rest = selection.strip_prefix("Default").unwrap_or_default().trim();
        return ("default", rest);
    }

    ("default", selection)
}

/// Check if search term is a plain url.
fn is_direct_url(query: &str) -> bool {
    match query {
        // First, check for spaces. If found, it's a search.
        _ if query.contains(' ') => false,

        // Check for protocols.
        _ if query.starts_with("http://") || query.starts_with("https://") => true,

        // Finally, check for the dot.
        _ => query.contains('.'),
    }
}

#[cfg(test)]
mod tests {
    use super::{Args, parse_selection};
    use clap::Parser;

    #[test]
    fn parses_direct_bang_command() {
        assert_eq!(parse_selection("!gh rust traits"), ("gh", "rust traits"));
    }

    #[test]
    fn parses_plain_default_search() {
        assert_eq!(
            parse_selection("rust closures"),
            ("default", "rust closures")
        );
    }

    #[test]
    fn parses_default_menu_search_terms() {
        assert_eq!(
            parse_selection("Default rust borrow checker"),
            ("default", "rust borrow checker")
        );
    }

    #[test]
    fn parses_help_menu_flag() {
        let args = Args::try_parse_from(["search", "--help-menu"]).unwrap();
        assert!(args.help_menu);
    }

    #[test]
    fn defaults_help_menu_to_false() {
        let args = Args::try_parse_from(["search"]).unwrap();
        assert!(!args.help_menu);
    }
}
