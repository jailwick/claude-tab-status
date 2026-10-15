# claude-tab-status

A background [zellij](https://zellij.dev) plugin that shows Claude Code's status glyph in the
names of tabs you named by hand:

| Claude in the tab | Tab name  |
|-------------------|-----------|
| working           | `◐ docs` / `◑ docs`, in step with Claude's spinner |
| idle              | `✳ docs`  |
| not running       | `docs`    |

Claude Code puts its status at the start of the pane title (`◐`/`◑ <topic>` while working,
`✳ <topic>` when idle). zellij already shows that title in tabs you haven't named, but a tab you
renamed keeps its name. This plugin reads the pane titles and renames those tabs. No commands,
no network, nothing rendered.

Under zellij, plain `claude` keeps a static `✳` in its title; a live working status comes from
jailwick sessions (`jw claude`), whose relay writes it into the title
(`zellij.status_in_title`, on by default).

## How it learns about title changes

zellij 0.45.1 doesn't tell plugins when a pane only changes its title: the plugin sees new titles
when something else changes (switching tabs, focus, layout). So the plugin also listens for a
nudge, sent by whatever changes the title:

```sh
zellij pipe --name claude-tab-status -- "$ZELLIJ_PANE_ID"
```

It then re-reads that pane's title (every pane's, without an id) and renames if needed.

- **With jailwick** (from the version that ships it), nothing to set up: jw sends the nudge itself
  when the status changes (working ↔ not working) and at the end of a session, after taking its
  status off the title, so the tab gets its name back. Leave `refresh_interval` off.
- **Without it** (plain `claude`, older jailwick), set `refresh_interval`: a timer then re-reads the
  titles every few seconds.

## Animation

A working tab shows the same `◐`/`◑` frame as its pane's title, so it spins in step with the
window title. jailwick nudges only when the status changes, not on every frame, so while a tab you
named is working, the plugin re-reads working panes' titles every 480 ms (half of Claude's 960 ms
frame: it never skips or repeats a frame, and lags the title by half a frame at most) and renames
the tab when the frame changed. This timer runs only while some tab you named is working; it stops
as soon as none is. With `steady true`, a working tab shows `working_glyph` the whole time, and no
timer runs for it.

Requires zellij **0.45.1** or later.

## Build

```sh
cargo build --release
```

`.cargo/config.toml` builds for `wasm32-wasip1`, and `rust-toolchain.toml` installs that target on
first use. The plugin is `target/wasm32-wasip1/release/claude-tab-status.wasm`.

## Install

1. Copy the plugin:

   ```sh
   mkdir -p ~/.config/zellij/plugins
   cp target/wasm32-wasip1/release/claude-tab-status.wasm ~/.config/zellij/plugins/
   ```

2. Load it in the background from `~/.config/zellij/config.kdl`:

   ```kdl
   load_plugins {
       "file:~/.config/zellij/plugins/claude-tab-status.wasm" {
           // options (see below), e.g.
           // steady true
       }
   }
   ```

3. Start a new zellij session. On first start, zellij shows the plugin's permission prompt in a
   floating pane: answer `y`. It asks to read the session's state (tabs, panes) and to change it
   (rename tabs); zellij remembers the answer.

## kitty: window titles without the glyph

zellij sends the outer terminal `<session> | <pane title>`, so kitty's window and tab titles show
the spinner too (`jailwick | ◐ fix the tests`). To keep the status in the zellij tab names only,
use the kitty watcher in [`kitty/strip_status_glyph.py`](kitty/strip_status_glyph.py): it shows
`jailwick | fix the tests`, where the topic is Claude's own summary of the conversation.

```sh
cp kitty/strip_status_glyph.py ~/.config/kitty/
echo 'watcher strip_status_glyph.py' >> ~/.config/kitty/kitty.conf
```

Then restart kitty (a config reload only affects windows opened after it). The watcher only touches
titles that start with a glyph (after the session name), and never clears a title you set in kitty.

## Options

| Option          | Default | Meaning |
|-----------------|---------|---------|
| `steady`        | `false` | `false`: a working tab spins with its pane title's `◐`/`◑`. `true`: it shows `working_glyph` the whole time, and is renamed only when the state changes. |
| `working_glyph` | `◐`     | Glyph for a working tab when `steady` is `true`. |
| `idle_glyph`    | `✳`     | Glyph for an idle tab. |
| `refresh_interval` | off  | Seconds between re-reads of all pane titles, e.g. `1` (quote fractions: `"0.5"`; zellij drops unquoted decimals). A fallback when nothing sends the nudge; it keeps waking the plugin, so leave it off when jailwick sends the nudge. While a tab works, the re-reads ride on the animation's timer, at their own pace. |

## Behaviour

- A tab's status comes from its terminal panes: working if any pane's title starts with `◐ ` or
  `◑ `, else idle if any starts with `✳ `, else none.
- The tab is renamed to `<glyph> <name>`, and back to `<name>` when no Claude runs in it. It is
  renamed only when the name has to change.
- Renaming a tab that shows a glyph makes the new name (without a glyph you kept) its name.
- While you are typing a tab name, the plugin waits.
- Tabs you haven't named are left alone, so they keep showing the pane title. zellij reports
  those as `Tab #<n>` or as their pane's title. A tab you name exactly like that is treated the
  same way.
- When the plugin unloads, tabs get their names back (unless renamed since).
- zellij runs a background plugin once per attached client. Only the instance of the client with
  the lowest id renames tabs, so several clients don't fight over names.

## Limitations

- zellij gives plugins the pane title as shown in the UI: if you named a pane yourself, that
  name replaces the title Claude sets, and the pane carries no status.
- The glyph is part of the tab's name while it shows: `zellij action go-to-tab-name docs` won't
  match `✳ docs`, and a session saved meanwhile keeps the glyph (the plugin clears it on the next
  update after the session is resurrected).
- In a mirrored session, clients don't see each other's focus, so every instance renames; the
  renames are identical, only repeated.

## Development

```sh
cargo test                                       # in wasmtime (the runner in .cargo/config.toml)
cargo test --target "$(rustc -vV | sed -n 's/^host: //p')"   # or natively, without wasmtime
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The naming logic lives in a library with no zellij calls (`src/lib.rs`), tested on its own; the
plugin binary is the thin zellij layer:

| Module       | Role |
|--------------|------|
| `status.rs`  | Status of a pane title and of a tab |
| `naming.rs`  | Glyph prefixes, zellij's default names and exit markers, composing names |
| `config.rs`  | Plugin options |
| `tracker.rs` | Which tabs to rename, and to what; whether a tab is working |
| `ticker.rs`  | When the one timer fires: spinner samples, title re-reads |
| `main.rs`    | The zellij plugin: events, pipe nudges and the timer in, `rename_tab_with_id` out |
