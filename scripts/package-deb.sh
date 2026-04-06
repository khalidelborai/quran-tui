#!/usr/bin/env bash
set -euo pipefail

RAW_VERSION="${1:?version is required}"
DEB_VERSION="${RAW_VERSION#v}"
PROJECT_ROOT="$(pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
APP_NAME="quran-tui"
ARCH="$(dpkg --print-architecture)"
OUTPUT_PATH="$DIST_DIR/${APP_NAME}-${RAW_VERSION}-linux-${ARCH}.deb"

mkdir -p "$DIST_DIR"
rm -f "$OUTPUT_PATH"
rm -f "$PROJECT_ROOT"/target/debian/*.deb

cargo deb --no-build --deb-version "$DEB_VERSION"

DEB_PATH="$(find "$PROJECT_ROOT/target/debian" -maxdepth 1 -type f -name '*.deb' | head -n 1)"
if [[ -z "$DEB_PATH" ]]; then
  echo "No .deb package was produced" >&2
  exit 1
fi

cp "$DEB_PATH" "$OUTPUT_PATH"
echo "Created $OUTPUT_PATH"
