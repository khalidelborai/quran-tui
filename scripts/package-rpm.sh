#!/usr/bin/env bash
set -euo pipefail

RAW_VERSION="${1:?version is required}"
RPM_VERSION="${RAW_VERSION#v}"
PROJECT_ROOT="$(pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
APP_NAME="quran-tui"
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64) ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *) ARCH="$ARCH_RAW" ;;
esac
OUTPUT_PATH="$DIST_DIR/${APP_NAME}-${RAW_VERSION}-linux-${ARCH}.rpm"

mkdir -p "$DIST_DIR"
rm -f "$OUTPUT_PATH"
rm -f "$PROJECT_ROOT"/target/generate-rpm/*.rpm

cargo generate-rpm -o "$OUTPUT_PATH" --set-metadata "version = \"$RPM_VERSION\""

if [[ ! -f "$OUTPUT_PATH" ]]; then
  echo "No .rpm package was produced" >&2
  exit 1
fi

echo "Created $OUTPUT_PATH"
