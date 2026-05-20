use std::env;
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
struct Engine {
    key: &'static str,
    search_url: &'static str,
    home_url: &'static str,
    description: &'static str,
}

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
    Engine {
        key: "default",
        search_url: "https://www.google.com/search?q=",
        home_url: "https://www.google.com",
        description: "google",
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
    let engine = find_engine(&engine_key).unwrap_or(default_engine());

    let target = if search_terms.is_empty() {
        engine.home_url.to_owned()
    } else {
        format!("{}{}", engine.search_url, url_encode(&search_terms))
    };

    Command::new("xdg-open")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

fn build_menu_text() -> String {
    let mut lines = Vec::with_capacity(ENGINES.len());
    lines.push(format!("Default: {}", default_engine().description));

    for engine in ENGINES.iter().filter(|engine| engine.key != "default") {
        lines.push(format!("!{}: {}", engine.key, engine.description));
    }

    lines.join("\n")
}

fn run_menu(menu_text: &str) -> Result<Option<String>, Box<dyn Error>> {
    let session_type = env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let launcher = if session_type == "wayland" {
        Launcher::new("fuzzel", &["--dmenu", "--prompt", "search:"])
    } else {
        Launcher::new(
            "rofi",
            &["-dmenu", "-p", "search:", "-mesg", "search options"],
        )
    };

    if which::which(launcher.program).is_err() {
        return Err(format!("{} not found", launcher.program).into());
    }

    let mut child = Command::new(launcher.program)
        .args(launcher.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(menu_text.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let selection = String::from_utf8(output.stdout)?.trim().to_owned();
    if selection.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selection))
    }
}

fn parse_selection(selection: &str) -> (String, String) {
    if let Some((menu_choice, after_colon)) = selection.split_once(':') {
        let trimmed_after = after_colon.trim();

        if let Some(bang) = menu_choice.strip_prefix('!') {
            let expected_desc = find_engine(bang)
                .map(|engine| engine.description)
                .unwrap_or_default();

            let search_terms = if trimmed_after == expected_desc {
                String::new()
            } else {
                trimmed_after.to_owned()
            };

            return (bang.to_owned(), search_terms);
        }

        if menu_choice.starts_with("Default") {
            let search_terms = if trimmed_after == default_engine().description {
                String::new()
            } else {
                trimmed_after.to_owned()
            };

            return ("default".to_owned(), search_terms);
        }
    }

    let mut parts = selection.splitn(2, ' ');
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();

    if let Some(bang) = first.strip_prefix('!') {
        (bang.to_owned(), rest.to_owned())
    } else {
        ("default".to_owned(), selection.trim().to_owned())
    }
}

fn find_engine(key: &str) -> Option<&'static Engine> {
    ENGINES.iter().find(|engine| engine.key == key)
}

fn default_engine() -> &'static Engine {
    ENGINES
        .iter()
        .find(|engine| engine.key == "default")
        .expect("default engine must exist")
}

fn url_encode(input: &str) -> String {
    urlencoding::encode(input).replace("%20", "+")
}

struct Launcher<'a> {
    program: &'a str,
    args: &'a [&'a str],
}

impl<'a> Launcher<'a> {
    const fn new(program: &'a str, args: &'a [&'a str]) -> Self {
        Self { program, args }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_selection;

    #[test]
    fn parses_direct_bang_command() {
        assert_eq!(
            parse_selection("!gh rust traits"),
            ("gh".into(), "rust traits".into())
        );
    }

    #[test]
    fn parses_plain_default_search() {
        assert_eq!(
            parse_selection("rust closures"),
            ("default".into(), "rust closures".into())
        );
    }

    #[test]
    fn treats_menu_description_as_empty_query() {
        assert_eq!(parse_selection("!gh: github"), ("gh".into(), String::new()));
    }

    #[test]
    fn parses_menu_bang_search_terms() {
        assert_eq!(
            parse_selection("!gh: rust ownership"),
            ("gh".into(), "rust ownership".into())
        );
    }

    #[test]
    fn parses_default_menu_search_terms() {
        assert_eq!(
            parse_selection("Default: rust borrow checker"),
            ("default".into(), "rust borrow checker".into())
        );
    }

    #[test]
    fn treats_default_menu_description_as_empty_query() {
        assert_eq!(
            parse_selection("Default: google"),
            ("default".into(), String::new())
        );
    }
}
