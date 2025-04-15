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
#[derive(Debug)]
pub struct ScrollState {
    pub offset: usize,
    pub hold: usize, // Only used for reset mode
    pub last_text: String,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            hold: 0,
            last_text: String::new(),
        }
    }

    fn reset_if_needed(&mut self, text: &str) {
        if text != self.last_text {
            self.last_text = text.to_string();
            self.offset = 0;
            self.hold = 0;
        }
    }
}

/// Scroll text according to mode and width.
pub fn scroll(text: &str, state: &mut ScrollState, width: usize, mode: ScrollMode) -> String {
    state.reset_if_needed(text);
    match mode {
        ScrollMode::Wrapping => {
            let padded = format!("{}{}", text, WRAP_SPACER);
            let chars: Vec<char> = padded.chars().collect();
            if chars.len() <= width {
                return text.to_string();
            }
            let frame: String = (0..width)
                .map(|i| chars[(state.offset + i) % chars.len()])
                .collect();
            state.offset = state.offset.wrapping_add(1);
            frame
        }
        ScrollMode::Reset => {
            let chars: Vec<char> = text.chars().collect();
            if chars.len() <= width {
                return text.to_string();
            }
            let max_offset = chars.len() - width;
            let frame: String = chars.iter().skip(state.offset).take(width).collect();
            if state.offset == 0 || state.offset == max_offset {
                if state.hold < RESET_HOLD {
                    state.hold += 1;
                } else {
                    state.hold = 0;
                    state.offset = if state.offset == max_offset { 0 } else { state.offset + 1 };
                }
            } else {
                state.offset += 1;
            }
            frame
        }
    }
}