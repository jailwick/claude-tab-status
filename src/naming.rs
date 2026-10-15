//! Pure helpers for reading and composing tab names.

use crate::config::Config;
use crate::status::Status;

/// What zellij appends to the reported name of a tab whose single pane is a held, exited command.
const EXITED_SUFFIX: &str = " [ EXITED ] ";
const EXIT_CODE_PREFIX: &str = " [ EXIT CODE: ";
const EXIT_CODE_END: &str = " ] ";

/// The glyph a tab shows for `status`: the pane title's spinner frame while working, so the tab
/// spins in step with the title, or `working_glyph` when `steady`.
pub fn tab_glyph(status: Status, config: &Config) -> &str {
    match status {
        Status::Working(_) if config.steady => &config.working_glyph,
        Status::Working(frame) => frame,
        Status::Idle => &config.idle_glyph,
    }
}

/// `<glyph> <base>`, or just `base` without a glyph.
pub fn tab_name(base: &str, glyph: Option<&str>) -> String {
    match glyph {
        Some(glyph) => format!("{glyph} {base}"),
        None => base.to_owned(),
    }
}

/// `name` without leading status glyphs (any of `glyphs`, each followed by a space).
pub fn strip_glyph<'a>(mut name: &'a str, glyphs: &[String]) -> &'a str {
    while let Some(rest) = glyphs
        .iter()
        .find_map(|g| name.strip_prefix(g.as_str())?.strip_prefix(' '))
    {
        name = rest;
    }
    name
}

/// `name` without the exit marker zellij adds for held, exited panes, so it never ends up in a
/// name this plugin sets (zellij would append it again and again).
pub fn strip_exit_suffix(name: &str) -> &str {
    if let Some(rest) = name.strip_suffix(EXITED_SUFFIX) {
        return rest;
    }
    name.strip_suffix(EXIT_CODE_END)
        .and_then(|rest| rest.rsplit_once(EXIT_CODE_PREFIX))
        .filter(|(_, code)| code.parse::<i32>().is_ok())
        .map_or(name, |(rest, _)| rest)
}

/// Whether `name` is zellij's default for a tab nobody named (`Tab #<n>`).
pub fn is_default_name(name: &str) -> bool {
    name.strip_prefix("Tab #")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs() -> Vec<String> {
        ["◐", "◑", "✳"].map(String::from).to_vec()
    }

    #[test]
    fn working_glyph_mirrors_the_title_unless_steady() {
        let config = Config::default();
        assert_eq!(tab_glyph(Status::Working("◐"), &config), "◐");
        assert_eq!(tab_glyph(Status::Working("◑"), &config), "◑");
        assert_eq!(tab_glyph(Status::Idle, &config), "✳");

        let steady = Config {
            steady: true,
            working_glyph: "●".into(),
            ..Config::default()
        };
        assert_eq!(tab_glyph(Status::Working("◐"), &steady), "●");
        assert_eq!(tab_glyph(Status::Working("◑"), &steady), "●");
    }

    #[test]
    fn composes_names() {
        assert_eq!(tab_name("docs", Some("◐")), "◐ docs");
        assert_eq!(tab_name("docs", None), "docs");
    }

    #[test]
    fn strips_glyph_prefixes() {
        assert_eq!(strip_glyph("◐ docs", &glyphs()), "docs");
        assert_eq!(strip_glyph("✳ ◑ docs", &glyphs()), "docs");
        assert_eq!(strip_glyph("docs", &glyphs()), "docs");
        assert_eq!(strip_glyph("✳docs", &glyphs()), "✳docs");
        assert_eq!(strip_glyph("docs ✳ x", &glyphs()), "docs ✳ x");
    }

    #[test]
    fn strips_exit_markers() {
        assert_eq!(strip_exit_suffix("docs [ EXITED ] "), "docs");
        assert_eq!(strip_exit_suffix("docs [ EXIT CODE: 1 ] "), "docs");
        assert_eq!(strip_exit_suffix("docs [ EXIT CODE: -9 ] "), "docs");
        assert_eq!(
            strip_exit_suffix("docs [ EXIT CODE: x ] "),
            "docs [ EXIT CODE: x ] "
        );
        assert_eq!(strip_exit_suffix("docs"), "docs");
    }

    #[test]
    fn detects_default_names() {
        assert!(is_default_name("Tab #1"));
        assert!(is_default_name("Tab #12"));
        assert!(!is_default_name("Tab #"));
        assert!(!is_default_name("Tab #1a"));
        assert!(!is_default_name("docs"));
    }
}
