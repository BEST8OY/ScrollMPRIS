# ScrollMPRIS

[![Rust 2024](https://img.shields.io/badge/Rust-2024_Edition-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![D-Bus](https://img.shields.io/badge/D--Bus-zbus_6-blue.svg?logo=linux)](https://crates.io/crates/zbus)
[![Async Runtime](https://img.shields.io/badge/Async-Tokio-black.svg?logo=tokio)](https://tokio.rs/)
[![AUR version](https://img.shields.io/aur/version/scrollmpris-git.svg)](https://aur.archlinux.org/packages/scrollmpris-git)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A fast, event-driven MPRIS media player module for [Waybar](https://github.com/Alexays/Waybar) written in pure Rust using [`zbus 6`](https://crates.io/crates/zbus) and [`tokio`](https://tokio.rs/).

---

## Preview

**Restart mode:**

![Restart mode](https://github.com/user-attachments/assets/5a151c83-394d-4f12-9660-6f248de1a71d)

**Marquee mode:**

![Marquee mode](https://github.com/user-attachments/assets/c72cc4be-3385-4a53-8848-7c292e12e400)

---

## Features

- **Pure Rust D-Bus (`zbus 6`):** No `libdbus-1` or C library dependencies.
- **Signal-driven discovery & lifecycle:** Reacts immediately to player launches, exits, track changes, seeks, and playback handoffs via broker-filtered `NameOwnerChanged` / `PropertiesChanged` match rules, with optional `playerctld` integration and a concurrent priority-ranked fallback (`Playing` > `Paused` > `Stopped`).
- **Smart position tracking:** Estimates elapsed/remaining time locally scaled by playback rate with zero steady-state D-Bus polling. For streaming players (`xesam:url` not `file://`), transient post-buffering calibration holds the timer until audio progression is confirmed; local `file://` tracks start immediately, and formats without timer tokens skip `Position` queries altogether.
- **Field-aware scrolling:** Scroll the entire line, individual tokens (`{title:20:marquee}`), or grouped `[scroll]...[/scroll]` blocks using `marquee`, `restart`, or `bounce` animations.
- **Configurable via TOML & CLI:** Declarative config at `~/.config/ScrollMPRIS/config.toml` with CLI flag overrides, player blocklists, custom player/status icons, freeze-on-pause, and primary-artist extraction.

---

## Installation

### Arch User Repository (AUR)

```bash
yay -S scrollmpris-git
```

### Build from Source

```bash
git clone https://github.com/BEST8OY/ScrollMPRIS.git
cd ScrollMPRIS
cargo build --release
install -Dm755 target/release/ScrollMPRIS /usr/local/bin/ScrollMPRIS
ln -sf /usr/local/bin/ScrollMPRIS /usr/local/bin/scrollmpris
```

---

## Waybar Quick Start

Add the custom module to your Waybar configuration (`~/.config/waybar/config` or `config.jsonc`):

```json
"custom/ScrollMPRIS": {
    "return-type": "json",
    "exec": "ScrollMPRIS",
    "escape": true,
    "on-click": "playerctl play-pause",
    "on-scroll-up": "playerctl next",
    "on-scroll-down": "playerctl previous"
}
```

Then include `"custom/ScrollMPRIS"` in `modules-left`, `modules-center`, or `modules-right`.

---

## Configuration

ScrollMPRIS checks `$XDG_CONFIG_HOME` then `~/.config` for `ScrollMPRIS/config.toml` (or `scrollmpris/config.toml`). CLI arguments always override config file values.

Generate a fully documented default configuration:

```bash
mkdir -p ~/.config/ScrollMPRIS
ScrollMPRIS --generate-config > ~/.config/ScrollMPRIS/config.toml
```

### Example `config.toml`

```toml
speed = 50                              # 0 (1000ms delay) to 100 (100ms delay)
width = 40                              # Default max character width for scrolling
scroll_mode = "marquee"                 # "marquee", "restart", or "bounce"
format = "{player_icon} {status_icon} {title:20:marquee} | {artist:12:bounce} [{position}/{length}]"
format_stopped = ""                     # "" auto-hides the module when stopped
tooltip_format = "{player_icon} {status_icon} {title} - {artist} | {album}"
blocked = ["firefox", "chromium"]       # Case-insensitive substrings to ignore
freeze_on_pause = true                  # Pause scrolling and reset offset when paused
first_artist = false                    # Show only the primary artist for {artist}

[icons.status]
playing = ""
paused = ""
stopped = ""

[icons.players]
spotify = ""
vlc = "󰕼"
edge = "󰇩"
firefox = "󰈹"
mpv = ""
chrome = ""
telegramdesktop = ""
tauon = ""
"404" = ""
```

---

## Format Tokens & Scrolling

### Available Tokens

| Category | Token | Aliases | Description | Example |
| :--- | :--- | :--- | :--- | :--- |
| **Icons** | `{player_icon}` | `{app_icon}` | Player brand glyph (falls back to `"404"`) | `` |
| | `{status_icon}` | `{play_icon}`, `{state_icon}` | Playback status glyph | `` |
| | `{icon}` | | Combined `{player_icon} {status_icon}` | ` ` |
| **Timers** | `{position}` | `{elapsed}` | Current elapsed time (`mm:ss` / `hh:mm:ss`) | `01:23` |
| | `{remaining}` | `{countdown}` | Remaining time | `02:45` |
| | `{length}` | `{duration}` | Total track duration from metadata | `04:08` |
| **Metadata** | `{title}` | | Track title | `Blinding Lights` |
| | `{artist}` | | Track artist(s) (or primary artist if `first_artist = true`) | `The Weeknd` |
| | `{first_artist}` | `{primary_artist}` | First/primary artist only | `The Weeknd` |
| | `{album}` | | Album name | `After Hours` |
| | `{player}` | | Clean MPRIS service name | `spotify` |
| | `{status}` | | Raw MPRIS `PlaybackStatus` | `Playing` |

### Scrolling Directives & Modes

- **Whole-line fallback:** Without inline modifiers, the entire rendered `format` scrolls when longer than `width`.
- **Inline field modifiers:** `{field:width}`, `{field:mode}`, or `{field:width:mode}` (e.g. `{title:20:marquee} | {artist:12:bounce}`).
- **Block modifiers:** `[scroll]...[/scroll]`, `[scroll:width]...[/scroll]`, or `[scroll:width:mode]...[/scroll]` (e.g. `[scroll:25]{title} - {artist}[/scroll] | {album}`).
- **Modes:**
  - `marquee` *(default)* — continuous circular ticker with spacer padding
  - `restart` — scrolls to the end, pauses briefly, and jumps back to the start
  - `bounce` — scrolls back and forth between the start and end

Token-specific widths override the global `width` setting for that block, and strings shorter than their configured width stay static without extra padding.

---

## CLI Options

| Flag | Description |
| :--- | :--- |
| `-c`, `--config <path>` | Path to a custom TOML config file |
| `--generate-config` | Print default `config.toml` to stdout and exit |
| `-s`, `--speed <0-100>` | Scroll speed (`0` = 1000 ms delay, `100` = 100 ms delay) |
| `-w`, `--width <number>` | Default maximum character width for scrolling |
| `-b`, `--blocked <list>` | Comma-separated player names to ignore |
| `--scroll <mode>` | Default scroll mode (`marquee`, `restart`, `bounce`) |
| `--format <string>` | Output format template when playing or paused |
| `--format-stopped <string>` | Output format template when stopped (`""` hides module) |
| `--tooltip-format <string>` | Unscrolled tooltip format template |
| `--icon-format <json>` | Override player brand icons with a JSON object |
| `--freeze` | Freeze scrolling and reset position when paused |
| `--first-artist` | Show only the primary artist for multi-artist tracks (`--only-first-artist` alias) |

---

## Waybar CSS Styling

ScrollMPRIS attaches status (`playing`, `paused`, `stopped`) and lowercase player name (e.g. `spotify`, `firefox`, `mpv`) CSS classes to every JSON message:

```css
#custom-ScrollMPRIS {
    padding: 0 10px;
    color: #cdd6f4;
    background: #1e1e2e;
    border-radius: 8px;
}

#custom-ScrollMPRIS.playing {
    color: #a6e3a1;
}

#custom-ScrollMPRIS.paused {
    color: #f9e2af;
}

#custom-ScrollMPRIS.stopped {
    color: #6c7086;
}

#custom-ScrollMPRIS.playing.spotify {
    border-bottom: 2px solid #1db954;
}
```

---

## License

Licensed under the [GNU General Public License v3.0 or later](LICENSE).
