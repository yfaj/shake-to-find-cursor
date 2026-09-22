use crate::cursor_helper;
use crate::detector::Detector;
use crate::fullscreen;
use crate::settings::Settings;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, HHOOK, MSG, WH_MOUSE_LL,
    WM_QUIT,
};

const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;

const BTN_LEFT: i32 = 1;
const BTN_RIGHT: i32 = 2;
const BTN_MIDDLE: i32 = 4;

static BUTTON_MASK: AtomicI32 = AtomicI32::new(0);
static HOOK_PTR: AtomicUsize = AtomicUsize::new(0);
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
#[repr(C)]
struct MsllHookStruct {
    pt: POINT,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: isize,
}

pub fn is_button_down() -> bool {
    BUTTON_MASK.load(Ordering::Relaxed) != 0
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

struct FsCache {
    last_check: i64,
    should_disable: bool,
}

struct HookState {
    cfg: Arc<RwLock<Settings>>,
    det: Arc<Detector>,
    wake: Arc<AtomicBool>,
    fs: FsCache,
}

// Hook proc must be a plain fn pointer; state lives in a static set before hook install.
// Access is safe: only this hook thread touches it after init, before GetMessage loop.
static mut STATE: Option<HookState> = None;

fn handle_move(state: &mut HookState, x: i32, y: i32) {
    if !crate::ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let now = now_ms();
    if now - state.fs.last_check >= 300 {
        state.fs.last_check = now;
        let (excl, dis_fs) = {
            let c = state.cfg.read().unwrap();
            (c.excluded.clone(), c.disable_fullscreen)
        };
        state.fs.should_disable = fullscreen::should_disable(&excl, dis_fs);
    }
    state.det.set_suppressed(state.fs.should_disable || is_button_down());
    state.det.add_sample(x, y, now);
    if state.det.energy() > 0.0 && cursor_helper::is_cached() {
        state.wake.store(true, Ordering::Relaxed);
    }
}

pub fn install_and_pump(cfg: Arc<RwLock<Settings>>, det: Arc<Detector>, wake: Arc<AtomicBool>) {
    unsafe {
        THREAD_ID.store(GetCurrentThreadId(), Ordering::Relaxed);

        STATE = Some(HookState {
            cfg,
            det,
            wake,
            fs: FsCache { last_check: 0, should_disable: false },
        });

        unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
            unsafe {
                if code >= 0 {
                    let state = (&raw mut STATE).as_mut().unwrap().as_mut().unwrap();
                    let msg = wparam.0 as u32;
                    match msg {
                        WM_MOUSEMOVE => {
                            let hs = *(lparam.0 as *const MsllHookStruct);
                            handle_move(state, hs.pt.x, hs.pt.y);
                        }
                        WM_LBUTTONDOWN => {
                            BUTTON_MASK.fetch_or(BTN_LEFT, Ordering::Relaxed);
                        }
                        WM_LBUTTONUP => {
                            BUTTON_MASK.fetch_and(!BTN_LEFT, Ordering::Relaxed);
                        }
                        WM_RBUTTONDOWN => {
                            BUTTON_MASK.fetch_or(BTN_RIGHT, Ordering::Relaxed);
                        }
                        WM_RBUTTONUP => {
                            BUTTON_MASK.fetch_and(!BTN_RIGHT, Ordering::Relaxed);
                        }
                        WM_MBUTTONDOWN => {
                            BUTTON_MASK.fetch_or(BTN_MIDDLE, Ordering::Relaxed);
                        }
                        WM_MBUTTONUP => {
                            BUTTON_MASK.fetch_and(!BTN_MIDDLE, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                }
                CallNextHookEx(
                    Some(HHOOK(HOOK_PTR.load(Ordering::Relaxed) as *mut _)),
                    code,
                    wparam,
                    lparam,
                )
            }
        }

        let hmod = GetModuleHandleW(None).unwrap_or_default();
        if let Ok(h) = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(hook_proc),
            Some(windows::Win32::Foundation::HINSTANCE(hmod.0)),
            0,
        ) {
            HOOK_PTR.store(h.0 as usize, Ordering::Relaxed);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // No translate/dispatch needed: we only care about the hook callbacks + WM_QUIT.
        }
    }
}

pub fn post_quit() {
    unsafe {
        let _ = PostThreadMessageW(THREAD_ID.load(Ordering::Relaxed), WM_QUIT, WPARAM(0), LPARAM(0));
    }
}
