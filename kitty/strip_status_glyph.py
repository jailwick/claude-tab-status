"""kitty watcher: shows window titles without Claude's status glyph.

Under zellij, kitty's title is "<session> | <pane title>", and a pane running Claude has the title
"<glyph> <topic>" (◐/◑ while it works, ✳ otherwise). This keeps "<session> | <topic>": the status
stays in the zellij tab names (claude-tab-status), not in kitty's window and tab titles.

Install: copy to ~/.config/kitty/ and add to kitty.conf:
    watcher strip_status_glyph.py
then restart kitty (a config reload only affects new windows).
"""

import re
from typing import Any

# A status glyph and its space, at the start of the title or of the pane title after zellij's
# "<session> | ".
GLYPH = re.compile(r"^([^|]* \| )?[◐◑✳] ")

# Windows whose title this watcher overrides, so it never clears a title the user set.
overridden: set[int] = set()


def strip_glyph(title: str) -> str:
    return GLYPH.sub(r"\1", title, count=1)


def on_title_change(boss: Any, window: Any, data: dict[str, Any]) -> None:
    # Titles set by this watcher come back with from_child False: leave those alone.
    if not data.get("from_child"):
        return
    title = data["title"]
    clean = strip_glyph(title)
    if clean != title:
        window.set_title(clean)
        overridden.add(window.id)
    elif window.id in overridden:
        # No glyph any more: back to kitty showing the program's title as usual.
        window.set_title(None)
        overridden.discard(window.id)
