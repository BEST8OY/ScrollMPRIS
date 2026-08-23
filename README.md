# ScrollMPRIS

[![Rust 2024](https://img.shields.io/badge/Rust-2024_Edition-orange.svg?logo=rust)](https://www.rust-lang.org/)
[![D-Bus](https://img.shields.io/badge/D--Bus-zbus_5-blue.svg?logo=linux)](https://crates.io/crates/zbus)
[![Async Runtime](https://img.shields.io/badge/Async-Tokio-black.svg?logo=tokio)](https://tokio.rs/)
[![AUR version](https://img.shields.io/aur/version/scrollmpris-git.svg)](https://aur.archlinux.org/packages/scrollmpris-git)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A fast, async, pure Rust scrolling MPRIS module for [Waybar](https://github.com/Alexays/Waybar) powered by `zbus 5` and `tokio`.

---

## ✨ Features

- **Pure Rust Async D-Bus (`zbus 5`)**: Zero C library dependencies (`libdbus-1` not required).
- **Dual-Tier Player Discovery**: Prefers `playerctld` for recency-ordered player prioritization, with automatic fallback to standard D-Bus discovery.
- **Real-Time Lifecycle Tracking**: Instant UI response to player startup, exit, and handoff via D-Bus `NameOwnerChanged` signals.
- **Multi-Artist & Album Support**: Formats collaborating/featured artists (`xesam:artist` joined with commas) and album metadata.
- **Rate-Adjusted Position Estimation**: Sub-second accurate elapsed & countdown timers dynamically adjusted by playback speed.
- **Pure Token-Driven Layout**: 100% template-controlled output. Place brand icons, state glyphs, timers, and metadata anywhere.
- **Field-Aware Inline Scrolling**: Scroll individual fields independently (e.g. `{title:20:marquee} | {artist:12:bounce}`).
- **Industry-Standard Scrolling Modes**: `marquee` (continuous ticker), `restart` (loop with hold), and `bounce` (pendulum back & forth).
- **TOML Configuration File**: Persistent declarative settings at `~/.config/ScrollMPRIS/config.toml` with automatic XDG resolution.
- **Freeze on Pause**: Option to freeze the ticker and snap text back to the start when music is paused.

---

## ⚡ Quick Start (Waybar)

### 1. Add to Waybar Config (`~/.config/waybar/config`)

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

### 2. Add to Waybar Module Bar

Include `"custom/ScrollMPRIS"` in your `modules-left`, `modules-center`, or `modules-right`.

---

## ⚙️ Configuration File (`config.toml`)

ScrollMPRIS automatically reads `~/.config/ScrollMPRIS/config.toml` (or `~/.config/scrollmpris/config.toml`).

### Generate Default Configuration

Generate a fully commented TOML configuration file with one command:

```bash
mkdir -p ~/.config/ScrollMPRIS
ScrollMPRIS --generate-config > ~/.config/ScrollMPRIS/config.toml
```

### Example `config.toml`

```toml
# General settings
speed = 50                              # 0 (1000ms delay) to 100 (100ms delay)
width = 40                              # Default max character width for scrolling text
scroll_mode = "marquee"                 # Default mode: "marquee", "restart", or "bounce"
format = "{player_icon} {status_icon} {title:20:marquee} | {artist:12:bounce} [{position}/{length}]"
format_stopped = ""                     # Output when stopped ("" to auto-hide from Waybar)
tooltip_format = "{player_icon} {status_icon} {title} - {artist} | {album}"
blocked = ["firefox", "chromium"]       # Ignore audio from specific players
freeze_on_pause = true                  # Pause ticker and reset to start when paused

# Status glyphs for playback states
[icons.status]
playing = ""
paused = ""
stopped = ""

# Player-specific brand icons (fallback icon is "404")
[icons.players]
spotify = ""
vlc = "󰕼"
firefox = "󰈹"
mpv = ""
chrome = ""
edge = "󰇩"
telegramdesktop = ""
tauon = ""
"404" = ""
```

> [!TIP]
> Any CLI argument passed directly in Waybar or terminal (such as `ScrollMPRIS --speed 80`) dynamically overrides `config.toml`.

---

## 🏷️ Supported Format Tokens

ScrollMPRIS is **100% token-driven**: you control exactly where each element appears.

| Category | Token | Aliases | Description | Example Output |
| :--- | :--- | :--- | :--- | :--- |
| **Icons** | `{player_icon}` | `{app_icon}` | Player brand glyph | `` (Spotify), `󰕼` (VLC), `󰈹` (Firefox) |
| | `{status_icon}` | `{play_icon}`, `{state_icon}` | Playback status glyph | `` (Playing), `` (Paused) |
| | `{icon}` | | Combined `{player_icon} {status_icon}` | ` ` |
| **Timers** | `{position}` | `{elapsed}` | Current playback elapsed time | `01:23` |
| | `{remaining}` | `{countdown}` | Remaining track countdown | `02:45` |
| | `{length}` | `{duration}` | Total track duration | `04:08` |
| **Metadata** | `{title}` | | Track title | `Blinding Lights` |
| | `{artist}` | | Track artist(s) (comma-joined) | `The Weeknd` |
| | `{album}` | | Track album name | `After Hours` |
| | `{player}` | | Clean player service name | `spotify` |
| | `{status}` | | Raw playback status | `Playing` / `Paused` |

---

## 🔄 Field-Aware Scrolling & Modifiers

Scroll individual metadata fields independently while keeping other elements static:

### 1. Inline Field Modifiers: `{field:width[:mode]}`
- `{title:20}`: Only `{title}` scrolls within 20 characters; `{artist}` stays fixed.
- `{title:20:bounce}`: `{title}` scrolls within 20 characters using `bounce` animation.
- `{title:15:marquee} | {artist:10:bounce}`: Title and artist scroll independently with separate widths and animation styles!

### 2. Block Scrolling: `[scroll:width[:mode]]...[/scroll]`
- `[scroll:25]{title} - {artist}[/scroll] | {album}`: Scrolls the combined title and artist block within 25 characters, leaving album fixed.

### 3. Whole-String Scrolling (Default Fallback)
- If no inline modifiers are present (e.g. `format = "{player_icon} {status_icon} {title} - {artist}"`), the entire formatted output scrolls up to `width` characters.

---

## 🎬 Scrolling Modes

| Mode | Description |
| :--- | :--- |
| **`marquee`** *(default)* | Seamless circular continuous ticker loop with separator padding. |
| **`restart`** | Smooth left-to-right scroll, holds at the end, and snaps back to start. |
| **`bounce`** | Scrolls to the end, holds, reverses direction back to start, holds, and repeats. |

---

## 🛠️ Command-Line Options

| Option | Description | Example |
| :--- | :--- | :--- |
| `-c`, `--config <path>` | Path to custom TOML configuration file | `-c ~/my-config.toml` |
| `--generate-config` | Output default configuration TOML to stdout and exit | `ScrollMPRIS --generate-config` |
| `-s`, `--speed <0-100>` | Scroll speed (0: slow=1000ms delay, 100: fast=100ms delay) | `-s 50` |
| `-w`, `--width <number>` | Maximum character width for scrolling text | `-w 40` |
| `-b`, `--blocked <list>` | Block specific players (comma-separated, case-insensitive) | `-b edge,firefox,mpv` |
| `--scroll <mode>` | Default scrolling behavior: `marquee`, `restart`, or `bounce` | `--scroll marquee` |
| `--format <string>` | Output format template when playing or paused | `--format '{player_icon} {title:20} - {artist}'` |
| `--format-stopped <string>` | Output format template when stopped (default: `""` to auto-hide) | `--format-stopped '{status_icon} No Media'` |
| `--tooltip-format <string>` | Tooltip format (resolves all fields un-scrolled on hover) | `--tooltip-format '{title} - {artist} \| {album}'` |
| `--icon-format <string>` | Override player brand icons via JSON mapping | `--icon-format '{"404": "", "spotify": ""}'` |
| `--freeze` | Pause scrolling and reset text to start when paused | `--freeze` |

---

## 💡 Configuration Recipes & Examples

### 1. Default Clean Ticker
```bash
ScrollMPRIS --format '{player_icon} {status_icon} {title:20} - {artist}'
```

### 2. Status Glyph at the End with Duration
```bash
ScrollMPRIS --format '{player_icon} {title:20:marquee} - {artist} [{position}/{length}] {status_icon}'
```

### 3. Countdown Remaining Time
```bash
ScrollMPRIS --format '{player_icon} {title:20:bounce} - {artist} (-{remaining})'
```

### 4. Minimalist (Status Glyph Only)
```bash
ScrollMPRIS --format '{status_icon} {title:25} - {artist}'
```

### 5. Plain Text (No Icons)
```bash
ScrollMPRIS --format '{title:20} - {artist} [{position}]'
```

### 6. Dual-Field Independent Scrolling
```bash
ScrollMPRIS -s 50 --format '{player_icon} {title:15:marquee} | {artist:10:bounce} {status_icon}'
```

### 7. Persistent Placeholder (No Auto-Hide on Stop)
Keep the Waybar module visible with custom placeholder text even when music stops:
```bash
ScrollMPRIS --format '{player_icon} {status_icon} {title:20} - {artist}' --format-stopped '{status_icon} No Media'
```

---

## 🎨 Waybar CSS Styling

ScrollMPRIS tags the Waybar JSON output with CSS classes matching playback status and player service name (e.g. `playing`, `paused`, `stopped`, `spotify`, `firefox`).

Customize your `~/.config/waybar/style.css`:

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

/* Player-specific styling */
#custom-ScrollMPRIS.spotify {
    color: #1db954;
}

#custom-ScrollMPRIS.firefox {
    color: #ff7139;
}

#custom-ScrollMPRIS.playing.spotify {
    border-bottom: 2px solid #1db954;
}
```

---

## 📦 Installation & Build

### Arch User Repository (AUR)

```bash
yay -S scrollmpris-git
```

### Manual Build from Source

```bash
git clone https://github.com/BEST8OY/ScrollMPRIS.git
cd ScrollMPRIS
cargo build --release
install -Dt /usr/local/bin target/release/ScrollMPRIS
```

---

## 🖼️ Preview

**Restart mode:**

![Restart mode](https://github.com/user-attachments/assets/5a151c83-394d-4f12-9660-6f248de1a71d)

**Marquee mode:**

![Marquee mode](https://github.com/user-attachments/assets/c72cc4be-3385-4a53-8848-7c292e12e400)

---

## 📄 License

This project is licensed under the [GNU General Public License v3.0 or later](LICENSE).
