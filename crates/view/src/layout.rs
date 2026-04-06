use ratatui::layout::{Constraint, Flex, Layout, Rect};

use crate::config;

/// Layout result for zen-mode views: a content area and a status bar below it.
pub struct ZenLayout {
    pub content: Rect,
    pub status: Rect,
}

/// Centered layout for the file list: constrained width + height, centered both axes.
pub fn zen_file_list_layout(area: Rect) -> ZenLayout {
    let list_height = area
        .height
        .saturating_mul(config::ZEN_FILE_LIST_MAX_HEIGHT_PERCENT)
        / 100;
    let content_height = list_height + 2; // +1 gap +1 status

    let [centered_v] = Layout::vertical([Constraint::Length(content_height)])
        .flex(Flex::Center)
        .areas(area);

    let list_width = config::ZEN_FILE_LIST_MAX_WIDTH.min(area.width.saturating_sub(4));
    let [centered_h] = Layout::horizontal([Constraint::Length(list_width)])
        .flex(Flex::Center)
        .areas(centered_v);

    split_content_status(centered_h)
}

/// Centered layout for the file viewer: wide content, centered horizontally.
pub fn zen_viewer_layout(area: Rect) -> ZenLayout {
    let viewer_width = area.width.saturating_mul(config::ZEN_VIEWER_WIDTH_PERCENT) / 100;
    let [centered_h] = Layout::horizontal([Constraint::Length(viewer_width)])
        .flex(Flex::Center)
        .areas(area);

    split_content_status(centered_h)
}

/// Center a rect by percentage within an area.
pub fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let width = area.width * percent_x / 100;
    let height = area.height * percent_y / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

fn split_content_status(area: Rect) -> ZenLayout {
    let [content, _gap, status] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    ZenLayout { content, status }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_computes_correct_dimensions() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(area, 70, 80);
        assert_eq!(popup.width, 70);
        assert_eq!(popup.height, 40);
        assert_eq!(popup.x, 15);
        assert_eq!(popup.y, 5);
    }

    #[test]
    fn centered_rect_with_small_area() {
        let area = Rect::new(0, 0, 10, 10);
        let popup = centered_rect(area, 70, 80);
        assert_eq!(popup.width, 7);
        assert_eq!(popup.height, 8);
    }

    #[test]
    fn zen_file_list_layout_produces_valid_areas() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = zen_file_list_layout(area);
        assert!(layout.content.height > 0);
        assert_eq!(layout.status.height, 1);
        assert!(layout.content.width <= config::ZEN_FILE_LIST_MAX_WIDTH);
    }

    #[test]
    fn zen_viewer_layout_produces_valid_areas() {
        let area = Rect::new(0, 0, 120, 40);
        let layout = zen_viewer_layout(area);
        assert!(layout.content.height > 0);
        assert_eq!(layout.status.height, 1);
        let expected_width = area.width * config::ZEN_VIEWER_WIDTH_PERCENT / 100;
        assert_eq!(layout.content.width, expected_width);
    }
}
