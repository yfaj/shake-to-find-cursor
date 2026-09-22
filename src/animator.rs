use crate::settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

const FOLLOW_TAU_MS: f64 = 55.0;
/// Separate (slower) easing for shrink so the cursor lingers macOS-brief but visible.
const SHRINK_TAU_MS: f64 = 160.0;

/// Animation loop. Runs forever; idles on a 4 ms poll until woken by the hook thread.
pub fn run(det: Arc<crate::detector::Detector>, cfg: Arc<RwLock<Settings>>, wake: Arc<AtomicBool>) {
    let mut current: f64 = 1.0;
    let mut last_frame: i32 = -1;
    let mut last_ticks = std::time::Instant::now();

    loop {
        if !wake.swap(false, Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(4));
            // Keep the clock moving so the first frame after wake has sane dt.
            last_ticks = std::time::Instant::now();
            continue;
        }

        loop {
            let now = std::time::Instant::now();
            let mut dt = (now - last_ticks).as_secs_f64();
            last_ticks = now;
            if dt <= 0.0 {
                dt = 1.0 / 144.0;
            }
            if dt > 0.05 {
                dt = 0.05;
            }

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let energy = det.tick(now_ms);
            let max = cfg.read().unwrap().magnification.max(1.0);

            let target = 1.0 + (max - 1.0) * energy;
            // Asymmetric easing: fast grow (snappy), slower shrink (lingers briefly).
            let tau = if target >= current { FOLLOW_TAU_MS } else { SHRINK_TAU_MS };
            let alpha = 1.0 - (-dt / (tau / 1000.0)).exp();
            current += (target - current) * alpha;
            if current < 1.0 {
                current = 1.0;
            }

            let done = energy <= 0.0 && (current - 1.0).abs() < 0.01;

            let frame = crate::cursor_helper::frame_index_for_scale(current);
            if frame != last_frame {
                last_frame = frame;
                crate::cursor_helper::apply_scale_frame(frame);
            }

            if done {
                crate::cursor_helper::apply_scale_frame(0);
                std::thread::sleep(std::time::Duration::from_millis(10));
                crate::cursor_helper::restore_theme_cursors();
                last_frame = -1;
                current = 1.0;
                break;
            }

            // ~7 ms frame pacing (~143 fps): sleep in 1 ms chunks; good enough here
            // because Windows timers + cursor swaps dominate anyway.
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
