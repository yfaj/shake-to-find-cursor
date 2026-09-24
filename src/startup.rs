// Launch-at-login via HKCU\...\Run registry key.

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    REG_OPEN_CREATE_OPTIONS,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "ShakeToFindCursor";

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn set(enable: bool) {
    let Ok(exe) = std::env::current_exe() else { return };
    let cmd = format!("\"{}\"", exe.to_string_lossy());
    unsafe {
        let mut hkey = HKEY::default();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(utf16(RUN_KEY).as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut hkey,
            None,
        );
        if status.is_err() {
            return;
        }
        if enable {
            let data = utf16(&cmd);
            let bytes: &[u8] = std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * 2,
            );
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(utf16(VALUE_NAME).as_ptr()),
                None,
                REG_SZ,
                Some(bytes),
            );
        } else {
            let _ = RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(utf16(RUN_KEY).as_ptr()),
                PCWSTR(utf16(VALUE_NAME).as_ptr()),
            );
        }
        let _ = RegCloseKey(hkey);
    }
}
