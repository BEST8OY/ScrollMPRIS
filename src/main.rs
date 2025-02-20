use std::thread;
use std::time::Duration;
use serde_json::json;

mod mpris;
mod scroll;

const MAX_DISPLAY_WIDTH: usize = 40;

///
/// Updates the display status by querying for an active MPRIS player. This function creates
/// the JSON output and prints it to stdout. If the metadata is longer than the available width,
/// the text is scrolled continuously.
///
fn update_status(scroll_offset: &mut usize) {
    // Try to obtain an active MPRIS player.
    let (icon, metadata, status_class) = if let Some(player) = mpris::get_active_player() {
        player.output_parts()
    } else {
        (String::new(), String::new(), "none".to_string())
    };

    // Scroll metadata if it overflows; otherwise use it as is.
    let scrolled_metadata = if metadata.is_empty() {
        String::new()
    } else if metadata.chars().count() > MAX_DISPLAY_WIDTH {
        let text = scroll::scroll_text(&metadata, *scroll_offset, MAX_DISPLAY_WIDTH);
        *scroll_offset = scroll_offset.wrapping_add(1);
        text
    } else {
        metadata
    };

    let display_text = format!("{}{}", icon, scrolled_metadata);
    let output = json!({
        "text": display_text,
        "class": status_class,
    });
    println!("{}", output.to_string());
}

fn main() {
    let mut scroll_offset: usize = 0;

    loop {
        update_status(&mut scroll_offset);
        thread::sleep(Duration::from_secs(1));
    }
}
