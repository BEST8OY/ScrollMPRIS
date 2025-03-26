# ScrollMPRIS
Scrolling MPRIS module for waybar

`You need playerctl daemon and dbus for this module to work`

`This code is made by AI`

# Dependencies:
- cargo
# How to build?

- `git clone https://github.com/BEST8OY/ScrollMPRIS.git`
- `cd ScrollMPRIS`
- `cargo build --release`
- The executable file will be in `ScrollMPRIS/target/release/`

# How to add as module to waybar?
- Add this to waybar config
```
    "custom/waybar-scrolling-mpris": {
    "return-type": "json",
    "exec": "~/.config/waybar/scripts/ScrollMPRIS",
    "on-click": "playerctl play-pause",
},
```
- You can use these classes in css
```
#custom-ScrollMPRIS
#custom-ScrollMPRIS.playing
#custom-ScrollMPRIS.paused
#custom-ScrollMPRIS.stopped       #this one is practically useless since ScrollMPRIS shows nothing when the state is stopped
```
# Customizations
```
usage: ScrollMPRIS [options]
    options:
    -s 50                         // Scroll speed (0: slow=1000ms, 100: fast=100ms) ---> Use number between 0-100
    -w 40                         // Max width
    -b edge,firefox,mpv           // Use this to block certain players
```
# To do?
?
