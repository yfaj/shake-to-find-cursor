use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, GWL_EXSTYLE,
    GWL_STYLE,
};

const WS_CAPTION: u32 = 0x00C0_0000;
const WS_THICKFRAME: u32 = 0x0004_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_POPUP: u32 = 0x8000_0000;

pub fn should_disable(excluded: &[String], disable_fullscreen: bool) -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }

        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if let Some(name) = process_name(pid) {
            if excluded.iter().any(|e| e == &name) {
                return true;
            }
        }

        if disable_fullscreen && is_fullscreen(hwnd) {
            return true;
        }
        false
    }
}

fn process_name(pid: u32) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let name = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .ok()
        .map(|_| {
            let s = String::from_utf16_lossy(&buf[..size as usize]);
            s.rsplit(['\\', '/'])
                .next()
                .unwrap_or("")
                .trim_end_matches(".exe")
                .to_ascii_lowercase()
        });
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        name
    }
}

fn is_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        let mut rect = Default::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let hmon: HMONITOR = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if hmon.is_invalid() {
            return false;
        }
        let mut mi = MONITORINFO::default();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return false;
        }
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let _ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

        let has_caption = (style & WS_CAPTION) == WS_CAPTION;
        let has_border = (style & (WS_BORDER | WS_THICKFRAME)) != 0;
        let is_popup = (style & WS_POPUP) != 0;

        let covers_full = rect.left <= mi.rcMonitor.left
            && rect.top <= mi.rcMonitor.top
            && rect.right >= mi.rcMonitor.right
            && rect.bottom >= mi.rcMonitor.bottom;

        let covers_work = rect.left <= mi.rcWork.left
            && rect.top <= mi.rcWork.top
            && rect.right >= mi.rcWork.right
            && rect.bottom >= mi.rcWork.bottom;

        if covers_full && (!has_caption || is_popup) {
            return true;
        }
        if covers_full && !has_border && !has_caption {
            return true;
        }
        if covers_full && !covers_work {
            return true;
        }
        false
    }
}
