#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?version is required}"
PLATFORM_RAW="${2:?platform is required}"
PLATFORM="$(printf '%s' "$PLATFORM_RAW" | tr '[:upper:]' '[:lower:]')"
PROJECT_ROOT="$(pwd)"
APP_NAME="quran-tui"
BINARY_PATH="$PROJECT_ROOT/target/release/$APP_NAME"
DIST_DIR="$PROJECT_ROOT/dist"
PACKAGE_NAME="$APP_NAME-$VERSION-$PLATFORM"
STAGE_DIR="$DIST_DIR/$PACKAGE_NAME"
ARCHIVE_PATH="$DIST_DIR/$PACKAGE_NAME.tar.gz"
LAUNCHER_PATH="$STAGE_DIR/run-quran-tui.sh"

rm -rf "$STAGE_DIR" "$ARCHIVE_PATH"
mkdir -p "$STAGE_DIR"
cp "$BINARY_PATH" "$STAGE_DIR/$APP_NAME"
cp "$PROJECT_ROOT/README.md" "$STAGE_DIR/README.md"
cp "$PROJECT_ROOT/SETUP.md" "$STAGE_DIR/SETUP.md"
cat > "$LAUNCHER_PATH" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if ! command -v mpv >/dev/null 2>&1; then
  echo "Error: mpv is not installed or not on PATH."
  echo "See SETUP.md for installation instructions."
  exit 1
fi
exec "$SCRIPT_DIR/quran-tui" "$@"
EOF
chmod +x "$STAGE_DIR/$APP_NAME" "$LAUNCHER_PATH"
tar -czf "$ARCHIVE_PATH" -C "$DIST_DIR" "$PACKAGE_NAME"
rm -rf "$STAGE_DIR"

echo "Created $ARCHIVE_PATH"
