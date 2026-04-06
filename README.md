# quran-tui

Terminal Quran player with:
- mp3quran audio playback
- ayah timing sync
- terminal-native Arabic ayah display

## Setup

See [SETUP.md](./SETUP.md) for Rust, `mpv`, and Arabic font setup.

## Quick start

### From source

```bash
cargo run
```

### From Linux release packages

- `.tar.gz`: extract, then run `./quran-tui`
- `.deb`: install with `sudo dpkg -i quran-tui-*.deb`
- `.rpm`: install with `sudo rpm -i quran-tui-*.rpm`

Packaged Linux builds expose a `quran-tui` launcher that:
- checks that `mpv` is installed
- retries with a UTF-8 locale when the current shell is not UTF-8

The packaged payload binary is `quran-tui-bin`; launch `quran-tui` unless you are debugging packaging internals.

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
- Linux releases include `.tar.gz`, `.deb`, and `.rpm` assets built on an Ubuntu 22.04 baseline.
- Unix release archives include a `quran-tui` launcher, the `quran-tui-bin` payload, a compatibility `run-quran-tui.sh`, `README.md`, and `SETUP.md`.
- Release archive names use normalized Rust host triples such as `linux-x86_64`, `macos-aarch64`, or `windows-x86_64`.
- Runtime dependencies such as `mpv` and Arabic fonts are **not** bundled; install them separately.
- Generated package-manager metadata for Homebrew, WinGet, Scoop, and Chocolatey is attached to tagged releases for downstream publishing.

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
