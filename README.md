# QuickTran

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078d7.svg)](https://microsoft.com/windows)
[![Built with: Tauri](https://img.shields.io/badge/Built%20with-Tauri%202.0-yellow.svg)](https://tauri.app/)

QuickTran is a lightweight, system-wide translation utility for Windows. Running as a tray application, it allows you to highlight text in any application (web browsers, editors, document viewers, PDF readers, Discord) and translate it instantly using global keyboard shortcuts.

---

## Key Features

* **Instant Translation Overlay**: Highlight text anywhere and press your global hotkey to display a borderless, transparent translation popup at the bottom-right corner of your screen.
* **Non-Intrusive Tooltip**: The tooltip window is non-focusable (`focusable: false`), meaning it behaves as a true overlay and never steals focus from your active application (e.g., Discord or code editors).
* **Inline Language Override Tags**: Temporarily override the configured translation target directly in the text you select. Simply prefix or suffix the text with tags like `<en`, `<vi`, or `<ja` (e.g. `hello <vi` or `<ja hello`). QuickTran dynamically reads, strips, and routes the translation to that target language.
* **Dual-Engine Architecture**:
  * **Google Translate (Free)**: Works out-of-the-box using the free gtx translate API (no setup or credentials required), routed securely through the Rust backend.
  * **Custom AI Engine**: Connect any OpenAI-compatible API (DeepSeek, OpenAI, Mistral, Groq, Google Gemini) for advanced context-aware translation with custom prompts.
* **Smart Auto-Paste**: Optional feature that auto-pastes the translated text back into the source field and hides the window in the background (triggered via the main hotkey).
* **Custom Hotkey Capturing**: Easily register separate keyboard shortcuts for the main window and tooltip overlay via modifier selectors and an interactive keystroke listener.
* **Auto-Hide Sleep Timer**: Configure a duration (in seconds) to automatically close/hide the translation tooltip after inactivity (default is 10s, set to 0 to keep open).
* **Single-Instance Protection**: Prevents multiple processes of the app from running concurrently, ensuring focus is routed back to the active instance.

---

## How to Use

### 1. Highlight and Translate (Tooltip)
Highlight text in any application (e.g. Discord chat, PDF, browser) and press your assigned global tooltip hotkey (default: `Alt + D`):
* A non-focusable tooltip appears at the bottom right containing the translation.
* Press `Escape` or wait for the auto-hide timer to dismiss it.
* You can click `✕` to close it immediately.

### 2. Translate & Overwrite (Main Window)
Press the main global shortcut (default: `Alt + T`):
* Opens the main translation window.
* If "Auto-paste translation back" is enabled in settings, the highlighted text is automatically replaced by its translation in the background.

### 3. Dynamic Target Tagging
QuickTran scans the selected text for tags formatting like `<??` (where `??` is a two-letter country code).
* *Example:* Copying `"Hello World <vi"` translates the text to Vietnamese and outputs `"Xin chào thế giới"`.
* *Example:* Copying `"<ja How are you?"` translates the text to Japanese and outputs `"お元気ですか？"`.

---

## Development & Build

### Prerequisites
* [Node.js](https://nodejs.org/) (v20 or higher)
* [Rust Compiler](https://www.rust-lang.org/) (Stable)
* Windows C++ Build Tools (required by Cargo for compiling Tauri dependencies)

### Local Dev Server
Install frontend and Rust dependencies, then start the hot-reloading dev server:
```bash
npm install
npm run tauri dev
```

### Compile Production Binary
Builds the highly optimized release bundle and outputs a single, portable executable file:
```bash
npm run tauri build
```
The compiled executable is saved to:
`src-tauri/target/release/QuickTran.exe`

*(Note: Installers are disabled to keep the distribution size minimal and output a single portable binary).*
