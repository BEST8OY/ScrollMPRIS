/// Spacer used for wrapping scroll mode.
pub const WRAP_SPACER: &str = "   ";
/// Number of cycles to hold at the start/end in reset mode.
pub const RESET_HOLD: usize = 2;

/// Scroll mode for the text output.
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum ScrollMode {
    /// Scrolls text in a continuous loop.
    Wrapping,
    /// Restarts scrolling after reaching the end.
    Reset,
}

/// State for scrolling text.
#[derive(Debug, Default, Clone)]
pub struct ScrollState {
    pub offset: usize,
    pub hold: usize, // Only used for reset mode
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
        ScrollMode::Wrapping => {
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
        ScrollMode::Reset => {
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
        let result = scroll("Short", &mut state, 10, ScrollMode::Wrapping);
        assert_eq!(result, "Short");
    }

    #[test]
    fn test_scroll_short_text_near_boundary() {
        let mut state = ScrollState::new();
        let text = "123456789";
        let frame1 = scroll(text, &mut state, 10, ScrollMode::Wrapping);
        assert_eq!(frame1, "123456789");
        let frame2 = scroll(text, &mut state, 10, ScrollMode::Wrapping);
        assert_eq!(frame2, "123456789");
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn test_scroll_without_advancing() {
        let mut state = ScrollState::new();
        let text = "Long Song Title";
        let frame1 = scroll_frame(text, &mut state, 6, ScrollMode::Wrapping, false);
        assert_eq!(frame1, "Long S");
        let frame2 = scroll_frame(text, &mut state, 6, ScrollMode::Wrapping, false);
        assert_eq!(frame2, "Long S");
        assert_eq!(state.offset, 0);

        let frame3 = scroll_frame(text, &mut state, 6, ScrollMode::Wrapping, true);
        assert_eq!(frame3, "Long S");
        assert_eq!(state.offset, 1);

        let frame4 = scroll_frame(text, &mut state, 6, ScrollMode::Wrapping, false);
        assert_eq!(frame4, "ong So");
        assert_eq!(state.offset, 1);
    }

    #[test]
    fn test_scroll_wrapping() {
        let mut state = ScrollState::new();
        let text = "Hello World";
        let frame1 = scroll(text, &mut state, 5, ScrollMode::Wrapping);
        assert_eq!(frame1, "Hello");
        let frame2 = scroll(text, &mut state, 5, ScrollMode::Wrapping);
        assert_eq!(frame2, "ello ");
        let frame3 = scroll(text, &mut state, 5, ScrollMode::Wrapping);
        assert_eq!(frame3, "llo W");
    }

    #[test]
    fn test_scroll_reset() {
        let mut state = ScrollState::new();
        let text = "ABCDE";
        let frame1 = scroll(text, &mut state, 3, ScrollMode::Reset);
        assert_eq!(frame1, "ABC");
    }

    #[test]
    fn test_scroll_text_change_resets_state() {
        let mut state = ScrollState::new();
        let _ = scroll("First Track", &mut state, 5, ScrollMode::Wrapping);
        let _ = scroll("First Track", &mut state, 5, ScrollMode::Wrapping);
        assert!(state.offset > 0);

        let frame = scroll("Second Track", &mut state, 5, ScrollMode::Wrapping);
        assert_eq!(frame, "Secon");
    }
}