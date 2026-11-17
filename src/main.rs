mod app;
mod clipboard;
mod keyboard;
mod links;
mod terminal;
mod window_chrome;
mod window_geometry;

fn main() -> eframe::Result {
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/branding/ripple-logo.png"))
            .expect("bundled Ripple icon must be a valid PNG");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Ripple")
            .with_decorations(false)
            .with_app_id("ripple")
            .with_icon(icon)
            .with_inner_size([1024.0, 640.0])
            .with_min_inner_size([720.0, 420.0]),
        centered: true,
        renderer: eframe::Renderer::Wgpu,
        clipboard_shortcuts: false,
        ..Default::default()
    };

    eframe::run_native(
        "Ripple",
        options,
        Box::new(|creation_context| Ok(Box::new(app::RippleApp::new(creation_context)))),
    )
}
