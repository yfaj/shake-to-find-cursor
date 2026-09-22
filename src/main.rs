// Win32 GUI app: no terminal window on launch (release builds).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod animator;
mod single_instance;
mod cursor_helper;
mod detector;
mod fullscreen;
mod hook;
mod settings;
mod tray;
mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub static ENABLED: AtomicBool = AtomicBool::new(true);

fn main() {
    // Single instance: second launch signals the first to open the UI and exits.
    if !single_instance::acquire() {
        single_instance::signal_show();
        return;
    }
    single_instance::spawn_show_listener();

    let app_dir = settings::app_dir();
    let _ = std::fs::create_dir_all(&app_dir);
    let cfg_path = app_dir.join("settings.cfg");
    let cfg = settings::Settings::load(&cfg_path);
    let cfg = Arc::new(RwLock::new(cfg));
    let dirty = Arc::new(AtomicBool::new(false));
    let rebuild = Arc::new(AtomicBool::new(false));

    // Cursor frame cache build + magnification rebuild watcher (background).
    {
        let cfg = cfg.clone();
        let rebuild = rebuild.clone();
        std::thread::spawn(move || {
            let factor = cfg.read().unwrap().magnification;
            cursor_helper::init_caches(factor);
            loop {
                if rebuild.swap(false, Ordering::Relaxed) {
                    let factor = cfg.read().unwrap().magnification;
                    cursor_helper::init_caches(factor);
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });
    }

    // Restore enabled state from settings.
    ENABLED.store(cfg.read().unwrap().enabled, Ordering::Relaxed);

    let detector = Arc::new(detector::Detector::new(&cfg.read().unwrap()));
    let wake = Arc::new(AtomicBool::new(false));

    {
        let cfg = cfg.clone();
        let det = detector.clone();
        let wake = wake.clone();
        std::thread::spawn(move || animator::run(det, cfg, wake));
    }

    {
        let cfg = cfg.clone();
        let det = detector.clone();
        let wake = wake.clone();
        std::thread::spawn(move || hook::install_and_pump(cfg, det, wake));
    }

    // Tray on its own thread (window + menu + message pump).
    {
        let cfg = cfg.clone();
        let dirty = dirty.clone();
        let det = detector.clone();
        std::thread::spawn(move || {
            tray::run(tray::TrayDeps {
                cfg,
                dirty,
                det,
            });
        });
    }

    // Settings persistence watcher.
    {
        let cfg = cfg.clone();
        let dirty = dirty.clone();
        let rebuild = rebuild.clone();
        let last_mag = Arc::new(Mutex::new(cfg.read().unwrap().magnification));
        std::thread::spawn(move || loop {
            if dirty.swap(false, Ordering::Relaxed) {
                let mag = cfg.read().unwrap().magnification;
                cfg.read().unwrap().save(&settings::app_dir().join("settings.cfg"));
                let mut lm = last_mag.lock().unwrap();
                if (mag - *lm).abs() > 1e-9 {
                    *lm = mag;
                    rebuild.store(true, Ordering::Relaxed);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        });
    }

    // Main thread: egui event loop (must be main thread on Windows/winit).
    // Window starts hidden; tray left-click sets ui::SHOW_REQUEST.
    ui::run_main_window(ui::UiState {
        cfg,
        dirty,
        rebuild,
        det: detector,
    });

    // Event loop returned (window closed) -> shut everything down.
    hook::post_quit();
    cursor_helper::restore_theme_cursors();
}
