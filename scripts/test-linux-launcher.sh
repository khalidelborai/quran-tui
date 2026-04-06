#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cp "$PROJECT_ROOT/packaging/linux/quran-tui" "$TMP_DIR/quran-tui"
cat > "$TMP_DIR/quran-tui-bin" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'LC_ALL=%s\n' "${LC_ALL:-}"
locale charmap
EOF
cat > "$TMP_DIR/mpv" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$TMP_DIR/quran-tui" "$TMP_DIR/quran-tui-bin" "$TMP_DIR/mpv"

PATH="$TMP_DIR:$PATH" LC_ALL=C "$TMP_DIR/quran-tui" >"$TMP_DIR/stdout.txt" 2>"$TMP_DIR/stderr.txt"

grep -Ei '^LC_ALL=.*utf-?8$' "$TMP_DIR/stdout.txt" >/dev/null
grep -Fx 'UTF-8' "$TMP_DIR/stdout.txt" >/dev/null
grep -F 'non-UTF-8 locale detected' "$TMP_DIR/stderr.txt" >/dev/null

echo "launcher UTF-8 fallback smoke test passed"
