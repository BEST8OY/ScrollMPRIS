# ScrollMPRIS

A scrolling MPRIS module for [Waybar](https://github.com/Alexays/Waybar) written in Rust.

> **Note:** This project was generated and improved with the help of AI.

---

## Prerequisites

- **DBus:** Required for inter-process communication.
- **playerctl:** Ensure this daemon is running for MPRIS control.
- **Cargo:** Rust's package manager and build tool ([Learn more](https://doc.rust-lang.org/cargo/)).

## Installation & Build

1. **Clone the Repository:**
   ```bash
   git clone https://github.com/BEST8OY/ScrollMPRIS.git
   cd ScrollMPRIS
   ```
2. **Build the Project:**
   ```bash
   cargo build --release
   ```
3. **Locate the Executable:**
   After a successful build, the binary will be in:
   ```
   ScrollMPRIS/target/release/
   ```

## Waybar Integration

To add ScrollMPRIS as a custom module in Waybar, insert the following snippet into your Waybar config:
```json
"custom/ScrollMPRIS": {
    "return-type": "json",
    "exec": "/path/to/ScrollMPRIS",
    "escape": true,
    "on-click": "playerctl play-pause"
},
```
Replace `/path/to/ScrollMPRIS` with the actual path to your built binary.

### Styling with CSS
You can customize the module's appearance using these selectors in your Waybar style:
```css
#custom-ScrollMPRIS,
#custom-ScrollMPRIS.playing,
#custom-ScrollMPRIS.paused,
```

## Command-Line Options

ScrollMPRIS offers several command-line options to tailor its behavior:

| Option                        | Description                                                                                 | Example                                  |
|-------------------------------|---------------------------------------------------------------------------------------------|------------------------------------------|
| `-s`, `--speed <0-100>`       | Scroll speed (0: slow=1000ms, 100: fast=100ms)                                              | `-s 50`                                  |
| `-w`, `--width <number>`      | Maximum width for the scrolling text                                                        | `-w 40`                                  |
| `-b`, `--blocked <list>`      | Block certain players (comma-separated, case-insensitive)                                   | `-b edge,firefox,mpv`                    |
| `-p`, `--position`            | Enable position display (show track time info)                                              | `-p` or `--position`                     |
| `--scroll <wrapping OR reset>`   | Choose scrolling behavior: `wrapping` for continuous loop, `reset` to restart after finish  | `--scroll wrapping`                      |
| `--position-mode <mode>`      | Position style: `increasing` (elapsed) or `remaining` (time left)                           | `--position-mode remaining`              |
| `--format <string>`           | Metadata format (supports `{title}`, `{artist}`, `{album}`)                                 | `--format '{title} - {artist}'`          |
| `--no-icon`                   | Disable icon in output                                                                        | `--no-icon`                              |

**Examples:**
```bash
./ScrollMPRIS -s 50 -w 40 -b edge,firefox,mpv --scroll wrapping --position --position-mode remaining --format '{title} - {artist}' --no-icon
```

- To enable position display, simply add `-p` or `--position` (no value needed).
- To disable, omit the flag.

## Preview

**Reset mode:**

![Reset mode](https://github.com/user-attachments/assets/5a151c83-394d-4f12-9660-6f248de1a71d)

**Wrapped mode:**

![Wrapped mode](https://github.com/user-attachments/assets/c72cc4be-3385-4a53-8848-7c292e12e400)



## Contributing

Contributions, feature requests, and issue reports are always welcome!
Feel free to open an issue or submit a pull request.

## Credits
- **ScrollMPRIS** and this **README** were written and improved using AI.

## License
Unlicensed. See LICENSE for details.
