use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    /// 1..=10, how easily a shake triggers.
    pub sensitivity: f64,
    /// 2..=10, max cursor magnification.
    pub magnification: f64,
    /// Pause in fullscreen apps.
    pub disable_fullscreen: bool,
    /// Process names (lowercase) never to activate in.
    pub excluded: Vec<String>,
    /// Launch at login via HKCU Run key.
    pub run_on_startup: bool,
    /// Shake detection enabled (tray/UI toggle).
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 5.0,
            magnification: 4.0,
            disable_fullscreen: true,
            excluded: Vec::new(),
            run_on_startup: false,
            enabled: true,
        }
    }
}

pub fn app_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("ShakeToFindCursor")
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(path) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "sensitivity" => s.sensitivity = v.parse().unwrap_or(s.sensitivity),
                "magnification" => s.magnification = v.parse().unwrap_or(s.magnification),
                "disable_fullscreen" => s.disable_fullscreen = v == "true",
                "run_on_startup" => s.run_on_startup = v == "true",
                "enabled" => s.enabled = v == "true",
                "excluded" => {
                    s.excluded = v
                        .split(';')
                        .filter(|p| !p.is_empty())
                        .map(|p| p.to_ascii_lowercase())
                        .collect();
                }
                _ => {}
            }
        }
        s.sensitivity = s.sensitivity.clamp(1.0, 10.0);
        s.magnification = s.magnification.clamp(2.0, 10.0);
        s
    }

    pub fn save(&self, path: &PathBuf) {
        let excluded = self.excluded.join(";");
        let text = format!(
            "sensitivity={}\nmagnification={}\ndisable_fullscreen={}\nrun_on_startup={}\nenabled={}\nexcluded={}\n",
            self.sensitivity, self.magnification, self.disable_fullscreen, self.run_on_startup, self.enabled, excluded
        );
        let _ = std::fs::write(path, text);
    }
}
