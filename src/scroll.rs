/// Spacer used for marquee scroll mode.
pub const WRAP_SPACER: &str = "   ";
/// Number of cycles to hold at boundaries in restart/bounce mode.
pub const RESET_HOLD: usize = 2;

/// Direction for bounce scroll mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    #[default]
    Forward,
    Backward,
}

/// Scroll mode for text output using industry-standard names.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default, serde::Deserialize, serde::Serialize,
)]
#[clap(rename_all = "kebab-case")]
#[serde(rename_all = "lowercase")]
pub enum ScrollMode {
    /// Scrolls text continuously in a ticker loop.
    #[default]
    Marquee,
    /// Scrolls to the end, holds, and jumps back to start.
    Restart,
    /// Scrolls to the end, holds, reverses direction to start, holds, and repeats.
    Bounce,
}

impl std::str::FromStr for ScrollMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "marquee" => Ok(ScrollMode::Marquee),
            "restart" => Ok(ScrollMode::Restart),
            "bounce" => Ok(ScrollMode::Bounce),
            other => Err(format!(
                "Unknown scroll mode '{other}'. Valid modes: marquee, restart, bounce"
            )),
        }
    }
}

/// State for scrolling text.
#[derive(Debug, Default, Clone)]
pub struct ScrollState {
    pub offset: usize,
    pub hold: usize,
    pub direction: ScrollDirection,
    pub last_text: String,
    pub chars: Vec<char>,
    pub padded_chars: Vec<char>,
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_if_needed(&mut self, text: &str) {
        if text != self.last_text {
            self.last_text = text.to_string();
            self.chars = text.chars().collect();
            let mut padded = self.chars.clone();
            padded.extend(WRAP_SPACER.chars());
            self.padded_chars = padded;
            self.offset = 0;
            self.hold = 0;
            self.direction = ScrollDirection::Forward;
        }
    }
}

/// Render a scroll frame. If `advance` is true, advances the scroll position for subsequent calls.
pub fn scroll_frame(
    text: &str,
    state: &mut ScrollState,
    width: usize,
    mode: ScrollMode,
    advance: bool,
) -> String {
    state.reset_if_needed(text);

    // If the original text fits within width, do not scroll
    if state.chars.len() <= width {
        return text.to_string();
    }

    match mode {
        ScrollMode::Marquee => {
            let len = state.padded_chars.len();
            if len == 0 {
                return String::new();
            }
            let frame: String = (0..width)
                .map(|i| state.padded_chars[(state.offset + i) % len])
                .collect();
            if advance {
                state.offset = (state.offset + 1) % len;
            }
            frame
        }
        ScrollMode::Restart => {
            let len = state.chars.len();
            let max_offset = len.saturating_sub(width);
            let frame: String = state.chars.iter().skip(state.offset).take(width).collect();
            if advance {
                if state.offset == 0 || state.offset >= max_offset {
                    if state.hold < RESET_HOLD {
                        state.hold += 1;
                    } else {
                        state.hold = 0;
                        state.offset = if state.offset >= max_offset { 0 } else { state.offset + 1 };
                    }
                } else {
                    state.offset += 1;
                }
            }
            frame
        }
        ScrollMode::Bounce => {
            let len = state.chars.len();
            let max_offset = len.saturating_sub(width);
            let frame: String = state.chars.iter().skip(state.offset).take(width).collect();
            if advance {
                match state.direction {
                    ScrollDirection::Forward => {
                        if state.offset >= max_offset {
                            if state.hold < RESET_HOLD {
                                state.hold += 1;
                            } else {
                                state.hold = 0;
                                state.direction = ScrollDirection::Backward;
                                state.offset = max_offset.saturating_sub(1);
                            }
                        } else {
                            state.offset += 1;
                        }
                    }
                    ScrollDirection::Backward => {
                        if state.offset == 0 {
                            if state.hold < RESET_HOLD {
                                state.hold += 1;
                            } else {
                                state.hold = 0;
                                state.direction = ScrollDirection::Forward;
                                state.offset = 1.min(max_offset);
                            }
                        } else {
                            state.offset = state.offset.saturating_sub(1);
                        }
                    }
                }
            }
            frame
        }
    }
}

/// Scroll text according to mode and width, advancing the scroll position.
pub fn scroll(text: &str, state: &mut ScrollState, width: usize, mode: ScrollMode) -> String {
    scroll_frame(text, state, width, mode, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_short_text() {
        let mut state = ScrollState::new();
        let result = scroll("Short", &mut state, 10, ScrollMode::Marquee);
        assert_eq!(result, "Short");
    }

    #[test]
    fn test_scroll_short_text_near_boundary() {
        let mut state = ScrollState::new();
        let text = "123456789";
        let frame1 = scroll(text, &mut state, 10, ScrollMode::Marquee);
        assert_eq!(frame1, "123456789");
        let frame2 = scroll(text, &mut state, 10, ScrollMode::Marquee);
        assert_eq!(frame2, "123456789");
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn test_scroll_without_advancing() {
        let mut state = ScrollState::new();
        let text = "Long Song Title";
        let frame1 = scroll_frame(text, &mut state, 6, ScrollMode::Marquee, false);
        assert_eq!(frame1, "Long S");
        let frame2 = scroll_frame(text, &mut state, 6, ScrollMode::Marquee, false);
        assert_eq!(frame2, "Long S");
        assert_eq!(state.offset, 0);

        let frame3 = scroll_frame(text, &mut state, 6, ScrollMode::Marquee, true);
        assert_eq!(frame3, "Long S");
        assert_eq!(state.offset, 1);

        let frame4 = scroll_frame(text, &mut state, 6, ScrollMode::Marquee, false);
        assert_eq!(frame4, "ong So");
        assert_eq!(state.offset, 1);
    }

    #[test]
    fn test_scroll_marquee() {
        let mut state = ScrollState::new();
        let text = "Hello World";
        let frame1 = scroll(text, &mut state, 5, ScrollMode::Marquee);
        assert_eq!(frame1, "Hello");
        let frame2 = scroll(text, &mut state, 5, ScrollMode::Marquee);
        assert_eq!(frame2, "ello ");
        let frame3 = scroll(text, &mut state, 5, ScrollMode::Marquee);
        assert_eq!(frame3, "llo W");
    }

    #[test]
    fn test_scroll_restart() {
        let mut state = ScrollState::new();
        let text = "ABCDE";
        // len 5, width 3 => max_offset = 2
        let frame1 = scroll(text, &mut state, 3, ScrollMode::Restart);
        assert_eq!(frame1, "ABC");
        assert_eq!(state.hold, 1); // hold at offset 0
        let frame2 = scroll(text, &mut state, 3, ScrollMode::Restart);
        assert_eq!(frame2, "ABC");
        assert_eq!(state.hold, 2);
        let frame3 = scroll(text, &mut state, 3, ScrollMode::Restart);
        assert_eq!(frame3, "ABC"); // hold reached RESET_HOLD, moves to offset 1
        assert_eq!(state.offset, 1);
        let frame4 = scroll(text, &mut state, 3, ScrollMode::Restart);
        assert_eq!(frame4, "BCD");
        assert_eq!(state.offset, 2);
        let frame5 = scroll(text, &mut state, 3, ScrollMode::Restart);
        assert_eq!(frame5, "CDE"); // max_offset reached, hold 1
        assert_eq!(state.hold, 1);
    }

    #[test]
    fn test_scroll_bounce() {
        let mut state = ScrollState::new();
        let text = "ABCDE"; // len 5, width 3 => max_offset = 2
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "ABC"); // offset 0 -> 1
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "BCD"); // offset 1 -> 2 (reached max)
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "CDE"); // hold 1
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "CDE"); // hold 2
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "CDE"); // hold=2 -> dir=Backward, offset=1
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "BCD"); // dir=Backward, offset=0 (reached 0)
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "ABC"); // hold 1
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "ABC"); // hold 2
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "ABC"); // hold=2 -> dir=Forward, offset=1
        assert_eq!(scroll(text, &mut state, 3, ScrollMode::Bounce), "BCD");
    }

    #[test]
    fn test_scroll_text_change_resets_state() {
        let mut state = ScrollState::new();
        let _ = scroll("First Track", &mut state, 5, ScrollMode::Marquee);
        let _ = scroll("First Track", &mut state, 5, ScrollMode::Marquee);
        assert!(state.offset > 0);

        let frame = scroll("Second Track", &mut state, 5, ScrollMode::Marquee);
        assert_eq!(frame, "Secon");
    }
}