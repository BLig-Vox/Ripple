//! GTK-style client decorations; native window operations stay with winit.
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, ResizeDirection, Sense, Stroke, Vec2,
    ViewportCommand,
};
use egui_lucide::Lucide;

const HEIGHT: f32 = 38.0;
const EDGE: f32 = 5.0;

pub fn draw(ui: &mut egui::Ui, window: Rect) -> Rect {
    let ctx = ui.ctx().clone();
    let (maximized, focused) = ctx.input(|i| {
        (
            i.viewport().maximized.unwrap_or(false),
            i.viewport().focused.unwrap_or(true),
        )
    });
    let header = Rect::from_min_size(window.min, Vec2::new(window.width(), HEIGHT));
    let foreground = if focused {
        Color32::from_gray(225)
    } else {
        Color32::from_gray(150)
    };
    ui.painter().rect_filled(
        header,
        0.0,
        Color32::from_gray(if focused { 36 } else { 31 }),
    );
    ui.painter().hline(
        header.x_range(),
        header.bottom(),
        Stroke::new(1.0, Color32::from_gray(48)),
    );
    egui::Image::new(egui::include_image!("../assets/branding/ripple-logo.png")).paint_at(
        ui,
        Rect::from_center_size(
            Pos2::new(header.left() + 23.0, header.center().y),
            Vec2::splat(19.0),
        ),
    );
    ui.painter().text(
        Pos2::new(header.center().x, header.center().y),
        Align2::CENTER_CENTER,
        "Ripple",
        FontId::proportional(13.0),
        foreground,
    );

    let buttons_left = header.right() - 116.0;
    let drag = Rect::from_min_max(header.min, Pos2::new(buttons_left, header.bottom()));
    let response = ui.interact(
        drag,
        ui.scope_id().with("window_drag"),
        Sense::CLICK | Sense::DRAG,
    );
    if response.double_clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    for (index, icon, label) in [
        (0, Lucide::Minus, "Minimize"),
        (
            1,
            if maximized {
                Lucide::Copy
            } else {
                Lucide::Square
            },
            if maximized { "Restore" } else { "Maximize" },
        ),
        (2, Lucide::X, "Close"),
    ] {
        let center = Pos2::new(buttons_left + 18.0 + index as f32 * 36.0, header.center().y);
        let rect = Rect::from_center_size(center, Vec2::splat(30.0));
        // Window controls don't take the PTY's keyboard focus.
        let response = ui
            .interact(rect, ui.scope_id().with(("window_button", index)), Sense::CLICK)
            .on_hover_text(label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        let hovered = response.hovered() || response.is_pointer_button_down_on();
        if index == 2 {
            ui.painter().circle_filled(
                center,
                12.0,
                Color32::from_gray(if hovered { 76 } else { 52 }),
            );
        } else if hovered {
            ui.painter()
                .rect_filled(rect.shrink(2.0), 6.0, Color32::from_gray(52));
        }
        icon.size(14.0)
            .stroke_width(1.8)
            .color(foreground)
            .image()
            .paint_at(ui, Rect::from_center_size(center, Vec2::splat(14.0)));
        if response.clicked() {
            ctx.send_viewport_cmd(match index {
                0 => ViewportCommand::Minimized(true),
                1 => ViewportCommand::Maximized(!maximized),
                _ => ViewportCommand::Close,
            });
        }
    }
    Rect::from_min_max(
        Pos2::new(window.left() + 1.0, header.bottom() + 1.0),
        Pos2::new(window.right() - 1.0, window.bottom() - 1.0),
    )
}

pub fn resize_edges(ui: &mut egui::Ui, window: Rect) {
    if ui.ctx().input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let p = window.min;
    let q = window.max;
    let e = EDGE;
    let c = 12.0;
    let regions = [
        (
            Rect::from_min_max(Pos2::new(p.x + c, p.y), Pos2::new(q.x - c, p.y + e)),
            ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(Pos2::new(p.x + c, q.y - e), Pos2::new(q.x - c, q.y)),
            ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(Pos2::new(p.x, p.y + c), Pos2::new(p.x + e, q.y - c)),
            ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(Pos2::new(q.x - e, p.y + c), Pos2::new(q.x, q.y - c)),
            ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_size(p, Vec2::splat(c)),
            ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_size(Pos2::new(q.x - c, p.y), Vec2::splat(c)),
            ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(Pos2::new(p.x, q.y - c), Vec2::splat(c)),
            ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(q - Vec2::splat(c), Vec2::splat(c)),
            ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
    ];
    for (index, (rect, direction, cursor)) in regions.into_iter().enumerate() {
        let response = ui.interact(rect, ui.scope_id().with(("window_resize", index)), Sense::DRAG);
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}
