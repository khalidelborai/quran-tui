param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [string]$Platform,
    [string]$ProjectRoot = "."
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path $ProjectRoot).Path
$AppName = "quran-tui"
$PackageName = "$AppName-$Version-$($Platform.ToLower())"
$DistDir = Join-Path $ProjectRoot "dist"
$StageDir = Join-Path $DistDir $PackageName
$BinaryPath = Join-Path $ProjectRoot "target/release/$AppName.exe"
$ArchivePath = Join-Path $DistDir "$PackageName.zip"
$LauncherPath = Join-Path $StageDir "run-quran-tui.cmd"

if (Test-Path $StageDir) {
    Remove-Item $StageDir -Recurse -Force
}
if (Test-Path $ArchivePath) {
    Remove-Item $ArchivePath -Force
}

New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
Copy-Item $BinaryPath (Join-Path $StageDir "$AppName.exe")
Copy-Item (Join-Path $ProjectRoot "README.md") (Join-Path $StageDir "README.md")
Copy-Item (Join-Path $ProjectRoot "SETUP.md") (Join-Path $StageDir "SETUP.md")
Set-Content -Path $LauncherPath -Value @'
@echo off
where mpv >nul 2>nul
if errorlevel 1 (
  echo Error: mpv is not installed or not on PATH.
  echo See SETUP.md for installation instructions.
  exit /b 1
)
"%~dp0quran-tui.exe" %*
'@ -NoNewline
Compress-Archive -Path $StageDir -DestinationPath $ArchivePath
Remove-Item $StageDir -Recurse -Force
Write-Host "Created $ArchivePath"
