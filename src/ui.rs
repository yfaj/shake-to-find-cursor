use crate::settings::Settings;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// Set by the tray when the window should be shown/focused.
pub static SHOW_REQUEST: AtomicBool = AtomicBool::new(false);

/// Shared egui Context, captured on the first frame.
static UI_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Called by the tray (any thread): show the settings window directly via Win32.
pub fn request_show() {
    debug_log("request_show called (tray thread)");
    let Some(raw) = find_settings_hwnd() else {
        debug_log("settings HWND not found");
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(raw as *mut _);
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, ShowWindow, SetForegroundWindow,
            SWP_NOZORDER, SWP_SHOWWINDOW,
            SW_RESTORE,
        };
        #[repr(C)]
        struct RECTc { left: i32, top: i32, right: i32, bottom: i32 }
        let mut wa = RECTc { left: 0, top: 0, right: 0, bottom: 0 };
        let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA,
            0,
            Some((&raw mut wa).cast()),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96) as f32;
        let scale = dpi / 96.0;
        let w = (340.0 * scale) as i32;
        let h = (320.0 * scale) as i32;
        let x = wa.right - w - 16;
        let y = wa.bottom - h - 16;
        // Topmost so it never buries itself under other windows; the dot (X) or
        // entering a fullscreen app are the ways it goes away.
        let topmost = windows::Win32::Foundation::HWND((-1isize) as *mut core::ffi::c_void);
        let _ = SetWindowPos(hwnd, Some(topmost), x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
        debug_log("window shown via win32");
    }
}

pub struct UiState {
    pub cfg: Arc<RwLock<Settings>>,
    pub dirty: Arc<AtomicBool>,
    pub rebuild: Arc<AtomicBool>,
    pub det: Arc<crate::detector::Detector>,
}

struct App {
    state: UiState,
    last_mag: f64,
    want_visible: bool,
    hide_pending: bool,
    github_tex: Option<egui::TextureHandle>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if UI_CTX.set(ctx.clone()).is_ok() {
            debug_log("first update: UI_CTX captured");
        }
        // Cancel any close request: closing hides to tray; exit only via tray menu.
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.want_visible = false;
            self.hide_pending = true;
        }

        // eframe forces the window visible after the first frame; hide it back.
        if self.hide_pending {
            self.hide_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        } else {
            SHOW_REQUEST.store(false, Ordering::Relaxed);
        }

        // Dark theme.
        let mut vis = egui::Visuals::dark();
        vis.window_fill = egui::Color32::TRANSPARENT;
        vis.panel_fill = egui::Color32::TRANSPARENT;
        vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(45, 45, 48);
        vis.widgets.inactive.bg_fill = egui::Color32::from_rgb(58, 58, 60);
        vis.widgets.hovered.bg_fill = egui::Color32::from_rgb(72, 72, 74);
        vis.selection.stroke.color = egui::Color32::from_rgb(10, 132, 255);
        vis.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(229, 229, 234);
        ctx.set_visuals(vis);

        // Card fills the window minus a 1px margin; 1px border, 8px corners.
        let screen = ctx.screen_rect();
        let m = 1.0;
        let card_rect = egui::Rect::from_min_size(
            screen.min + egui::vec2(m, m),
            screen.size() - egui::vec2(2.0 * m, 2.0 * m),
        );

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT).inner_margin(0.0))
            .show(ctx, |ui| {
                ui.painter().rect_filled(card_rect, 8.0, egui::Color32::from_rgb(28, 28, 30));
                ui.painter().rect_stroke(card_rect, 8.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 84)), egui::StrokeKind::Outside);

                let inner = egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::same(16));
                inner.show(ui, |ui| {
                    ui.set_width(card_rect.width() - 32.0);
                    ui.set_max_height(card_rect.height() - 32.0);

                    // Title bar with glowing dot.
                    let bar_h = 22.0;
                    ui.horizontal(|ui| {
                        ui.set_height(bar_h);
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("Shake to Find Cursor").size(15.0).strong());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (dot_rect, dot_resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            let center = dot_rect.center();
                            let radius = 5.0;
                            let painter = ui.painter();
                            if dot_resp.hovered() {
                                for i in 0..5 {
                                    let t = i as f32 / 4.0;
                                    let r = radius + 2.0 + t * 7.0;
                                    let a = (1.0 - t) * 70.0;
                                    painter.circle_filled(center, r, egui::Color32::from_rgba_unmultiplied(232, 17, 35, a as u8));
                                }
                                painter.circle_filled(center, radius + 1.0, egui::Color32::from_rgb(232, 17, 35));
                                painter.circle_filled(center, radius - 1.0, egui::Color32::from_rgb(255, 120, 130));
                            } else {
                                painter.circle_filled(center, radius, egui::Color32::from_rgb(158, 78, 88));
                            }
                            if dot_resp.clicked() {
                                debug_log("dot clicked -> hide to tray");
                                hide_via_win32();
                            }
                        });
                    });
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("macOS-style cursor locator").size(11.0).color(egui::Color32::from_rgb(142, 142, 147)));
                    ui.add_space(10.0);

                    let mut cfg = self.state.cfg.write().unwrap();
                    let mut changed = false;

                    // Sensitivity.
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Sensitivity").size(12.5));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{}", cfg.sensitivity as i32)).strong().color(egui::Color32::from_rgb(10, 132, 255)));
                        });
                    });
                    let r = ui.add(egui::Slider::new(&mut cfg.sensitivity, 1.0..=10.0).show_value(false).smart_aim(false));
                    if r.changed() { changed = true; }
                    ui.label(egui::RichText::new("Higher values trigger with a lighter shake").size(10.0).color(egui::Color32::from_rgb(120, 122, 126)));
                    ui.add_space(10.0);

                    // Max size.
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Maximum Size").size(12.5));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{:.1}x", cfg.magnification)).strong().color(egui::Color32::from_rgb(10, 132, 255)));
                        });
                    });
                    let r = ui.add(egui::Slider::new(&mut cfg.magnification, 2.0..=10.0).show_value(false).smart_aim(false));
                    if r.changed() { changed = true; }
                    ui.label(egui::RichText::new("How large the cursor grows during a vigorous shake").size(10.0).color(egui::Color32::from_rgb(120, 122, 126)));
                    ui.add_space(10.0);

                    // Toggles.
                    let mut en = cfg.enabled;
                    if ui.checkbox(&mut en, "Enabled").changed() {
                        cfg.enabled = en;
                        crate::ENABLED.store(en, Ordering::Relaxed);
                        changed = true;
                    }
                    let mut fs = cfg.disable_fullscreen;
                    if ui.checkbox(&mut fs, "Pause in fullscreen apps").changed() {
                        cfg.disable_fullscreen = fs;
                        changed = true;
                    }
                    let mut su = cfg.run_on_startup;
                    if ui.checkbox(&mut su, "Launch at login").changed() {
                        cfg.run_on_startup = su;
                        crate::startup::set(su);
                        changed = true;
                    }

                    ui.add_space(8.0);

                    // Footer: GitHub mark, 40% idle -> 100% hover.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                        if self.github_tex.is_none() {
                            self.github_tex = load_github_texture(ctx);
                        }
                        let alpha: f32 = if resp.hovered() { 1.0 } else { 0.4 };
                        if let Some(tex) = &self.github_tex {
                            egui::Image::new(tex)
                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                .tint(egui::Color32::from_white_alpha((alpha * 255.0) as u8))
                                .paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0)));
                        }
                        if resp.clicked() {
                            #[cfg(windows)]
                            {
                                use std::os::windows::process::CommandExt;
                                let _ = std::process::Command::new("cmd")
                                    .args(["/C", "start", "", "https://github.com/yfaj/shake-to-find-cursor"])
                                    .creation_flags(0x0800_0000)
                                    .spawn();
                            }
                        }
                    });

                    drop(cfg);

                    if changed {
                        self.state.det.update_settings(&self.state.cfg.read().unwrap());
                        self.state.dirty.store(true, Ordering::Relaxed);
                        let mag = self.state.cfg.read().unwrap().magnification;
                        if (mag - self.last_mag).abs() > 1e-9 {
                            self.last_mag = mag;
                            self.state.rebuild.store(true, Ordering::Relaxed);
                        }
                    }
                });
            });

        if self.want_visible {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

// ---------- logging + win32 helpers ----------

/// Official GitHub mark: PNG embedded at build time (assets/github.png),
/// recolored to white-alpha so it tints cleanly on the dark card.
fn load_github_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    const PNG: &[u8] = include_bytes!("../assets/github.png");
    let img = image::load_from_memory(PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let pixels: Vec<egui::Color32> = img
        .pixels()
        .map(|p| {
            // Source is white-on-transparent; carry alpha through as white.
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, p[3])
        })
        .collect();
    Some(ctx.load_texture(
        "github_mark",
        egui::ColorImage::new([w as usize, h as usize], pixels),
        egui::TextureOptions::LINEAR,
    ))
}

fn chrono_like_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("[{}ms]", d.as_millis())
}

pub fn debug_log(msg: &str) {
    use std::io::Write;
    let Some(temp) = std::env::var_os("TEMP") else {
        return;
    };
    let mut path = std::path::PathBuf::from(temp);
    path.push("stfc_debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{} {}", chrono_like_now(), msg);
    }
}

/// Hide the settings window via Win32 (custom X / glowing dot).
fn hide_via_win32() {
    if let Some(raw) = find_settings_hwnd() {
        let hwnd = windows::Win32::Foundation::HWND(raw as *mut _);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

/// Find the eframe settings window by title within our process.
fn find_settings_hwnd() -> Option<isize> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };
    static mut FOUND: isize = 0;
    unsafe extern "system" fn cb(hwnd: HWND, _l: LPARAM) -> windows::core::BOOL {
        unsafe {
            let mut pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != std::process::id() {
                return windows::core::BOOL::from(true);
            }
            let mut title = [0u16; 128];
            let n = GetWindowTextW(hwnd, &mut title);
            if String::from_utf16_lossy(&title[..n.max(0) as usize]) == "Shake to Find Cursor" {
                FOUND = hwnd.0 as isize;
                return windows::core::BOOL::from(false);
            }
            windows::core::BOOL::from(true)
        }
    }
    unsafe {
        FOUND = 0;
        let _ = EnumWindows(Some(cb), LPARAM(0));
        let f = FOUND;
        if f == 0 { None } else { Some(f) }
    }
}

/// Runs the UI event loop on the MAIN thread. Blocks until app exit.
pub fn run_main_window(state: UiState) {
    let last_mag = state.cfg.read().unwrap().magnification;
    let app = App {
        state,
        last_mag,
        want_visible: false,
        hide_pending: true, // cancel eframe's first-frame auto-show
        github_tex: None,
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 320.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_title("Shake to Find Cursor"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "ShakeToFindSettings",
        opts,
        Box::new(move |_cc| Ok(Box::new(app))),
    );
}
