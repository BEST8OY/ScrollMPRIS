# ScrollMPRIS
Scrolling MPRIS module for waybar

`You need playerctl daemon for this module to work`

`This code is made by AI`

# How to build?
- `git clone https://github.com/BEST8OY/ScrollMPRIS.git`
- `cd ScrollMPRIS`
- `cargo build --release`
- The executable file will be in `ScrollMPRIS/target/release/`

# How to add as module to waybar?
- Add this to waybar config
```
    "custom/ScrollMPRIS": {
    "return-type": "json",
    "exec": "~/.config/waybar/scripts/ScrollMPRIS",
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
- To change the max width chnage line 8 of `mpris.rs` file; ScrollMPRIS will start scrolling if the text is bigger than this number. 
    -     const MAX_DISPLAY_WIDTH: usize = 40;
# To do?
- [ ] The current format is `icon title - artist`
    - Commandline format choosing?

- [ ] Control scrolling speed?
- [ ] Player blacklisting?
