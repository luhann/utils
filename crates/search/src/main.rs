use std::env;
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};
use urlencoding::encode;

#[derive(Clone, Copy, Debug)]
struct Engine {
    key: &'static str,
    search_url: &'static str,
    home_url: &'static str,
    description: &'static str,
}

const DEFAULT_ENGINE: Engine = Engine {
    key: "default",
    search_url: "https://www.google.com/search?q=",
    home_url: "https://www.google.com",
    description: "google",
};

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

fn main() {
    if let Err(err) = run() {
        eprintln!("search-rs: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let menu_text = build_menu_text();
    let Some(selection) = run_menu(&menu_text)? else {
        return Ok(());
    };

    let (engine_key, search_terms) = parse_selection(&selection);
    let engine = find_engine(engine_key).unwrap_or(&DEFAULT_ENGINE);

    let target = if search_terms.is_empty() {
        engine.home_url.to_owned()
    } else {
        format!(
            "{}{}",
            engine.search_url,
            encode(search_terms).replace("%20", "+")
        )
    };

    Command::new("xdg-open")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

fn build_menu_text() -> String {
    let mut capacity = DEFAULT_ENGINE.description.len() + 10;
    for e in ENGINES {
        capacity += e.key.len() + e.description.len() + 4;
    }

    let mut output = String::with_capacity(capacity);
    output.push_str("Default: ");
    output.push_str(DEFAULT_ENGINE.description);

    for engine in ENGINES {
        output.push_str("\n!");
        output.push_str(engine.key);
        output.push_str(": ");
        output.push_str(engine.description);
    }

    output
}

fn run_menu(menu_text: &str) -> Result<Option<String>, Box<dyn Error>> {
    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let (program, args): (&str, &[&str]) = if session_type == "wayland" {
        ("fuzzel", &["--dmenu", "--prompt", "search:"])
    } else {
        (
            "rofi",
            &["-dmenu", "-p", "search:", "-mesg", "search options"],
        )
    };

    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("Launcher '{program}' not found on your system").into());
        }
        Err(err) => return Err(err.into()),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(menu_text.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Ok(None);
    }

    let selection = String::from_utf8(output.stdout)?;
    let trimmed = selection.trim();

    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_owned()))
    }
}

// Memory Optimization: Shift from returning String to returning zero-allocation &str slices
fn parse_selection(selection: &str) -> (&str, &str) {
    if let Some((menu_choice, after_colon)) = selection.split_once(':') {
        let trimmed_after = after_colon.trim();

        if let Some(bang) = menu_choice.strip_prefix('!') {
            let expected_desc = find_engine(bang)
                .map(|engine| engine.description)
                .unwrap_or_default();

            let search_terms = if trimmed_after == expected_desc {
                ""
            } else {
                trimmed_after
            };
            return (bang, search_terms);
        }

        if menu_choice.starts_with("Default") {
            let search_terms = if trimmed_after == DEFAULT_ENGINE.description {
                ""
            } else {
                trimmed_after
            };
            return ("default", search_terms);
        }
    }

    let mut parts = selection.splitn(2, ' ');
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();

    if let Some(bang) = first.strip_prefix('!') {
        (bang, rest)
    } else {
        ("default", selection.trim())
    }
}

fn find_engine(key: &str) -> Option<&'static Engine> {
    ENGINES.iter().find(|engine| engine.key == key)
}

#[cfg(test)]
mod tests {
    use super::parse_selection;

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
    fn treats_menu_description_as_empty_query() {
        assert_eq!(parse_selection("!gh: github"), ("gh", ""));
    }

    #[test]
    fn parses_menu_bang_search_terms() {
        assert_eq!(
            parse_selection("!gh: rust ownership"),
            ("gh", "rust ownership")
        );
    }

    #[test]
    fn parses_default_menu_search_terms() {
        assert_eq!(
            parse_selection("Default: rust borrow checker"),
            ("default", "rust borrow checker")
        );
    }

    #[test]
    fn treats_default_menu_description_as_empty_query() {
        assert_eq!(parse_selection("Default: google"), ("default", ""));
    }
}
