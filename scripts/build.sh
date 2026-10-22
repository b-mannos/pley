#!/usr/bin/env sh
# Release build: web UI + both binaries + licensing files.
#   ./scripts/build.sh [--skip-web] [--target <rust target triple>] [--archive]
# Output: dist/pley-<version>-<os>-<arch>/ (plus a .tar.gz with --archive)
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/apps/server/Cargo.toml" | head -n1)

skip_web=0
target=""
archive=0
while [ $# -gt 0 ]; do
    case "$1" in
        --skip-web) skip_web=1 ;;
        --target) target=$2; shift ;;
        --archive) archive=1 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ "$skip_web" = 0 ]; then
    (cd "$root" && { [ -d node_modules ] || npm ci --no-fund --no-audit; } && npm run build -w apps/web)
fi
touch "$root/apps/server/src/web.rs"   # re-embed the fresh UI build

if [ -n "$target" ]; then
    (cd "$root" && cargo build --release --locked --target "$target" -p pley-server -p pley-transcoder)
    bin="$root/target/$target/release"
    case "$target" in
        *-apple-darwin) os=macos ;;
        *-linux-*) os=linux ;;
        *) os=$(echo "$target" | cut -d- -f3) ;;
    esac
    arch=$(echo "$target" | cut -d- -f1)
else
    (cd "$root" && cargo build --release --locked -p pley-server -p pley-transcoder)
    bin="$root/target/release"
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    [ "$os" = darwin ] && os=macos
    arch=$(uname -m)
fi
case "$arch" in
    aarch64) arch=arm64 ;;
    amd64) arch=x86_64 ;;
esac

name="pley-$version-$os-$arch"
out="$root/dist/$name"
rm -rf "$out" && mkdir -p "$out"
cp "$bin/pley" "$bin/pley-transcoder" "$out/"
cp "$root/apps/server/LICENSE" "$out/LICENSE.txt"
cp "$root/LICENSING.md" "$root/README.md" "$root/pley.example.toml" "$out/"

if [ "$archive" = 1 ]; then
    tar -C "$root/dist" -czf "$root/dist/$name.tar.gz" "$name"
    echo "wrote dist/$name.tar.gz"
fi
echo "release ready in $out"
