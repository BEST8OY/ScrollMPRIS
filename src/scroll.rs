///
/// Contains text scrolling utilities.
///
pub const SCROLL_SPACER: &str = "   ";

///
/// Scrolls the provided text by taking a substring of fixed length based on the offset.
/// The text is padded with a spacer to allow for smooth scrolling.
///
pub fn scroll_text(text: &str, offset: usize, width: usize) -> String {
    let padded = format!("{}{}", text, SCROLL_SPACER);
    let chars: Vec<char> = padded.chars().collect();
    let len = chars.len();

    // If text length is less than or equal to the display width, no scrolling is needed.
    if len <= width {
        return text.to_string();
    }

    (0..width)
        .map(|i| chars[(offset + i) % len])
        .collect()
}
