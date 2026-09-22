use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, DestroyIcon, LoadCursorW, LoadImageW, SetSystemCursor, SystemParametersInfoW,
    HCURSOR, HICON, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO,
    IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_UPARROW, IDC_WAIT,
    IMAGE_CURSOR, LR_LOADFROMFILE, SPI_SETCURSORS, SYSTEM_CURSOR_ID,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

pub const SCALE_STEPS: usize = 64;

struct Entry {
    id: u32,
    reg_name: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry { id: 32512, reg_name: "Arrow" },
    Entry { id: 32513, reg_name: "IBeam" },
    Entry { id: 32514, reg_name: "Wait" },
    Entry { id: 32515, reg_name: "Crosshair" },
    Entry { id: 32516, reg_name: "UpArrow" },
    Entry { id: 32640, reg_name: "SizeAll" },
    Entry { id: 32642, reg_name: "SizeNWSE" },
    Entry { id: 32643, reg_name: "SizeNESW" },
    Entry { id: 32644, reg_name: "SizeWE" },
    Entry { id: 32645, reg_name: "SizeNS" },
    Entry { id: 32646, reg_name: "SizeAll" },
    Entry { id: 32648, reg_name: "No" },
    Entry { id: 32649, reg_name: "Hand" },
    Entry { id: 32650, reg_name: "AppStarting" },
    Entry { id: 32651, reg_name: "Help" },
];

fn stock_cursor(id: u32) -> HCURSOR {
    unsafe {
        let idc = match id {
            32512 => IDC_ARROW,
            32513 => IDC_IBEAM,
            32514 => IDC_WAIT,
            32515 => IDC_CROSS,
            32516 => IDC_UPARROW,
            32640 | 32646 => IDC_SIZEALL,
            32642 => IDC_SIZENWSE,
            32643 => IDC_SIZENESW,
            32644 => IDC_SIZEWE,
            32645 => IDC_SIZENS,
            32648 => IDC_NO,
            32649 => IDC_HAND,
            32650 => IDC_APPSTARTING,
            32651 => IDC_HELP,
            _ => IDC_ARROW,
        };
        LoadCursorW(None, idc).unwrap_or_default()
    }
}

static CACHED: AtomicBool = AtomicBool::new(false);
static LAST_FACTOR_BITS: AtomicU64 = AtomicU64::new(u64::MAX);

struct Cache {
    // cursor id -> [handle; SCALE_STEPS]
    map: HashMap<u32, Vec<isize>>,
    scales: Vec<f64>,
}

static FRAMES: Mutex<Option<Cache>> = Mutex::new(None);

pub fn is_cached() -> bool {
    CACHED.load(Ordering::Relaxed)
}

pub fn init_caches(magnification: f64) {
    let last = f64::from_bits(LAST_FACTOR_BITS.load(Ordering::Relaxed));
    if CACHED.load(Ordering::Relaxed) && (last - magnification).abs() < 1e-3 {
        return;
    }

    // Non-linear scale table: power bias packs more frames near 1.0.
    let peak = magnification * 1.05;
    let scales: Vec<f64> = (0..SCALE_STEPS)
        .map(|i| {
            let t = i as f64 / (SCALE_STEPS - 1) as f64;
            1.0 + (peak - 1.0) * t.powf(1.8)
        })
        .collect();

    let theme_paths = read_theme_paths();

    let mut map: HashMap<u32, Vec<isize>> = HashMap::new();
    for e in ENTRIES {
        let path: Option<&str> = theme_paths.get(e.reg_name).and_then(|o| o.as_deref());
        let fallback = stock_cursor(e.id);
        let mut frames = Vec::with_capacity(SCALE_STEPS);
        for &scale in &scales {
            let (h, _) = generate_frame(path, fallback, scale);
            frames.push(h.0 as isize);
        }
        map.insert(e.id, frames);
    }

    let mut guard = FRAMES.lock().unwrap();
    if let Some(old) = guard.take() {
        for frames in old.map.values() {
            for &h in frames {
                if h != 0 {
                    unsafe {
                        let _ = DestroyIcon(HICON(h as *mut _));
                    }
                }
            }
        }
    }
    *guard = Some(Cache { map, scales });
    drop(guard);

    LAST_FACTOR_BITS.store(magnification.to_bits(), Ordering::Relaxed);
    CACHED.store(true, Ordering::Relaxed);
}

fn read_theme_paths() -> HashMap<&'static str, Option<String>> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };
    let mut out: HashMap<&'static str, Option<String>> = HashMap::new();
    unsafe {
        let mut key = HKEY::default();
        let subkey: Vec<u16> = "Control Panel\\Cursors\0".encode_utf16().collect();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut key)
            .is_ok()
        {
            for e in ENTRIES {
                let name: Vec<u16> = format!("{}\0", e.reg_name).encode_utf16().collect();
                let mut buf = [0u16; 512];
                let mut size = (buf.len() * 2) as u32;
                let val = RegQueryValueExW(
                    key,
                    PCWSTR(name.as_ptr()),
                    None,
                    None,
                    Some(buf.as_mut_ptr() as *mut u8),
                    Some(&mut size),
                )
                .ok()
                .map(|_| {
                    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    String::from_utf16_lossy(&buf[..len])
                });
                let val = match val {
                    Ok(s) => Some(s),
                    Err(_) => None,
                };
                out.insert(e.reg_name, val);
            }
            let _ = RegCloseKey(key);
        }
    }
    out
}

fn generate_frame(path: Option<&str>, fallback: HCURSOR, scale: f64) -> (HCURSOR, bool) {
    if let Some(p) = path {
        let base = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CXCURSOR,
        ) };
        let base = if base == 0 { 32 } else { base };
        let ts = (base as f64 * scale).max(1.0) as i32;
        let wide: Vec<u16> = p.encode_utf16().chain(std::iter::once(0)).collect();
        let h = unsafe {
            LoadImageW(
                None,
                PCWSTR(wide.as_ptr()),
                IMAGE_CURSOR,
                ts,
                ts,
                LR_LOADFROMFILE,
            )
        };
        if let Ok(h) = h {
            return (HCURSOR(h.0), true);
        }
    }
    if !fallback.is_invalid() {
        if let Some(h) = magnify_fallback(fallback) {
            return (h, true);
        }
    }
    (HCURSOR(std::ptr::null_mut()), false)
}

/// Fallback: reuse a copy of the stock cursor unchanged. Rare path (only when the
/// theme cursor file cannot be loaded); Windows scales it poorly but keeps cursor
/// identity consistent across frames.
fn magnify_fallback(h: HCURSOR) -> Option<HCURSOR> {
    let copy = unsafe { CopyIcon(HICON(h.0)) }.ok()?;
    Some(HCURSOR(copy.0))
}

pub fn frame_index_for_scale(scale: f64) -> i32 {
    let guard = FRAMES.lock().unwrap();
    let Some(cache) = guard.as_ref() else { return 0 };
    let scales = &cache.scales;
    if scales.is_empty() {
        return 0;
    }
    let s = scale.clamp(scales[0], scales[scales.len() - 1]);
    match scales
        .binary_search_by(|p| p.partial_cmp(&s).expect("NaN in scales"))
    {
        Ok(i) => i as i32,
        Err(i) => {
            if i == 0 {
                0
            } else if i >= scales.len() {
                (scales.len() - 1) as i32
            } else {
                let before = scales[i - 1];
                let after = scales[i];
                if (before - s).abs() <= (after - s).abs() {
                    (i - 1) as i32
                } else {
                    i as i32
                }
            }
        }
    }
}

pub fn apply_scale_frame(frame: i32) {
    let guard = FRAMES.lock().unwrap();
    let Some(cache) = guard.as_ref() else { return };
    let frame = frame.clamp(0, (SCALE_STEPS - 1) as i32) as usize;
    unsafe {
        for e in ENTRIES {
            let Some(frames) = cache.map.get(&e.id) else { continue };
            if frame >= frames.len() {
                continue;
            }
            let h = frames[frame];
            if h == 0 {
                continue;
            }
            // SetSystemCursor takes ownership on success; copy so cache keeps its handle.
            if let Ok(copy) = CopyIcon(HICON(h as *mut _)) {
                if SetSystemCursor(HCURSOR(copy.0), SYSTEM_CURSOR_ID(e.id)).is_err() {
                    let _ = DestroyIcon(copy);
                }
            }
        }
    }
}

pub fn restore_theme_cursors() {
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETCURSORS,
            0,
            None,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
}
