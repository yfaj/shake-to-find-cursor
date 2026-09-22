use crate::settings::Settings;
use std::sync::Mutex;

const WINDOW_MS: i64 = 350;
const WIGGLE_GATE: f64 = 0.5;
const MIN_REVERSALS: usize = 2;
const RELEASE_TAU_MS: f64 = 320.0;

#[derive(Clone, Copy)]
struct Sample {
    x: i32,
    y: i32,
    t: i64,
}

struct Inner {
    history: Vec<Sample>,
    energy: f64,
    last_update: i64,
    suppressed: bool,
    trigger_path: f64,
    path_for_full: f64,
}

pub struct Detector {
    inner: Mutex<Inner>,
}

impl Detector {
    pub fn new(cfg: &Settings) -> Self {
        let (trigger_path, path_for_full) = Self::derive_paths(cfg.sensitivity);
        Self {
            inner: Mutex::new(Inner {
                history: Vec::with_capacity(64),
                energy: 0.0,
                last_update: 0,
                suppressed: false,
                trigger_path,
                path_for_full,
            }),
        }
    }

    pub fn update_settings(&self, cfg: &Settings) {
        let mut g = self.inner.lock().unwrap();
        let (tp, pf) = Self::derive_paths(cfg.sensitivity);
        g.trigger_path = tp;
        g.path_for_full = pf;
    }

    fn derive_paths(sensitivity: f64) -> (f64, f64) {
        // Higher sensitivity -> less pointer travel needed to trigger and reach full size.
        // Tuned for high-DPI mice (6400+): a light wrist wiggle should trigger at default 5.
        let t = ((sensitivity.clamp(1.0, 10.0) - 1.0) / 9.0) as f64; // 0 hard .. 1 easy
        let trigger = 1000.0 - (750.0 * t); // 1000 .. 250 px within the window
        (trigger, trigger * 2.0)
    }

    pub fn set_suppressed(&self, suppressed: bool) {
        let mut g = self.inner.lock().unwrap();
        g.suppressed = suppressed;
        if suppressed {
            g.energy = 0.0;
            g.history.clear();
        }
    }

    pub fn energy(&self) -> f64 {
        self.inner.lock().unwrap().energy
    }

    pub fn add_sample(&self, x: i32, y: i32, now: i64) {
        let mut g = self.inner.lock().unwrap();
        if g.suppressed {
            return;
        }
        g.history.push(Sample { x, y, t: now });
        while !g.history.is_empty() && now - g.history[0].t > WINDOW_MS {
            g.history.remove(0);
        }
        Self::decay(&mut g, now);
        let instant = Self::compute_instant(&g);
        if instant > g.energy {
            g.energy = instant; // attack instantly; the animator eases the visual
        }
    }

    /// Advances the decay envelope on the animator's clock; returns current energy.
    pub fn tick(&self, now: i64) -> f64 {
        let mut g = self.inner.lock().unwrap();
        Self::decay(&mut g, now);
        g.energy
    }

    fn decay(g: &mut Inner, now: i64) {
        let dt = now - g.last_update;
        g.last_update = now;
        if dt <= 0 {
            return;
        }
        g.energy *= (-dt as f64 / RELEASE_TAU_MS).exp();
        if g.energy < 1e-3 {
            g.energy = 0.0;
        }
    }

    fn compute_instant(g: &Inner) -> f64 {
        if g.history.len() < 5 {
            return 0.0;
        }
        let h = &g.history;
        let mut total_path = 0.0f64;
        let mut reversals = 0usize;
        let mut prev_dx: i64 = 0;
        let mut prev_dy: i64 = 0;
        let mut have_prev_seg = false;
        let (first, last) = (h[0], h[h.len() - 1]);

        for w in h.windows(2) {
            let dx = (w[1].x - w[0].x) as i64;
            let dy = (w[1].y - w[0].y) as i64;
            total_path += ((dx * dx + dy * dy) as f64).sqrt();
            if have_prev_seg && (dx * prev_dx + dy * prev_dy) < 0 {
                reversals += 1;
            }
            if dx != 0 || dy != 0 {
                prev_dx = dx;
                prev_dy = dy;
                have_prev_seg = true;
            }
        }

        if total_path < g.trigger_path {
            return 0.0;
        }
        if reversals < MIN_REVERSALS {
            return 0.0;
        }
        let net = dist(first, last);
        let wiggle = if total_path > 0.0 { 1.0 - net / total_path } else { 0.0 };
        if wiggle < WIGGLE_GATE {
            return 0.0;
        }
        ((total_path - g.trigger_path) / (g.path_for_full - g.trigger_path)).clamp(0.0, 1.0)
    }
}

fn dist(a: Sample, b: Sample) -> f64 {
    let dx = (a.x - b.x) as i64;
    let dy = (a.y - b.y) as i64;
    ((dx * dx + dy * dy) as f64).sqrt()
}
