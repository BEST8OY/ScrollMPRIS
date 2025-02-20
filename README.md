# ScrollMPRIS
Scrolling MPRIS module for waybar

```You need playerctl daemon for this module to work```

# How to build?
- ```git clone https://github.com/BEST8OY/ScrollMPRIS.git```
- ```cd ScrollMPRIS```
- ```cargo build --release```
- The executable file will be in ```ScrollMPRIS/target/release/```

# How to add as module to waybar?
- Add this to waybar config
```add this to waybar config
    "custom/ScrollMPRIS": {
    "return-type": "json",
    "exec": "~/.config/waybar/scripts/waybar-scrolling-mpris",
},
```
- You can use these classes in css
```
#custom-ScrollMPRIS
#custom-ScrollMPRIS.playing
#custom-ScrollMPRIS.paused
#custom-ScrollMPRIS.stopped       #this one is practically useless since ScrollMPRIS shows nothing when the state is stopped
```
