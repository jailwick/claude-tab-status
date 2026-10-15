//! Decides which tabs to rename, from the latest tabs and pane titles zellij reported.

use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::naming::{is_default_name, strip_exit_suffix, strip_glyph, tab_glyph, tab_name};
use crate::status::{tab_status, Status, IDLE_GLYPH, WORKING_GLYPHS};

/// A tab as the tracker needs it.
#[derive(Clone, Debug)]
pub struct TabView<'a> {
    pub id: usize,
    /// The name zellij reports for the tab.
    pub name: &'a str,
    /// Titles of the tab's terminal (non-plugin) panes.
    pub pane_titles: Vec<&'a str>,
}

/// A rename the plugin should apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub tab_id: usize,
    pub name: String,
}

/// A tab whose name this plugin manages.
#[derive(Debug)]
struct Tracked {
    /// The user's name for the tab, without a glyph.
    base: String,
    /// The name this plugin last gave the tab.
    applied: String,
    /// Claude works in the tab, so its glyph follows the title's spinner.
    working: bool,
}

#[derive(Debug)]
pub struct Tracker {
    config: Config,
    /// Every glyph that may prefix a tab name, stripped to recover the base.
    glyphs: Vec<String>,
    tabs: HashMap<usize, Tracked>,
}

impl Tracker {
    pub fn new(config: Config) -> Self {
        let mut glyphs: Vec<String> = WORKING_GLYPHS
            .into_iter()
            .chain([IDLE_GLYPH])
            .map(str::to_owned)
            .collect();
        for glyph in [&config.working_glyph, &config.idle_glyph] {
            if !glyphs.contains(glyph) {
                glyphs.push(glyph.clone());
            }
        }
        Self {
            config,
            glyphs,
            tabs: HashMap::new(),
        }
    }

    /// Whether a hand-named tab is working with its glyph following the title's spinner, so the
    /// plugin should keep re-reading working panes' titles (zellij doesn't report each frame).
    /// False while no such tab works, and with `steady`: nothing needs to run then.
    pub fn animating(&self) -> bool {
        !self.config.steady && self.tabs.values().any(|t| t.working)
    }

    /// The renames that bring every hand-named tab in line with its panes' status. Returns
    /// nothing when names already match, so the `TabUpdate` a rename causes ends here.
    pub fn reconcile(&mut self, tabs: &[TabView]) -> Vec<Rename> {
        let present: HashSet<usize> = tabs.iter().map(|tab| tab.id).collect();
        self.tabs.retain(|id, _| present.contains(id));
        tabs.iter()
            .filter_map(|tab| self.reconcile_tab(tab))
            .collect()
    }

    fn reconcile_tab(&mut self, tab: &TabView) -> Option<Rename> {
        let current = strip_exit_suffix(tab.name);
        let tracked = self.tabs.get(&tab.id);

        let base = match tracked {
            Some(t) if t.applied == current => t.base.clone(),
            // A new tab, or the user renamed it.
            _ => match self.user_name(current, &tab.pane_titles) {
                Some(base) => base.to_owned(),
                None => {
                    self.tabs.remove(&tab.id);
                    return None;
                }
            },
        };

        let status = tab_status(tab.pane_titles.iter().copied());
        let glyph = status.map(|status| tab_glyph(status, &self.config));
        let wanted = tab_name(&base, glyph);
        // A rename already sent but not yet reflected in this update.
        let in_flight = tracked.is_some_and(|t| t.applied == wanted);
        let rename = (wanted != current && !in_flight).then(|| Rename {
            tab_id: tab.id,
            name: wanted.clone(),
        });

        if glyph.is_none() && current == base {
            self.tabs.remove(&tab.id);
        } else {
            self.tabs.insert(
                tab.id,
                Tracked {
                    base,
                    applied: wanted,
                    working: matches!(status, Some(Status::Working(_))),
                },
            );
        }
        rename
    }

    /// The base name a user gave a tab named `name`, or `None` for a tab nobody named: zellij
    /// shows those with its default name or their pane's title, and they should keep following
    /// the title. An empty name (the user cleared it) counts as unnamed too.
    fn user_name<'a>(&self, name: &'a str, pane_titles: &[&str]) -> Option<&'a str> {
        if is_default_name(name) || pane_titles.contains(&name) {
            return None;
        }
        Some(strip_glyph(name, &self.glyphs))
            .filter(|base| !base.trim_matches(['\0', ' ']).is_empty())
    }

    /// Renames that put tabs back to their base names, e.g. when the plugin unloads.
    /// `current_name` gives a tab's name right now (or `None` if it is gone); a tab is restored
    /// only while it still shows the name this tracker gave it, so a stale plugin instance never
    /// undoes a newer rename.
    pub fn restore(&mut self, current_name: impl Fn(usize) -> Option<String>) -> Vec<Rename> {
        self.tabs
            .drain()
            .filter(|(tab_id, t)| {
                t.applied != t.base
                    && current_name(*tab_id)
                        .is_some_and(|name| strip_exit_suffix(&name) == t.applied)
            })
            .map(|(tab_id, t)| Rename {
                tab_id,
                name: t.base,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab<'a>(id: usize, name: &'a str, pane_titles: &[&'a str]) -> TabView<'a> {
        TabView {
            id,
            name,
            pane_titles: pane_titles.to_vec(),
        }
    }

    fn rename(tab_id: usize, name: &str) -> Rename {
        Rename {
            tab_id,
            name: name.to_owned(),
        }
    }

    /// A `current_name` lookup for `restore` where every tab is named `name`.
    fn named(name: &str) -> impl Fn(usize) -> Option<String> + '_ {
        move |_| Some(name.to_owned())
    }

    /// Reconciles one tab, as zellij would report it after applying earlier renames.
    fn step(tracker: &mut Tracker, name: &str, titles: &[&str]) -> Vec<Rename> {
        tracker.reconcile(&[tab(0, name, titles)])
    }

    #[test]
    fn named_tab_follows_claude_through_a_session() {
        let mut t = Tracker::new(Config::default());
        assert_eq!(
            step(&mut t, "docs", &["◐ fix", "bash"]),
            [rename(0, "◐ docs")]
        );
        assert_eq!(
            step(&mut t, "◐ docs", &["◑ fix", "bash"]),
            [rename(0, "◑ docs")]
        );
        assert_eq!(
            step(&mut t, "◑ docs", &["✳ fix", "bash"]),
            [rename(0, "✳ docs")]
        );
        assert_eq!(
            step(&mut t, "✳ docs", &["bash", "bash"]),
            [rename(0, "docs")]
        );
        assert_eq!(step(&mut t, "docs", &["bash", "bash"]), []);
        assert!(t.tabs.is_empty());
    }

    #[test]
    fn no_rename_when_name_already_matches() {
        let mut t = Tracker::new(Config::default());
        assert_eq!(
            step(&mut t, "docs", &["✳ fix", "bash"]),
            [rename(0, "✳ docs")]
        );
        assert_eq!(step(&mut t, "✳ docs", &["✳ fix", "bash"]), []);
        assert_eq!(step(&mut t, "✳ docs", &["✳ other topic", "bash"]), []);
    }

    #[test]
    fn no_duplicate_while_rename_in_flight() {
        let mut t = Tracker::new(Config::default());
        assert_eq!(
            step(&mut t, "docs", &["◐ fix", "bash"]),
            [rename(0, "◐ docs")]
        );
        // A PaneUpdate arrives before the TabUpdate for our rename.
        assert_eq!(step(&mut t, "docs", &["◐ fix", "bash"]), []);
        assert_eq!(step(&mut t, "◐ docs", &["◐ fix", "bash"]), []);
    }

    #[test]
    fn no_duplicate_while_restoring_rename_in_flight() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["✳ fix", "bash"]);
        assert_eq!(
            step(&mut t, "✳ docs", &["bash", "bash"]),
            [rename(0, "docs")]
        );
        assert_eq!(step(&mut t, "✳ docs", &["bash", "bash"]), []);
        assert_eq!(step(&mut t, "docs", &["bash", "bash"]), []);
    }

    #[test]
    fn steady_renames_only_on_state_change() {
        let mut t = Tracker::new(Config {
            steady: true,
            ..Config::default()
        });
        assert_eq!(
            step(&mut t, "docs", &["◐ fix", "bash"]),
            [rename(0, "◐ docs")]
        );
        assert_eq!(step(&mut t, "◐ docs", &["◑ fix", "bash"]), []);
        assert_eq!(step(&mut t, "◐ docs", &["◐ fix", "bash"]), []);
        assert_eq!(
            step(&mut t, "◐ docs", &["✳ fix", "bash"]),
            [rename(0, "✳ docs")]
        );
    }

    #[test]
    fn custom_glyphs() {
        let mut t = Tracker::new(Config {
            steady: true,
            working_glyph: "●".into(),
            idle_glyph: "○".into(),
            ..Config::default()
        });
        assert_eq!(
            step(&mut t, "docs", &["◐ fix", "bash"]),
            [rename(0, "● docs")]
        );
        assert_eq!(
            step(&mut t, "● docs", &["✳ fix", "bash"]),
            [rename(0, "○ docs")]
        );
        // A user rename that keeps a custom glyph still yields a clean base.
        assert_eq!(step(&mut t, "○ notes", &["✳ fix", "bash"]), []);
        assert_eq!(t.restore(named("○ notes")), [rename(0, "notes")]);
    }

    #[test]
    fn user_rename_becomes_base() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["◐ fix", "bash"]);
        assert_eq!(
            step(&mut t, "notes", &["◐ fix", "bash"]),
            [rename(0, "◐ notes")]
        );
        assert_eq!(
            step(&mut t, "◐ notes", &["bash", "bash"]),
            [rename(0, "notes")]
        );
    }

    #[test]
    fn user_rename_keeping_glyph_becomes_base() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["◐ fix", "bash"]);
        assert_eq!(step(&mut t, "◐ notes", &["◐ fix", "bash"]), []);
        assert_eq!(
            step(&mut t, "◐ notes", &["bash", "bash"]),
            [rename(0, "notes")]
        );
    }

    #[test]
    fn named_tab_already_carrying_glyph_is_adopted() {
        // e.g. after the plugin reloads while a tab still shows a glyph.
        let mut t = Tracker::new(Config::default());
        assert_eq!(
            step(&mut t, "◐ docs", &["✳ fix", "bash"]),
            [rename(0, "✳ docs")]
        );
        assert_eq!(t.restore(named("✳ docs")), [rename(0, "docs")]);
    }

    #[test]
    fn leaves_unnamed_tabs_alone() {
        let mut t = Tracker::new(Config::default());
        // Several panes: zellij reports its default name.
        assert_eq!(step(&mut t, "Tab #3", &["◐ fix", "bash"]), []);
        // A single pane: zellij reports the pane's title as the tab name.
        assert_eq!(step(&mut t, "✳ fix", &["✳ fix"]), []);
        assert_eq!(step(&mut t, "bash", &["bash"]), []);
        assert!(t.tabs.is_empty());
    }

    #[test]
    fn exit_marker_never_reaches_a_name() {
        let mut t = Tracker::new(Config::default());
        assert_eq!(
            step(&mut t, "docs [ EXITED ] ", &["✳ fix"]),
            [rename(0, "✳ docs")]
        );
        assert_eq!(step(&mut t, "✳ docs [ EXIT CODE: 0 ] ", &["✳ fix"]), []);
    }

    #[test]
    fn tabs_are_independent_and_closed_tabs_forgotten() {
        let mut t = Tracker::new(Config::default());
        let renames = t.reconcile(&[
            tab(1, "docs", &["◐ a"]),
            tab(2, "web", &["✳ b", "bash"]),
            tab(3, "logs", &["tail"]),
        ]);
        assert_eq!(renames, [rename(1, "◐ docs"), rename(2, "✳ web")]);
        assert_eq!(t.reconcile(&[tab(2, "✳ web", &["✳ b", "bash"])]), []);
        assert_eq!(t.restore(named("✳ web")), [rename(2, "web")]);
    }

    #[test]
    fn restore_skips_tabs_already_at_base() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["✳ fix", "bash"]);
        step(&mut t, "✳ docs", &["bash", "bash"]);
        assert_eq!(t.restore(named("docs")), []);
    }

    #[test]
    fn restore_skips_tabs_renamed_or_closed_since() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["✳ fix", "bash"]);
        assert_eq!(t.restore(named("notes")), []);

        step(&mut t, "docs", &["✳ fix", "bash"]);
        assert_eq!(t.restore(|_| None), []);
    }

    #[test]
    fn cleared_name_is_left_alone() {
        let mut t = Tracker::new(Config::default());
        assert_eq!(step(&mut t, "", &["◐ fix", "bash"]), []);
        assert_eq!(step(&mut t, "\0", &["◐ fix", "bash"]), []);
        assert!(t.tabs.is_empty());
    }

    #[test]
    fn working_tab_mirrors_the_title_frame() {
        let mut t = Tracker::new(Config::default());
        assert!(!t.animating());
        assert_eq!(step(&mut t, "docs", &["◑ fix"]), [rename(0, "◑ docs")]);
        assert!(t.animating());
        assert_eq!(step(&mut t, "◑ docs", &["◐ fix"]), [rename(0, "◐ docs")]);
        assert_eq!(
            step(&mut t, "◐ docs", &["◐ fix"]),
            [],
            "same frame: no rename"
        );
    }

    #[test]
    fn animation_stops_when_no_tab_works() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["◐ fix"]);
        assert!(t.animating());
        assert_eq!(step(&mut t, "◐ docs", &["✳ fix"]), [rename(0, "✳ docs")]);
        assert!(!t.animating());
        assert_eq!(step(&mut t, "✳ docs", &["bash"]), [rename(0, "docs")]);
        assert!(!t.animating());
    }

    #[test]
    fn each_tab_follows_its_own_pane() {
        let mut t = Tracker::new(Config::default());
        let renames = t.reconcile(&[
            tab(1, "docs", &["◐ a"]),
            tab(2, "web", &["◑ b", "bash"]),
            tab(3, "notes", &["✳ c", "bash"]),
        ]);
        assert_eq!(
            renames,
            [
                rename(1, "◐ docs"),
                rename(2, "◑ web"),
                rename(3, "✳ notes")
            ]
        );
    }

    #[test]
    fn idle_or_unnamed_tabs_and_steady_dont_animate() {
        let mut t = Tracker::new(Config::default());
        step(&mut t, "docs", &["✳ fix", "bash"]);
        assert!(!t.animating());
        // A tab nobody named follows the pane title; zellij shows jw's own spinner there.
        assert_eq!(t.reconcile(&[tab(4, "Tab #4", &["◐ fix", "bash"])]), []);
        assert!(!t.animating());

        let mut steady = Tracker::new(Config {
            steady: true,
            ..Config::default()
        });
        step(&mut steady, "docs", &["◐ fix", "bash"]);
        assert!(!steady.animating());
    }
}
