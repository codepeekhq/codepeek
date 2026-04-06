use std::time::Duration;

pub const TICK_RATE: Duration = Duration::from_millis(16);

pub const PAGE_SCROLL_LINES: usize = 20;

pub const MAX_LINE_LENGTH: usize = 500;

pub const BINARY_DETECTION_LIMIT: usize = 8192;

/// Outer margin around the entire UI (cells on each side).
pub const OUTER_MARGIN: u16 = 1;

pub const POPUP_WIDTH_PERCENT: u16 = 70;
pub const POPUP_HEIGHT_PERCENT: u16 = 80;

/// Max width for the file list in zen mode (cells).
pub const ZEN_FILE_LIST_MAX_WIDTH: u16 = 60;
/// Width percentage for the file viewer in zen mode.
pub const ZEN_VIEWER_WIDTH_PERCENT: u16 = 80;
/// Max height percentage for the file list in zen mode.
pub const ZEN_FILE_LIST_MAX_HEIGHT_PERCENT: u16 = 70;
