#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adb;
mod app;
mod scanner;
mod storage;

use app::AdbApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default()
        .with_inner_size([1000.0, 720.0])
        .with_min_inner_size([820.0, 580.0])
        .with_title("ADB Bridge GUI");

    eframe::run_native(
        "ADB Bridge GUI",
        native_options,
        Box::new(|cc| Ok(Box::new(AdbApp::new(cc)))),
    )
}
