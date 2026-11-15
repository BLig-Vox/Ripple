use eframe::egui::{self, Pos2, Vec2, ViewportCommand};

const SCREEN_FRACTION: f32 = 0.8;

/// Size the first window to 80% of the active monitor and center it there.
/// The fallback in `main` is used when the window system exposes no monitor.
pub fn apply_initial_geometry(cc: &eframe::CreationContext<'_>) {
    let Some(window) = cc.winit_window() else {
        return;
    };
    let Some(monitor) = window.current_monitor() else {
        return;
    };

    let scale = window.scale_factor() as f32;
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }

    let monitor_size = monitor.size();
    let monitor_position = monitor.position();
    let size = Vec2::new(
        (monitor_size.width as f32 * SCREEN_FRACTION / scale).max(720.0),
        (monitor_size.height as f32 * SCREEN_FRACTION / scale).max(420.0),
    );
    let origin = Pos2::new(
        monitor_position.x as f32 / scale,
        monitor_position.y as f32 / scale,
    );
    let centered_position = origin
        + (Vec2::new(
            monitor_size.width as f32 / scale,
            monitor_size.height as f32 / scale,
        ) - size)
            * 0.5;

    cc.egui_ctx
        .send_viewport_cmd(ViewportCommand::InnerSize(size));
    cc.egui_ctx
        .send_viewport_cmd(ViewportCommand::OuterPosition(centered_position));
}

#[allow(dead_code)]
fn _egui_types_are_publicly_reexported(_: egui::Context) {}
