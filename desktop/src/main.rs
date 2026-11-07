//! Curia's desktop app.
//!   curia-desktop [--data <host data folder>] [--shot <png>] [--live <qml folder>]
//! Without `--data` it follows the host chosen last (at first, the installed one on this computer).
//! `--shot` saves a picture of the window once it has shown the host's threads, then quits.
//! `--live` runs the screens from a QML folder and reloads them when a file there is saved.
// The installed (release) app opens no console window beside its own; development builds
// keep one for QML errors.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod bridge;
mod hosts;
mod live;
mod native;

use cxx_qt_lib::{QFont, QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    // Every Text uses the design's font (QML's own FontLoader registers the file; Qt matches
    // the family by name when it draws).
    if let Some(app) = app.as_mut() {
        let mut font = QFont::default();
        font.set_family(&QString::from("Inter Variable"));
        font.set_pixel_size(13);
        app.set_application_font(&font);
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // A window that fails to load must not leave an invisible program running.
        engine
            .as_mut()
            .on_object_creation_failed(|_, url| {
                eprintln!("curia-desktop: the window could not be loaded ({url}); see the QML errors above");
                std::process::exit(1);
            })
            .release();
        let live = std::env::args().any(|a| a == "--live");
        engine.load(&QUrl::from(if live { "qrc:/qt/qml/Curia/Desktop/qml/Live.qml" } else { "qrc:/qt/qml/Curia/Desktop/qml/Main.qml" }));
    }
    native::restore_window_abilities();
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
