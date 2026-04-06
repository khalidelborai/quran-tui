# Setup Guide

## Requirements

- Rust toolchain (`cargo`, `rustc`)
- `mpv` available on `PATH`
- A terminal with solid Arabic shaping and font fallback
- For Windows, prefer **WezTerm** for the most predictable Arabic font setup

## 1. Install Rust

### Linux / macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version
cargo --version
```

### Windows

Download and run `rustup-init.exe`, then open a new PowerShell or Command Prompt window and verify:

```powershell
rustc --version
cargo --version
```

If the installer asks for Visual Studio C++ build tools, install them and rerun the check.

## 2. Install mpv

### Ubuntu / Debian
```bash
sudo apt update
sudo apt install mpv
```

### Fedora
```bash
sudo dnf install mpv
```

### Arch
```bash
sudo pacman -S mpv
```

### macOS
```bash
brew install mpv
```

### Windows

mpv's official installation page lists these Windows options:

- shinchiro builds
- zhongfly builds
- Scoop
- Chocolatey
- MSYS2

Practical install commands:

```powershell
scoop install mpv
```

or:

```powershell
choco install mpvio
```

If you do not use Scoop or Chocolatey, download a current Windows build from the mpv installation page and make sure the `mpv.exe` location is on `PATH`.

Verify that playback support is available:

### Linux / macOS
```bash
mpv --version
which mpv
```

### Windows
```powershell
mpv --version
Get-Command mpv
```

If `mpv` is missing or not on `PATH`, audio playback will not start.

## 3. Install a terminal that handles Arabic well

### Windows: install WezTerm

WezTerm has an official Windows installer and official WinGet package:

```powershell
winget install wez.wezterm
```

### Linux / macOS

Use a terminal with HarfBuzz-based shaping and font fallback. WezTerm is a good default here as well.

## 4. Install Arabic-capable fonts

Recommended stack for this app:

```text
KFGQPC Uthmanic Script Hafs
KFGQPC Warsh
Amiri Quran
Amiri
Noto Naskh Arabic
```

Practical default:
- use **Amiri** or **Noto Naskh Arabic** as the main terminal font
- add Quran-specific fonts as fallback when your terminal supports it

### Linux

User fonts are commonly installed in:

```text
~/.local/share/fonts
```

After copying font files there, refresh the cache:

```bash
fc-cache -fv
fc-list | grep -Ei 'amiri|noto naskh|kfgqpc'
```

### Windows

Install `.ttf` or `.otf` font files from Settings > Personalization > Fonts, or right-click the font file and choose **Install**. Restart the terminal after installing fonts.

## 5. Configure your terminal

### WezTerm example (Linux / macOS / Windows)

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback {
    'Amiri Quran',
    'Amiri',
    'Noto Naskh Arabic',
  },
  font_shaper = 'Harfbuzz',
}
```

### Kitty example (Linux / macOS / BSD)

```conf
font_family      Amiri
bold_font        auto
italic_font      auto
bold_italic_font auto
symbol_map U+0600-U+06FF,U+0750-U+077F,U+08A0-U+08FF Amiri Quran
```

## 6. Run the app

```bash
cd quran-tui
cargo run
```

Useful checks:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Troubleshooting

### Arabic text looks broken
- switch the terminal font to **Amiri** or **Noto Naskh Arabic**
- enable font fallback
- prefer WezTerm or another terminal with HarfBuzz-based shaping
- on Linux, verify the font is installed with `fc-list`
- on Windows, restart the terminal after installing fonts

### Audio does not start
- run `mpv --version`
- confirm `mpv` is on `PATH`
- start the app from the same shell where `mpv` works

### Text is readable but not Mushaf-perfect
That is expected in a TUI. This app renders terminal-native Arabic text, so exact Mushaf typography depends on terminal rendering and available fonts.
