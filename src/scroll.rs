/// Spacer used for wrapping scroll mode.
pub const WRAP_SPACER: &str = "   ";
/// Number of cycles to hold at the start/end in reset mode.
pub const RESET_HOLD: usize = 2;

/// Manages state for mod-style wrapping scrolling.
#[derive(Debug)]
pub struct WrappingState {
    pub offset: usize,
    pub last_text: String,
}

impl WrappingState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            last_text: String::new(),
        }
    }
}

/// Scrolls text in a wrapping style by appending a spacer and using modulo arithmetic.
pub fn wrapping(text: &str, state: &mut WrappingState, width: usize) -> String {
    if text != state.last_text {
        state.last_text = text.to_string();
        state.offset = 0;
    }
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

/// Manages state for reset scroll-mode.
#[derive(Debug)]
pub struct ResetState {
    pub offset: usize,
    pub hold: usize,
    pub last_text: String,
}

impl ResetState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            hold: 0,
            last_text: String::new(),
        }
    }
}

/// Scrolls text in reset mode with a holding period at the beginning and end.
pub fn reset(text: &str, state: &mut ResetState, width: usize) -> String {
    if text != state.last_text {
        state.last_text = text.to_string();
        state.offset = 0;
        state.hold = 0;
    }
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
            state.offset = if state.offset == max_offset {
                0
            } else {
                state.offset + 1
            };
        }
    } else {
        state.offset += 1;
    }
    frame
}