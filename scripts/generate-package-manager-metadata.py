#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import shutil
import sys
from pathlib import Path
from textwrap import dedent
import zipfile

APP_NAME = "quran-tui"
PACKAGE_IDENTIFIER = "KhalidElBorai.QuranTui"
PACKAGE_TITLE = "quran-tui"
DESCRIPTION = "Terminal Quran player and study app"
LONG_DESCRIPTION = "Terminal Quran player and study app with browse, playback, downloads, and study mode."
HOMEPAGE = "https://github.com/khalidelborai/quran-tui"
PUBLISHER = "Khalid El Borai"
PUBLISHER_URL = "https://github.com/khalidelborai"
SUPPORT_URL = "https://github.com/khalidelborai/quran-tui/issues"

def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def release_url(repo: str, version: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{version}/{filename}"


def write_homebrew(version: str, repo: str, macos_tar: Path, output_dir: Path) -> Path:
    formula_path = output_dir / f"{APP_NAME}-{version}-homebrew-formula.rb"
    formula_path.write_text(dedent(f'''
        class QuranTui < Formula
          desc "{DESCRIPTION}"
          homepage "{HOMEPAGE}"
          url "{release_url(repo, version, macos_tar.name)}"
          version "{version.lstrip('v')}"
          sha256 "{sha256(macos_tar)}"

          depends_on "mpv"
          depends_on arch: :arm64

          def install
            bin.install "quran-tui"
            doc.install "README.md", "SETUP.md"
          end
        end
    ''').strip() + "\n")
    return formula_path


def write_scoop(version: str, repo: str, windows_zip: Path, output_dir: Path) -> Path:
    manifest = {
        "version": version.lstrip('v'),
        "description": DESCRIPTION,
        "homepage": HOMEPAGE,
        "url": release_url(repo, version, windows_zip.name),
        "hash": sha256(windows_zip),
        "bin": "quran-tui.exe",
        "notes": [
            "Install mpv separately and see SETUP.md for Arabic font guidance.",
        ],
    }
    path = output_dir / f"{APP_NAME}-{version}-scoop.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    return path


def write_winget(version: str, repo: str, windows_zip: Path, output_dir: Path) -> Path:
    base = output_dir / f"{APP_NAME}-{version}-winget-manifests"
    if base.exists():
        shutil.rmtree(base)
    base.mkdir(parents=True)
    version_num = version.lstrip('v')
    (base / f"{PACKAGE_IDENTIFIER}.yaml").write_text(dedent(f'''
        PackageIdentifier: "{PACKAGE_IDENTIFIER}"
        PackageVersion: "{version_num}"
        DefaultLocale: "en-US"
        ManifestType: "version"
        ManifestVersion: "1.12.0"
    ''').strip() + "\n")
    (base / f"{PACKAGE_IDENTIFIER}.locale.en-US.yaml").write_text(dedent(f'''
        PackageIdentifier: "{PACKAGE_IDENTIFIER}"
        PackageVersion: "{version_num}"
        PackageLocale: "en-US"
        Publisher: "{PUBLISHER}"
        PublisherUrl: "{PUBLISHER_URL}"
        PublisherSupportUrl: "{SUPPORT_URL}"
        PackageName: "{PACKAGE_TITLE}"
        PackageUrl: "{HOMEPAGE}"
        License: "UNLICENSED"
        ShortDescription: "{DESCRIPTION}"
        Description: "{LONG_DESCRIPTION}"
        Tags:
        - "quran"
        - "tui"
        - "terminal"
        - "audio"
        - "study"
        ManifestType: "defaultLocale"
        ManifestVersion: "1.12.0"
    ''').strip() + "\n")
    (base / f"{PACKAGE_IDENTIFIER}.installer.yaml").write_text(dedent(f'''
        PackageIdentifier: "{PACKAGE_IDENTIFIER}"
        PackageVersion: "{version_num}"
        InstallerType: "zip"
        NestedInstallerType: "portable"
        InstallModes:
        - "interactive"
        - "silent"
        Installers:
        - Architecture: "x64"
          InstallerUrl: "{release_url(repo, version, windows_zip.name)}"
          InstallerSha256: "{sha256(windows_zip)}"
          NestedInstallerFiles:
          - RelativeFilePath: "quran-tui.exe"
            PortableCommandAlias: "quran-tui"
        ManifestType: "installer"
        ManifestVersion: "1.12.0"
    ''').strip() + "\n")
    zip_path = output_dir / f"{APP_NAME}-{version}-winget-manifests.zip"
    if zip_path.exists():
        zip_path.unlink()
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
        for file in sorted(base.iterdir()):
            zf.write(file, arcname=file.name)
    shutil.rmtree(base)
    return zip_path


def write_chocolatey(version: str, repo: str, windows_zip: Path, output_dir: Path) -> Path:
    base = output_dir / f"{APP_NAME}-{version}-chocolatey"
    if base.exists():
        shutil.rmtree(base)
    tools_dir = base / "tools"
    tools_dir.mkdir(parents=True)
    version_num = version.lstrip('v')
    nuspec = dedent(f'''
        <?xml version="1.0"?>
        <package>
          <metadata>
            <id>quran-tui</id>
            <version>{version_num}</version>
            <title>quran-tui</title>
            <authors>{PUBLISHER}</authors>
            <projectUrl>{HOMEPAGE}</projectUrl>
            <projectSourceUrl>{HOMEPAGE}</projectSourceUrl>
            <docsUrl>{HOMEPAGE}</docsUrl>
            <licenseUrl>{HOMEPAGE}</licenseUrl>
            <requireLicenseAcceptance>false</requireLicenseAcceptance>
            <summary>{DESCRIPTION}</summary>
            <description>{LONG_DESCRIPTION}</description>
            <tags>quran tui terminal audio study</tags>
          </metadata>
        </package>
    ''').strip() + "\n"
    (base / "quran-tui.nuspec").write_text(nuspec)
    install_ps1 = dedent(f'''
        $ErrorActionPreference = 'Stop'

        $packageArgs = @{{
          packageName    = 'quran-tui'
          unzipLocation  = Split-Path -Parent $MyInvocation.MyCommand.Definition
          url64bit       = '{release_url(repo, version, windows_zip.name)}'
          checksum64     = '{sha256(windows_zip)}'
          checksumType64 = 'sha256'
        }}

        Install-ChocolateyZipPackage @packageArgs
    ''').strip() + "\n"
    (tools_dir / "chocolateyInstall.ps1").write_text(install_ps1)
    zip_path = output_dir / f"{APP_NAME}-{version}-chocolatey-source.zip"
    if zip_path.exists():
        zip_path.unlink()
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
        for file in sorted(base.rglob('*')):
            if file.is_file():
                zf.write(file, arcname=file.relative_to(base))
    shutil.rmtree(base)
    return zip_path


def main() -> int:
    if len(sys.argv) != 5:
        print("usage: generate-package-manager-metadata.py <version> <repo> <assets_dir> <output_dir>", file=sys.stderr)
        return 1
    version, repo, assets_dir_raw, output_dir_raw = sys.argv[1:]
    assets_dir = Path(assets_dir_raw)
    output_dir = Path(output_dir_raw)
    output_dir.mkdir(parents=True, exist_ok=True)

    macos_tar = next(assets_dir.rglob(f"{APP_NAME}-{version}-macos-*.tar.gz"), None)
    windows_zip = next(assets_dir.rglob(f"{APP_NAME}-{version}-windows-*.zip"), None)
    if macos_tar is None or windows_zip is None:
        missing = []
        if macos_tar is None:
            missing.append('macOS tarball')
        if windows_zip is None:
            missing.append('windows zip')
        print(f"missing required assets: {', '.join(missing)}", file=sys.stderr)
        return 1

    outputs = [
        write_homebrew(version, repo, macos_tar, output_dir),
        write_scoop(version, repo, windows_zip, output_dir),
        write_winget(version, repo, windows_zip, output_dir),
        write_chocolatey(version, repo, windows_zip, output_dir),
    ]
    for path in outputs:
        print(path)
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
