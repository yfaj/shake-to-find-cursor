# Shake to Find Cursor

macOS-style shake-to-find for Windows. Shake your mouse and the cursor grows into a big, easy-to-spot pointer — so you never lose it again.

Native Rust, single `.exe`, no runtime dependencies.

![platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue) ![size](https://img.shields.io/badge/exe%20size-~3.4%20MB-green)

## Features

- **Shake detection** — oscillation-based detector (reversal counting + wiggle gate), tuned to trigger on a deliberate shake, not accidental movement
- **Smooth animation** — instant attack, eased release; the cursor blooms out and shrinks back naturally
- **HiDPI-sharp** — renders frames from your actual theme cursors up to 15 types × scale 10
- **Fullscreen-aware** — optional pause in fullscreen apps/games, plus an exclusion list
- **Settings UI** — minimal dark window that opens next to the tray: sensitivity, max size, enabled, fullscreen pause, launch at login
- **Tray-first** — left-click opens settings, right-click opens the menu; closing the window keeps the app running
- **Single instance** — launching twice focuses the running app instead
- **Portable** — one executable, zero installs

## Install

1. Grab `shake-to-find-cursor.exe` from [Releases](https://github.com/yfaj/shake-to-find-cursor/releases)
2. Run it — it lives in your tray
3. Optional: check *Launch at login* in settings

## Build from source

Requires the Rust toolchain ([rustup.rs](https://rustup.rs)).

```bash
cargo build --release
```

Output: `target/release/shake-to-find-cursor.exe`

## Settings

Stored in `%APPDATA%\ShakeToFindCursor\settings.cfg`:

| Key | Values | Meaning |
|---|---|---|
| `sensitivity` | 1–10 | Higher triggers with a lighter shake |
| `magnification` | 2–10 | How large the cursor grows |
| `disable_fullscreen` | 0/1 | Pause detection in fullscreen apps |
| `run_on_startup` | 0/1 | Launch at login |
| `enabled` | 0/1 | Master on/off |
| `excluded` | `app1.exe;app2.exe` | Semicolon-separated process names to ignore |

## Why native?

The original [shake-to-find-cursor](https://github.com/nowonderwhyy/shake-to-find-cursor) is C# and needs the .NET runtime (~200 MB self-contained). This rewrite is pure Rust + `windows-rs`: same behavior, 3.4 MB, nothing to install.

## License

[MIT](LICENSE)
