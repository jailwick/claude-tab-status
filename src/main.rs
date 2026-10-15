//! The zellij plugin: a thin shell around [`claude_tab_status::tracker::Tracker`].
//!
//! zellij 0.45.1 tells plugins nothing when a pane only changes its title, so `PaneUpdate`s carry
//! fresh titles only when something else changed (focus, layout, tabs). Fresher titles come from:
//! - a pipe message named [`PIPE_NAME`], sent by whoever changes the title (jailwick's relay):
//!   `zellij pipe --name claude-tab-status -- "$ZELLIJ_PANE_ID"`; without a pane id, every pane is
//!   re-read;
//! - optionally, a timer (`refresh_interval`) that re-reads all titles.
//!
//! While a hand-named tab works, its glyph follows the `◐`/`◑` spinner of the pane's title.
//! jailwick nudges only when the status changes, not on every frame, so a timer re-reads working
//! panes' titles twice a frame; it stops once no tab works.
//!
//! zellij runs one instance of a background plugin per connected client; only the instance of the
//! lowest client id renames tabs, so instances never fight over names. Nothing renders, and
//! renames happen only when a name has to change.

use std::collections::BTreeMap;

use claude_tab_status::config::Config;
use claude_tab_status::status::{pane_status, Status};
use claude_tab_status::ticker::Ticker;
use claude_tab_status::tracker::{Rename, TabView, Tracker};
use zellij_tile::prelude::*;

/// Name of the pipe messages that announce a title change.
const PIPE_NAME: &str = "claude-tab-status";

#[derive(Default)]
struct Plugin {
    tracker: Option<Tracker>,
    ticker: Ticker,
    client_id: ClientId,
    /// This instance renames tabs: no other connected client has a lower id.
    leader: bool,
    tabs: Vec<TabInfo>,
    panes: PaneManifest,
    /// The user is typing a tab name; renaming now would fight their input.
    renaming: bool,
}

register_plugin!(Plugin);

impl ZellijPlugin for Plugin {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        let config = Config::from_options(&configuration);
        // Goes to zellij's log: shows which build and options a session runs.
        eprintln!(
            "claude-tab-status {} loaded: {config:?}",
            env!("CARGO_PKG_VERSION")
        );
        self.ticker = Ticker::new(config.refresh_interval);
        self.tracker = Some(Tracker::new(config));
        self.client_id = get_plugin_ids().client_id;
        // zellij shows the prompt in a floating pane and hides it again once answered.
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            // Makes zellij leave the keybindings out of every `ModeUpdate`.
            EventType::InitialKeybinds,
            EventType::Timer,
            EventType::BeforeClose,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.leader = tabs
                    .iter()
                    .flat_map(|tab| &tab.other_focused_clients)
                    .all(|&other| other > self.client_id);
                self.tabs = tabs;
            }
            // Always followed by a `TabUpdate`; the manifest is keyed by tab position, so it is
            // matched to the tabs only once those are current too.
            Event::PaneUpdate(panes) => {
                self.panes = panes;
                return false;
            }
            Event::ModeUpdate(mode) => {
                let renaming = mode.mode == InputMode::RenameTab;
                if renaming == self.renaming {
                    return false;
                }
                self.renaming = renaming;
            }
            Event::Timer(_) => {
                if self.ticker.fire() {
                    self.refresh_titles(|_| true);
                } else if self.tracker.as_ref().is_some_and(Tracker::animating) {
                    self.refresh_titles(|pane| {
                        matches!(pane_status(&pane.title), Some(Status::Working(_)))
                    });
                }
            }
            Event::BeforeClose => {
                if let Some(tracker) = self.tracker.as_mut() {
                    let current_name = |tab_id| get_tab_info(tab_id).map(|tab| tab.name);
                    apply(tracker.restore(current_name));
                }
                return false;
            }
            _ => return false,
        }
        self.reconcile();
        false
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        if message.name == PIPE_NAME {
            let pane_id = message.payload.and_then(|p| p.trim().parse::<u32>().ok());
            self.refresh_titles(|pane| pane_id.is_none_or(|id| id == pane.id));
            self.reconcile();
        }
        false
    }
}

impl Plugin {
    /// Re-reads from zellij the titles of the terminal panes `wanted` picks.
    fn refresh_titles(&mut self, wanted: impl Fn(&PaneInfo) -> bool) {
        let panes = self.panes.panes.values_mut().flatten();
        for pane in panes.filter(|p| !p.is_plugin && wanted(p)) {
            if let Some(info) = get_pane_info(PaneId::Terminal(pane.id)) {
                pane.title = info.title;
            }
        }
    }

    fn reconcile(&mut self) {
        let Some(tracker) = self.tracker.as_mut() else {
            return;
        };
        if !self.leader {
            return;
        }
        if !self.renaming {
            let views: Vec<TabView> = self
                .tabs
                .iter()
                .map(|tab| TabView {
                    id: tab.tab_id,
                    name: &tab.name,
                    pane_titles: terminal_titles(
                        self.panes.panes.get(&tab.position).into_iter().flatten(),
                    )
                    .collect(),
                })
                .collect();
            apply(tracker.reconcile(&views));
        }
        self.arm_timer();
    }

    /// Sets the next timeout, if the animation or the refresh needs one and none is pending.
    fn arm_timer(&mut self) {
        let animating = self.tracker.as_ref().is_some_and(Tracker::animating);
        if let Some(delay) = self.ticker.arm(animating) {
            set_timeout(delay.as_secs_f64());
        }
    }
}

/// Titles of the terminal panes among `panes`, hidden ones included (a Claude pane hidden behind
/// the scrollback editor still runs).
fn terminal_titles<'a>(
    panes: impl IntoIterator<Item = &'a PaneInfo>,
) -> impl Iterator<Item = &'a str> {
    panes
        .into_iter()
        .filter(|pane| !pane.is_plugin)
        .map(|pane| pane.title.as_str())
}

fn apply(renames: Vec<Rename>) {
    for Rename { tab_id, name } in renames {
        rename_tab_with_id(tab_id as u64, &name);
    }
}
