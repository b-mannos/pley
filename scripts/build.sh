#!/usr/bin/env sh
# Release build: web UI + both binaries + licensing files.   ./scripts/build.sh [--skip-web]
# Output: dist/pley-<version>-<os>-<arch>/
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/pley-server/Cargo.toml" | head -n1)

if [ "${1:-}" != "--skip-web" ]; then
    (cd "$root/web" && { [ -d node_modules ] || npm ci --no-fund --no-audit; } && npm run build)
fi
touch "$root/crates/pley-server/src/web.rs"   # re-embed the fresh UI build
(cd "$root" && cargo build --release -p pley-server -p pley-transcoder)

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
out="$root/dist/pley-$version-$os-$arch"
rm -rf "$out" && mkdir -p "$out"
cp "$root/target/release/pley" "$root/target/release/pley-transcoder" "$out/"
cp "$root/crates/pley-server/LICENSE" "$out/LICENSE.txt"
cp "$root/LICENSING.md" "$root/README.md" "$root/pley.example.toml" "$out/"
echo "release ready in $out"
