use crate::settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging as win;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetMessageW, RegisterClassW, TrackPopupMenu, HMENU, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TPM_RETURNCMD, WM_APP, WM_COMMAND, WM_RBUTTONUP, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};

const WM_TRAYICON: u32 = WM_APP + 1;
const IDI_TRAY: u32 = 1;
const CMD_OPEN: usize = 2001;
const CMD_TOGGLE: usize = 2002;
const CMD_EXIT: usize = 2003;

pub struct TrayDeps {
    pub cfg: Arc<RwLock<Settings>>,
    pub dirty: Arc<AtomicBool>,
    pub det: Arc<crate::detector::Detector>,
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

thread_local! {
    static DEPS: std::cell::RefCell<Option<TrayDeps>> = const { std::cell::RefCell::new(None) };
}

pub fn run(deps: TrayDeps) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();

        let class_name = utf16("ShakeToFindTray");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wndproc),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hInstance: hinstance.into(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            0, 0, 0, 0,
            None, None,
            Some(windows::Win32::Foundation::HINSTANCE(hinstance.0)),
            None,
        )
        .expect("tray window");

        DEPS.with(|d| *d.borrow_mut() = Some(deps));

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = IDI_TRAY;
        nid.uFlags = NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        let tip = utf16("Shake to Find Cursor");
        nid.szTip[..tip.len()].copy_from_slice(&tip);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let mut msg = win::MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = win::TranslateMessage(&msg);
            win::DispatchMessageW(&msg);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAYICON if (lparam.0 as u32) == win::WM_LBUTTONUP => {
                crate::ui::request_show();
                LRESULT(0)
            }
            WM_TRAYICON if (lparam.0 as u32) == WM_RBUTTONUP => {
                show_menu(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                match wparam.0 as usize {
                    CMD_OPEN => {
                        crate::ui::request_show();
                    }
                    CMD_TOGGLE => {
                        let e = !crate::ENABLED.load(Ordering::Relaxed);
                        crate::ENABLED.store(e, Ordering::Relaxed);
                    }
                    CMD_EXIT => {
                        crate::hook::post_quit();
                        DestroyWindow(hwnd).ok();
                        std::process::exit(0);
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let menu: HMENU = CreatePopupMenu().expect("popup menu");
        let enabled = crate::ENABLED.load(Ordering::Relaxed);

        let open_t = utf16("Open Settings");
        AppendMenuW(menu, Default::default(), CMD_OPEN, PCWSTR(open_t.as_ptr())).ok();
        let toggle_t = utf16(if enabled { "Disable" } else { "Enable" });
        AppendMenuW(menu, Default::default(), CMD_TOGGLE, PCWSTR(toggle_t.as_ptr())).ok();
        let exit_t = utf16("Exit");
        AppendMenuW(menu, Default::default(), CMD_EXIT, PCWSTR(exit_t.as_ptr())).ok();

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
            pt.x, pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        if cmd.0 != 0 {
            tray_wndproc(hwnd, WM_COMMAND, WPARAM(cmd.0 as usize), LPARAM(0));
        }
    }
}
