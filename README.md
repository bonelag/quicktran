# QuickTran

System-wide quick translator (Tauri 2 + React). Select text in any app, press a
global hotkey, preview the translation, and one-click overwrite the source field.

## How it works

1. App runs in the background with a tray icon (window hidden).
2. Select text anywhere → press **Ctrl+Shift+T**.
3. Rust grabs the focused window, sends `Ctrl+C`, reads the clipboard, then shows
   the preview window with the captured text auto-translated.
4. Pick a target language from the dropdown (re-translates live).
5. Click **Overwrite** → translation is pasted back into the original field
   (`Ctrl+V` into the remembered window). **Copy** copies it; **Close** / `Esc`
   hides the window (app stays in the tray).

Translation uses Google's free `translate.googleapis.com` gtx endpoint
(no API key, source auto-detected). Called from Rust to avoid browser CORS.

## Dev

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

## Notes

- Windows only — focus capture/restore uses Win32 `GetForegroundWindow` /
  `SetForegroundWindow`.
- The Google endpoint is unofficial and may rate-limit; errors surface in the
  translation box.
- The hotkey clobbers the clipboard with the selected text (no save/restore yet).
- Synthetic keystrokes (`enigo`) may be flagged by aggressive AV/anti-cheat.
- Change the hotkey or languages in `src-tauri/src/lib.rs` (`Code::KeyT`) and
  `src/App.tsx` (`LANGS`).
