#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod config;
mod hotkey;
mod translator;
mod tray;

use app::QuickTranApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let icon_data = match image::load_from_memory(include_bytes!("../src-tauri/icons/32x32.png")) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            Some(egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            })
        }
        Err(_) => None,
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("QuickTran Native")
        .with_inner_size([440.0, 390.0])
        .with_min_inner_size([380.0, 360.0]);

    if let Some(ic) = icon_data {
        viewport = viewport.with_icon(ic);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "QuickTran Native",
        options,
        Box::new(|cc| Ok(Box::new(QuickTranApp::new(cc)))),
    )
}
