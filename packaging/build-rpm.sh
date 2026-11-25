#!/usr/bin/env bash
set -euo pipefail

project=$(cd "$(dirname "$0")/.." && pwd)
dist=$(realpath -m "${PHOTO60_DIST_DIR:-$project/dist}")
top=$(mktemp -d)
trap 'rm -rf "$top"' EXIT
mkdir -p "$top"/{SOURCES,tmp} "$dist"
version=$(rpmspec -q --srpm --qf '%{VERSION}' "$project/packaging/photo60-test.spec")
base="photo60-test-$version"
items=(Cargo.toml Cargo.lock src assets tools packaging)
for file in README.md README.zh-CN.md LICENSE; do
    if test -f "$project/$file"; then items+=("$file"); fi
done
tar --directory="$project" --sort=name --owner=0 --group=0 --numeric-owner \
    --exclude='__pycache__' --exclude='*.pyc' \
    --transform="s,^,$base/," -czf "$top/SOURCES/$base.tar.gz" "${items[@]}"

cargo fetch --locked --manifest-path "$project/Cargo.toml"
rpmbuild -bb --define "_topdir $top" --define "_tmppath $top/tmp" \
    "$@" "$project/packaging/photo60-test.spec"
cp "$top/SOURCES/$base.tar.gz" "$top"/RPMS/*/*.rpm "$dist/"
