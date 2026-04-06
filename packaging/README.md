# Packaging Registry Notes

This repository publishes first-party release assets on GitHub Releases.

Additional package-manager metadata is generated for each tagged release:

- `*.deb` for Debian/Ubuntu-style installs
- `*.rpm` for RPM-based Linux distributions
- `*-homebrew-formula.rb` for a Homebrew tap (currently targeting the macOS arm64 release asset)
- `*-scoop.json` for a Scoop bucket
- `*-winget-manifests.zip` for WinGet submission
- `*-chocolatey-source.zip` for Chocolatey packaging

These generated files are intended to be the source material for external package repositories.
They are not auto-submitted to Homebrew, WinGet, Scoop, or Chocolatey from this repository.

## Linux packaging notes

- Linux release artifacts are built on an **Ubuntu 22.04** runner.
- The shipped Linux entrypoint is a wrapper installed as `quran-tui`.
- The real packaged binary is stored as `quran-tui-bin`.
- The wrapper checks for `mpv` and applies a UTF-8 locale fallback for Arabic text rendering.

## Manual publishing flow

- Homebrew: copy the generated formula into a `Formula/` directory inside a `homebrew-*` tap repo.
- WinGet: submit the generated YAML manifest set to `microsoft/winget-pkgs`.
- Scoop: copy the generated JSON manifest into a Scoop bucket repository.
- Chocolatey: unpack the generated source zip, review it, run `choco pack`, then publish.
