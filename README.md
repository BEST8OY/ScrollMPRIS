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
- **Configurable Scrolling**: Industry-standard scrolling modes (`marquee`, `restart`, `bounce`) with configurable speed, width, and pauses.
- **Field-Aware Scrolling**: Scroll individual fields independently (e.g. `{title:20} - {artist}`) or target specific fields via `--scroll-targets`.
- **Rich Status & Formatting**: Custom format strings, tooltips, player icons, play/pause state icons, and position/remaining time display.

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

## Waybar Integration

Add ScrollMPRIS as a custom module in your Waybar configuration (`~/.config/waybar/config`):

```json
"custom/ScrollMPRIS": {
    "return-type": "json",
    "exec": "ScrollMPRIS --format '{title:20:marquee} | {artist:12:bounce}' -b firefox,chromium --freeze",
    "escape": true,
    "on-click": "playerctl play-pause",
    "on-scroll-up": "playerctl next",
    "on-scroll-down": "playerctl previous"
}
```

> [!TIP]
> Always wrap `--format` strings containing `|` or `{...}` in single quotes (`'...'`) to prevent the shell from interpreting `|` as a pipeline.

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
3. **CLI Targets Flag**:
   - `--scroll-targets title`: With `--format "{title} - {artist}" -w 20`, only `{title}` scrolls up to 20 characters.

### Supported Format Tokens

| Token | Description |
| :--- | :--- |
| `{title}` | Track title |
| `{artist}` | Track artist(s) |
| `{album}` | Album name |
| `{player}` | Player name (e.g. `spotify`, `firefox`, `vlc`) |
| `{status}` | Playback status (e.g. `Playing`, `Paused`, `Stopped`) |
| `{position}` | Track elapsed or formatted position (e.g. `01:23`) |
| `{length}` | Track duration (e.g. `03:45`) |

---

## Command-Line Options

| Option | Description | Example |
| :--- | :--- | :--- |
| `-s`, `--speed <0-100>` | Scroll speed (0: slow=1000ms, 100: fast=100ms) | `-s 50` |
| `-w`, `--width <number>` | Maximum width for scrolling text | `-w 40` |
| `-b`, `--blocked <list>` | Block certain players (comma-separated, case-insensitive) | `-b edge,firefox,mpv` |
| `-p`, `--position` | Enable position display (shows track time info) | `-p` |
| `--scroll <mode>` | Scrolling behavior: `marquee`, `restart`, or `bounce` | `--scroll marquee` |
| `--scroll-targets <fields>` | Metadata fields to scroll (e.g. `title` or `title,artist`) | `--scroll-targets title` |
| `--position-mode <mode>` | Position style: `increasing` (elapsed) or `remaining` (time left) | `--position-mode remaining` |
| `--format <string>` | Metadata format (supports tokens, `{title:20}`, `[scroll:...]`) | `--format '{title:20} - {artist}'` |
| `--tooltip-format <string>` | Tooltip metadata format (resolves all fields un-scrolled) | `--tooltip-format '{title} - {artist} \| {album}'` |
| `--icon-format <string>` | Icon format mapping as JSON. `"404"` defines fallback icon | `--icon-format '{"404": "", "vlc": "󰕼", "mpv": "", "spotify": ""}'` |
| `--no-icon` | Disable all icons in output | `--no-icon` |
| `--no-status-icon` | Disable only the play/pause status icon | `--no-status-icon` |
| `--switch-icons` | Swap play/pause icons (playing: , paused: ) | `--switch-icons` |
| `--freeze` | Pause scrolling and reset text when playback is paused | `--freeze` |

---

## Configuration Recipes & Examples

### 1. Title-Only Scrolling with Static Artist
Only long titles scroll within 20 characters, while the artist and separator remain fixed:
```bash
ScrollMPRIS -s 50 --format '{title:20} - {artist}'
```

### 2. Independent Dual-Field Scrolling with Different Modes
Title scrolls in continuous `marquee` (15 chars) while artist scrolls in `bounce` (10 chars):
```bash
ScrollMPRIS -s 50 --format '{title:15:marquee} | {artist:10:bounce}'
```

### 3. Blocked Players & Pause Freeze
Ignore browser audio (e.g., Firefox, Edge), freeze scrolling when music is paused, and block unwanted apps:
```bash
ScrollMPRIS -s 40 -w 35 -b firefox,edge,chromium --freeze
```

### 4. Custom Position & Duration Inside Format String
Embed track time directly anywhere in the format template:
```bash
ScrollMPRIS -s 50 --format '{title:20:marquee} - {artist} [{position}/{length}]'
```

### 5. Custom Tooltip Format
Display full track title, artist, album, and player name on hover:
```bash
ScrollMPRIS --format '{title:20} - {artist}' --tooltip-format '{title} by {artist} on {album} ({player})'
```

### 6. Minimalist (No Icons, Remaining Time)
Clean text-only output showing remaining time:
```bash
ScrollMPRIS --no-icon -p --position-mode remaining --format '{title:25} - {artist}'
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
