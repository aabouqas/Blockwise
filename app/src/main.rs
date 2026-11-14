//! Blockwise — a desktop app builder: design screens, snap logic blocks together,
//! try the app in a pretend phone and export its Android source.

// release builds on Windows open no console window next to the app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod android_setup;
mod app;
mod archive;
mod asset_view;
mod blocks;
#[cfg(test)]
mod check;
mod codegen;
mod designer;
mod device;
mod examples;
mod highlight;
mod icons;
mod logic;
mod mcp;
mod menu;
mod model;
mod native;
mod phone;
mod runtime;
mod templates;
mod theme;
mod player;
mod viewer;

fn arg_after(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

/// The Blockwise icon for the window and the Dock: the bolt made of two
/// puzzle pieces, white on indigo (drawn in brand/app-icon.svg at the top of
/// the repository).
fn app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/brand/icon-512.png")).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // `Blockwise --mcp`: relay an MCP client's stdio to the running app
    if args.iter().any(|a| a == "--mcp") {
        let port = arg_after(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or_else(|| model::Store::load().settings.mcp_port);
        if let Err(e) = mcp::bridge(port) {
            eprintln!("blockwise --mcp: {e}");
        }
        return Ok(());
    }
    let shot = arg_after(&args, "--screenshot");
    let open = arg_after(&args, "--open");

    let mut viewport = egui::ViewportBuilder::default().with_title("Blockwise").with_inner_size([1440.0, 920.0]).with_min_inner_size([960.0, 640.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    if shot.is_some() {
        // open quietly: don't take focus from whatever the person is doing
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native("Blockwise", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot, open)))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_app_icon_loads() {
        let icon = super::app_icon().expect("the bundled icon should decode");
        assert_eq!((icon.width, icon.height), (512, 512));
        assert_eq!(icon.rgba.len(), 512 * 512 * 4);
    }
}
