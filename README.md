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
- **Configurable Scrolling**: Continuous `wrapping` loop or start/end `reset` mode with configurable speed and width.
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
    "exec": "ScrollMPRIS",
    "escape": true,
    "on-click": "playerctl play-pause"
}
```

### Styling with CSS

Customize the appearance in your Waybar stylesheet (`~/.config/waybar/style.css`):

```css
#custom-ScrollMPRIS {
    /* Default / general style */
}

#custom-ScrollMPRIS.playing {
    /* Style when playback is active */
}

#custom-ScrollMPRIS.paused {
    /* Style when playback is paused */
}

#custom-ScrollMPRIS.stopped {
    /* Style when no player is active */
}

/* Player-specific styles */
#custom-ScrollMPRIS.spotify {
    /* Style specifically for Spotify */
}

#custom-ScrollMPRIS.firefox {
    /* Style specifically for Firefox */
}

#custom-ScrollMPRIS.playing.spotify {
    /* Style when Spotify is actively playing */
}
```

---

## Command-Line Options

| Option | Description | Example |
| :--- | :--- | :--- |
| `-s`, `--speed <0-100>` | Scroll speed (0: slow=1000ms, 100: fast=100ms) | `-s 50` |
| `-w`, `--width <number>` | Maximum width for the scrolling text | `-w 40` |
| `-b`, `--blocked <list>` | Block certain players (comma-separated, case-insensitive) | `-b edge,firefox,mpv` |
| `-p`, `--position` | Enable position display (shows track time info) | `-p` |
| `--scroll <mode>` | Scrolling behavior: `wrapping` (loop) or `reset` (restart after finish) | `--scroll wrapping` |
| `--position-mode <mode>` | Position style: `increasing` (elapsed) or `remaining` (time left) | `--position-mode remaining` |
| `--format <string>` | Metadata format (supports `{title}`, `{artist}`, `{album}`) | `--format '{title} - {artist}'` |
| `--tooltip-format <string>` | Tooltip metadata format (supports `{title}`, `{artist}`, `{album}`) | `--tooltip-format '{title} - {artist} \| {album}'` |
| `--icon-format <string>` | Icon format mapping as JSON. `"404"` defines the fallback icon | `--icon-format '{"404": "", "vlc": "󰕼", "mpv": "", "spotify": ""}'` |
| `--no-icon` | Disable all icons in output | `--no-icon` |
| `--no-status-icon` | Disable only the play/pause status icon | `--no-status-icon` |
| `--switch-icons` | Swap play/pause icons (playing: , paused: ) | `--switch-icons` |
| `--freeze` | Pause scrolling and reset text when playback is paused | `--freeze` |

**Example Command:**

```bash
ScrollMPRIS -s 50 -w 40 -b edge,firefox --scroll wrapping --position --position-mode remaining --format '{title} - {artist}'
```

---

## Preview

**Reset mode:**

![Reset mode](https://github.com/user-attachments/assets/5a151c83-394d-4f12-9660-6f248de1a71d)

**Wrapped mode:**

![Wrapped mode](https://github.com/user-attachments/assets/c72cc4be-3385-4a53-8848-7c292e12e400)

---

## Process Tracking

The running process PID is written to `/tmp/scrollbarmpris/{timestamp}.pid` for easy instance management.

---

## Contributing

Contributions, feature requests, and issue reports are welcome! Feel free to open an issue or submit a pull request.

---

## License

Unlicense. See [LICENSE](LICENSE) for details.
