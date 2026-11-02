#!/usr/bin/env sh
# Synology package from a static Linux release folder (deploy/synology):
#   ./scripts/build-spk.sh dist/pley-server-<version>-linux-x86_64-static [<out dir>]
# Output: <out dir>/pley-server-<version>-synology-x86_64.spk (default: dist).
# What the package does is in deploy/synology/README.md.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
syn="$root/deploy/synology"

[ $# -ge 1 ] || { echo "usage: $0 <release folder> [<out dir>]" >&2; exit 2; }
release=$(cd "$1" && pwd)
name=$(basename "$release")
case "$name" in
    pley-server-*-linux-x86_64-static) ;;
    *) echo "$name isn't a static Linux release folder (pley-server-<version>-linux-x86_64-static)" >&2; exit 2 ;;
esac
version=${name#pley-server-}
version=${version%-linux-x86_64-static}
# DSM's versions are numbers separated by ".", "-" or "_".
case "$version" in
    *[!0-9._-]*) echo "DSM takes only numbers in a version: $version" >&2; exit 2 ;;
esac
for f in pley pley-transcoder LICENSE.txt LICENSING.md README.md pley.example.toml; do
    [ -f "$release/$f" ] || { echo "$release has no $f" >&2; exit 2; }
done
out=${2:-$root/dist}
mkdir -p "$out"
out=$(cd "$out" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export COPYFILE_DISABLE=1   # no ._ files from macOS

# package.tgz: what DSM extracts to /var/packages/pley/target.
mkdir -p "$work/package/bin"
cp "$release/pley" "$release/pley-transcoder" "$work/package/bin/"
chmod 755 "$work/package/bin/"*
cp "$release/LICENSE.txt" "$release/LICENSING.md" "$release/README.md" "$release/pley.example.toml" "$work/package/"
cp -R "$syn/app" "$work/package/app"
tar -C "$work/package" -czf "$work/package.tgz" .

# The package: INFO, icons, scripts, conf, wizards and package.tgz.
cp -R "$syn/scripts" "$syn/conf" "$syn/WIZARD_UIFILES" "$work/"
chmod 755 "$work/scripts/"*
cp "$syn/PACKAGE_ICON.PNG" "$syn/PACKAGE_ICON_256.PNG" "$work/"
if command -v md5sum > /dev/null 2>&1; then
    checksum=$(md5sum "$work/package.tgz" | cut -d' ' -f1)
else
    checksum=$(md5 -q "$work/package.tgz")
fi
extractsize=$(du -sk "$work/package" | cut -f1)
create_time=$(date -u +%Y%m%d-%H:%M:%S)
sed -e "s/@VERSION@/$version/" -e "s/@CHECKSUM@/$checksum/" -e "s/@EXTRACTSIZE@/$extractsize/" \
    -e "s/@CREATE_TIME@/$create_time/" "$syn/INFO.in" > "$work/INFO"
grep -q '@' "$work/INFO" && { echo "INFO.in has a placeholder build-spk.sh doesn't fill" >&2; exit 1; }

spk="$out/pley-server-$version-synology-x86_64.spk"
rm -f "$spk"
(cd "$work" && tar -cf "$spk" INFO PACKAGE_ICON.PNG PACKAGE_ICON_256.PNG package.tgz scripts conf WIZARD_UIFILES)
echo "wrote $spk"
