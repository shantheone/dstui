pub mod app;
use ratatui::layout::Rect;

// Helper function for the popups

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    // raw size
    let raw_w = area.width.saturating_mul(percent_x) / 100;
    let raw_h = area.height.saturating_mul(percent_y) / 100;

    // enforce minimum of 3 (1 border + 1 content + 1 border)
    let w = raw_w.max(3).min(area.width);
    let h = raw_h.max(3).min(area.height);

    // center it in `area`
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;

    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
