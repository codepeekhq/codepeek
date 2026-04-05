use std::time::Duration;

pub const TICK_RATE: Duration = Duration::from_millis(16);

pub const PAGE_SCROLL_LINES: usize = 20;

pub const MAX_LINE_LENGTH: usize = 500;

pub const BINARY_DETECTION_LIMIT: usize = 8192;

/// Outer margin around the entire UI (cells on each side).
pub const OUTER_MARGIN: u16 = 1;

pub const FILE_LIST_WIDTH_PERCENT: u16 = 28;
pub const FILE_VIEWER_WIDTH_PERCENT: u16 = 72;

pub const POPUP_WIDTH_PERCENT: u16 = 70;
pub const POPUP_HEIGHT_PERCENT: u16 = 80;

/// Gap between adjacent panels (cells).
pub const PANEL_GAP: u16 = 1;
