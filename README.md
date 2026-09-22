# Shake to Find Cursor

Shake your mouse and the cursor grows big so you can find it. For Windows.

Native Rust, single `.exe` (~3.4 MB), no runtime dependencies.

## Install

Grab `shake-to-find-cursor.exe` from [Releases](https://github.com/yfaj/shake-to-find-cursor/releases) and run it. It lives in your tray.

- Left-click tray: settings
- Right-click tray: menu (open settings / disable / exit)
- Closing the settings window keeps the app running; exit only from the tray menu

## Build

```bash
cargo build --release
```

## Settings

Left-click the tray icon. Sensitivity (1–10, higher = lighter shake triggers) and max cursor size (2–10x) are there, plus toggles for fullscreen pause, launch at login, and enable/disable.

Settings are saved to `%APPDATA%\ShakeToFindCursor\settings.cfg` and restored on launch. Only one instance can run; launching again just opens the settings window.

## Credits

Based on the behavior of [shake-to-find-cursor](https://github.com/nowonderwhyy/shake-to-find-cursor) (C#), rewritten in Rust to drop the .NET runtime.

## License

[MIT](LICENSE)
