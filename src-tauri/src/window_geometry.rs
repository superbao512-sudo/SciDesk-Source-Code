// Window bounds use physical work-area coordinates and logical client dimensions.
// Account for decorations so both the title bar and bottom edge stay on screen.
pub const MIN_WIDTH: f64 = 800.0;
pub const MIN_HEIGHT: f64 = 500.0;

pub fn fit_client_size(width: f64, height: f64, work_width: u32, work_height: u32, scale: f64, frame_width: u32, frame_height: u32) -> (f64, f64) {
    let available_width = (work_width.saturating_sub(frame_width + 16) as f64 / scale).max(1.0);
    let available_height = (work_height.saturating_sub(frame_height + 16) as f64 / scale).max(1.0);
    (width.max(MIN_WIDTH.min(available_width)).min(available_width), height.max(MIN_HEIGHT.min(available_height)).min(available_height))
}
