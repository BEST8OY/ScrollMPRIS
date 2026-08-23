# ScrollMPRIS

A fast, async, scrolling MPRIS module for [Waybar](https://github.com/Alexays/Waybar) written in pure Rust using `zbus 5` and `tokio`.

> **Note:** This project was generated and improved with the help of AI.

---

## Features

- **Pure Rust Async D-Bus (`zbus 5`)**: Zero C library dependencies (`libdbus-1` not required).
- **Dual-Tier Player Discovery**: Prefers `playerctld` for recency-ordered player discovery, with automatic fallback to standard D-Bus daemon player discovery.
- **Real-Time Lifecycle Tracking**: Instant UI reaction to player launch, exit, and handoff via D-Bus `NameOwnerChanged` signals.
- **Multi-Artist & Album Support**: Preserves and formats all collaborating/featured artists and album fields (`xesam:artist` joined with commas).
- **Rate-Adjusted Position Estimation**: Dynamic position tracking accurately scaled by player playback rate.
- **Pure Token-Driven Layout**: Place player icons, status glyphs, and timers anywhere within your format template.
- **Configurable Scrolling**: Industry-standard scrolling modes (`marquee`, `restart`, `bounce`) with configurable speed, width, and pauses.
- **Field-Aware Scrolling**: Scroll individual fields independently (e.g. `{title:20} - {artist}`) with inline width and mode controls.
- **TOML Configuration File**: Persistent settings at `~/.config/ScrollMPRIS/config.toml` with automatic XDG resolution.

---

## Prerequisites

- **D-Bus Session Bus**: Standard desktop session bus for MPRIS communication.
- **playerctl / playerctld (Optional)**: If `playerctld` is running, ScrollMPRIS uses it to prioritize the most recently active player. If absent, standard MPRIS discovery is used seamlessly.
- **Cargo / Rust**: Build toolchain (Rust 2024 edition).

---

## Installation & Build

### Arch User Repository (AUR)

You can install ScrollMPRIS using an AUR helper such as `yay` or `paru`:

```bash
yay -S scrollmpris-git
```

### Manual Build

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/BEST8OY/ScrollMPRIS.git
   cd ScrollMPRIS
   ```

2. **Build the Project:**

   ```bash
   cargo build --release
   ```

3. **Install the Binary:**

   ```bash
   install -Dt /usr/local/bin target/release/ScrollMPRIS
   ```

---

## Configuration File (`config.toml`)

ScrollMPRIS supports persistent configuration via TOML. Instead of passing long command-line arguments, define your settings in `~/.config/ScrollMPRIS/config.toml` (or `~/.config/scrollmpris/config.toml`).

### Generate Default Configuration

Generate a fully commented default configuration file with a single command:

```bash
mkdir -p ~/.config/ScrollMPRIS
ScrollMPRIS --generate-config > ~/.config/ScrollMPRIS/config.toml
```

### Example `config.toml`

```toml
# General settings
speed = 50                              # 0 (1000ms delay) to 100 (100ms delay)
width = 40                              # Default max width for scrolling text
scroll_mode = "marquee"                 # "marquee", "restart", or "bounce"
format = "{player_icon} {status_icon} {title:20:marquee} | {artist:12:bounce} [{position}/{length}]"
tooltip_format = "{player_icon} {status_icon} {title} - {artist} | {album}"
blocked = ["firefox", "chromium"]       # Blocked player names
freeze_on_pause = true                  # Pause ticker when playback is paused

# Icon settings
[icons]

# Status glyphs for playback states
[icons.status]
playing = ""
paused = ""
stopped = ""

# Player-specific icons (replaces 404 for unmatched players)
[icons.players]
spotify = ""
vlc = "󰕼"
firefox = "󰈹"
mpv = ""
"404" = ""
```

---

## Waybar Integration

With a configuration file in place, your Waybar configuration (`~/.config/waybar/config`) stays clean and simple:

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

> [!TIP]
> Any CLI argument passed directly in Waybar or terminal (such as `ScrollMPRIS --speed 80`) will dynamically override the settings in `config.toml`.

### Styling with CSS

Customize the appearance in your Waybar stylesheet (`~/.config/waybar/style.css`):

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

## Supported Format Tokens

ScrollMPRIS is **100% token-driven**: you control exactly where icons, metadata, and timers appear in the output.

| Category | Token | Aliases | Description | Example Output |
| :--- | :--- | :--- | :--- | :--- |
| **Icons** | `{player_icon}` | `{app_icon}` | Player brand icon | `` (Spotify), `󰕼` (VLC), `󰈹` (Firefox) |
| | `{status_icon}` | `{play_icon}`, `{state_icon}` | Play/pause status glyph | `` (Playing), `` (Paused) |
| | `{icon}` | | Combined player and status icon | ` ` |
| **Timers** | `{position}` | `{elapsed}` | Current playback position | `01:23` |
| | `{remaining}` | `{countdown}` | Remaining countdown time | `02:45` |
| | `{length}` | `{duration}` | Total track duration | `04:08` |
| **Metadata** | `{title}` | | Track title | `Blinding Lights` |
| | `{artist}` | | Track artist(s) | `The Weeknd` |
| | `{album}` | | Album name | `After Hours` |
| | `{player}` | | Player process name | `spotify` |
| | `{status}` | | Raw playback status text | `Playing` / `Paused` |

---

## Scrolling Modes

ScrollMPRIS supports three industry-standard scrolling behaviors:

| Mode | Description |
| :--- | :--- |
| **`marquee`** *(default)* | Continuous seamless circular ticker loop with separator spacing. |
| **`restart`** | Scrolls from left to right, holds at the end, and restarts back at the beginning. |
| **`bounce`** | Scrolls to the end, holds, reverses direction back to start, holds, and repeats. |

---

## Field-Aware Scrolling

Scroll individual metadata fields independently while keeping other text static:

1. **Inline Field Width & Mode**:
   - `{title:20}`: Only `{title}` scrolls within 20 characters; `{artist}` remains static.
   - `{title:20:bounce}`: Only `{title}` scrolls at width 20 using `bounce` mode.
   - `{title:15:marquee} | {artist:10:bounce}`: `{title}` and `{artist}` scroll independently with their own widths and modes!
2. **Scroll Tag Blocks**:
   - `[scroll:25]{title} - {artist}[/scroll] | {album}`: Scrolls the combined title and artist within 25 characters, leaving album static.

---

## Command-Line Options

All CLI options can be used as one-off overrides or standalone:

| Option | Description | Example |
| :--- | :--- | :--- |
| `-c`, `--config <path>` | Path to custom TOML configuration file | `-c ~/my-config.toml` |
| `--generate-config` | Output default configuration TOML to stdout and exit | `ScrollMPRIS --generate-config` |
| `-s`, `--speed <0-100>` | Scroll speed (0: slow=1000ms, 100: fast=100ms) | `-s 50` |
| `-w`, `--width <number>` | Maximum width for scrolling text | `-w 40` |
| `-b`, `--blocked <list>` | Block certain players (comma-separated, case-insensitive) | `-b edge,firefox,mpv` |
| `--scroll <mode>` | Default scrolling behavior: `marquee`, `restart`, or `bounce` | `--scroll marquee` |
| `--format <string>` | Output format template | `--format '{player_icon} {title:20} - {artist}'` |
| `--tooltip-format <string>` | Tooltip metadata format (resolves all fields un-scrolled) | `--tooltip-format '{title} - {artist} \| {album}'` |
| `--icon-format <string>` | Icon format mapping as JSON. `"404"` defines fallback icon | `--icon-format '{"404": "", "spotify": ""}'` |
| `--freeze` | Pause scrolling and reset text when playback is paused | `--freeze` |

---

## Configuration Recipes & Examples

### 1. Default Clean Ticker
Player brand icon, play/pause state glyph, and track info:
```bash
ScrollMPRIS --format '{player_icon} {status_icon} {title:20} - {artist}'
```

### 2. Status Icon at the End with Elapsed / Total Time
Place the play/pause glyph at the far right with embedded track timers:
```bash
ScrollMPRIS --format '{player_icon} {title:20:marquee} - {artist} [{position}/{length}] {status_icon}'
```

### 3. Countdown Remaining Time
Show time left in track:
```bash
ScrollMPRIS --format '{player_icon} {title:20:bounce} - {artist} (-{remaining})'
```

### 4. Status Icon Only (Minimalist)
Omit the player icon entirely without any special flags:
```bash
ScrollMPRIS --format '{status_icon} {title:25} - {artist}'
```

### 5. Plain Text Only (No Icons)
Simple text output:
```bash
ScrollMPRIS --format '{title:20} - {artist} [{position}]'
```

### 6. Independent Dual-Field Scrolling with Different Modes
Title scrolls in continuous `marquee` (15 chars) while artist scrolls in `bounce` (10 chars):
```bash
ScrollMPRIS -s 50 --format '{player_icon} {title:15:marquee} | {artist:10:bounce} {status_icon}'
```

---

## Preview

**Restart mode:**

![Restart mode](https://github.com/user-attachments/assets/5a151c83-394d-4f12-9660-6f248de1a71d)

**Marquee mode:**

![Marquee mode](https://github.com/user-attachments/assets/c72cc4be-3385-4a53-8848-7c292e12e400)

---

## Process Tracking

The running process PID is written to `/tmp/scrollbarmpris/{timestamp}.pid` for easy instance management.

---

## Contributing

Contributions, feature requests, and issue reports are welcome! Feel free to open an issue or submit a pull request.

---

## License

Unlicense. See [LICENSE](LICENSE) for details.
