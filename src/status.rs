//! Claude's status, as Claude Code (or jailwick's relay) writes it at the start of the pane title:
//! `◐ <topic>` / `◑ <topic>` while working, `✳ <topic>` when idle.

/// The frames of Claude's working spinner.
pub const WORKING_GLYPHS: [&str; 2] = ["◐", "◑"];
pub const IDLE_GLYPH: &str = "✳";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Claude is working; carries the spinner frame the pane's title shows right now.
    Working(&'static str),
    Idle,
}

/// The status a pane title carries: a known glyph followed by a space.
pub fn pane_status(title: &str) -> Option<Status> {
    let (glyph, _) = title.split_once(' ')?;
    if glyph == IDLE_GLYPH {
        return Some(Status::Idle);
    }
    WORKING_GLYPHS
        .into_iter()
        .find(|&g| g == glyph)
        .map(Status::Working)
}

/// A tab's status from its panes' titles: working if any pane works, else idle if any pane is
/// idle, else none.
pub fn tab_status<'a>(titles: impl IntoIterator<Item = &'a str>) -> Option<Status> {
    let mut status = None;
    for title in titles {
        match pane_status(title) {
            working @ Some(Status::Working(_)) => return working,
            Some(Status::Idle) => status = Some(Status::Idle),
            None => {}
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_status_from_title() {
        assert_eq!(pane_status("◐ fix tests"), Some(Status::Working("◐")));
        assert_eq!(pane_status("◑ fix tests"), Some(Status::Working("◑")));
        assert_eq!(pane_status("✳ fix tests"), Some(Status::Idle));
    }

    #[test]
    fn no_status_without_glyph_and_space() {
        assert_eq!(pane_status("✳fix"), None);
        assert_eq!(pane_status("✳"), None);
        assert_eq!(pane_status("bash"), None);
        assert_eq!(pane_status(""), None);
        assert_eq!(pane_status("vim ✳ notes"), None);
    }

    #[test]
    fn working_beats_idle() {
        assert_eq!(
            tab_status(["✳ a", "bash", "◑ b", "◐ c"]),
            Some(Status::Working("◑"))
        );
    }

    #[test]
    fn idle_when_no_pane_works() {
        assert_eq!(tab_status(["bash", "✳ a"]), Some(Status::Idle));
    }

    #[test]
    fn none_without_claude() {
        assert_eq!(tab_status(["bash", "vim"]), None);
        assert_eq!(tab_status([]), None);
    }
}
