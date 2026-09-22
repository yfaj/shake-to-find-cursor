//! Single-instance guard via named Win32 mutex + a named event used to ask the
//! running instance to open its settings window.
//!
//! Raw FFI via windows-link to avoid extra windows-rs feature gates; HANDLE bits
//! stored as usize in atomics.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use windows::core::PCWSTR;

windows_link::link!("kernel32.dll" "system" fn CreateMutexW(lpmutexattributes: *mut core::ffi::c_void, binitialowner: i32, lpname: PCWSTR) -> isize);
windows_link::link!("kernel32.dll" "system" fn OpenMutexW(dwdesiredaccess: u32, binheritflag: i32, lpname: PCWSTR) -> isize);
windows_link::link!("kernel32.dll" "system" fn CreateEventW(lpeventattributes: *mut core::ffi::c_void, bmanualreset: i32, binitialstate: i32, lpname: PCWSTR) -> isize);
windows_link::link!("kernel32.dll" "system" fn OpenEventW(dwdesiredaccess: u32, binheritflag: i32, lpname: PCWSTR) -> isize);
windows_link::link!("kernel32.dll" "system" fn SetEvent(hevent: isize) -> i32);
windows_link::link!("kernel32.dll" "system" fn WaitForSingleObject(hhandle: isize, dwmilliseconds: u32) -> u32);
windows_link::link!("kernel32.dll" "system" fn CloseHandle(hobject: isize) -> i32);

const MUTEX_NAME: &str = "ShakeToFindCursor_SingleInstance_Mutex";
const EVENT_NAME: &str = "ShakeToFindCursor_ShowSettings_Event";

static MUTEX_HANDLE: AtomicUsize = AtomicUsize::new(0);
static EVENT_HANDLE: AtomicUsize = AtomicUsize::new(0);
static LISTENING: AtomicBool = AtomicBool::new(false);

const WAIT_OBJECT_0: u32 = 0;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const SYNCHRONIZE: u32 = 0x0010_0000;
const MUTEX_ALL_ACCESS: u32 = 0x001F_0001;

fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Returns true if we are the first instance (mutex acquired and owned).
pub fn acquire() -> bool {
    let name = wide(MUTEX_NAME);
    unsafe {
        let existing = OpenMutexW(MUTEX_ALL_ACCESS, 0, PCWSTR(name.as_ptr()));
        if existing != 0 {
            CloseHandle(existing);
            return false; // already running
        }
        // Create owned mutex.
        let h = CreateMutexW(std::ptr::null_mut(), 1, PCWSTR(name.as_ptr()));
        if h == 0 {
            return false;
        }
        MUTEX_HANDLE.store(h as usize, Ordering::Relaxed);
        true
    }
}

/// Ask the running instance to show its settings window.
pub fn signal_show() {
    let name = wide(EVENT_NAME);
    unsafe {
        let mut h = OpenEventW(EVENT_MODIFY_STATE, 0, PCWSTR(name.as_ptr()));
        if h == 0 {
            // First run created it via listener; fall back to create+set.
            h = CreateEventW(std::ptr::null_mut(), 0, 0, PCWSTR(name.as_ptr()));
        }
        if h != 0 {
            SetEvent(h);
            CloseHandle(h);
        }
    }
}

/// Spawn a thread that watches the named auto-reset event and raises SHOW_REQUEST.
/// Also creates the event so late second-launchers can signal it.
pub fn spawn_show_listener() {
    let name = wide(EVENT_NAME);
    unsafe {
        let h = CreateEventW(std::ptr::null_mut(), 0, 0, PCWSTR(name.as_ptr()));
        if h == 0 {
            return;
        }
        EVENT_HANDLE.store(h as usize, Ordering::Relaxed);
    }
    LISTENING.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let h = EVENT_HANDLE.load(Ordering::Relaxed) as isize;
        loop {
            let r = unsafe { WaitForSingleObject(h, 1000) };
            if r == WAIT_OBJECT_0 {
                crate::ui::SHOW_REQUEST.store(true, Ordering::Relaxed);
            }
            if !LISTENING.load(Ordering::Relaxed) {
                break;
            }
        }
    });
}
