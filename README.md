# quran-tui

Terminal Quran player with:
- mp3quran audio playback
- ayah timing sync
- terminal-native Arabic ayah display


## Setup

See [SETUP.md](./SETUP.md) for Rust, `mpv`, and Arabic font setup.

## Quran font recommendation

This TUI now renders the active ayah as **terminal-native text** instead of drawing it as an image.
That means the app can shape and wrap Arabic text correctly, but the **actual font** is chosen by
your terminal emulator, not by the app itself.

Recommended font stack for Quran text:

```text
KFGQPC Uthmanic Script Hafs
KFGQPC Warsh
Amiri Quran
Amiri
Noto Naskh Arabic
```

Practical advice:

- If you want the best general result, use **Amiri** or **Noto Naskh Arabic** as your terminal font.
- If your terminal supports font fallback, add the Quran-specific fonts first and keep Amiri/Noto as fallback.
- For the cleanest terminal presentation in this app, the ayah text API currently uses a simpler Arabic edition instead of dense Uthmani glyph variants.


## CI and releases

- Pull requests and pushes to `main` run GitHub Actions CI on Linux, macOS, and Windows.
- Tags matching `v*` build release binaries and upload packaged assets to GitHub Releases.
- Linux releases include both a `.tar.gz` archive and a native `.deb` package.
- Release archives include the binary, a helper launcher script, `README.md`, and `SETUP.md`.
- Release archive names use normalized Rust host triples such as `linux-x86_64`, `macos-aarch64`, or `windows-x86_64`.
- Runtime dependencies such as `mpv` and Arabic fonts are **not** bundled; install them separately.

Example release flow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Important limitation

Because this is a TUI:

- the app **cannot force a font family**
- visual quality depends on your terminal emulator
- font fallback behavior varies between terminals

If you need exact Mushaf-style typography, that should be done in a GUI/image-rendered surface rather than plain terminal text.
