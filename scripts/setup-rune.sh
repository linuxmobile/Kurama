#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
pin="$root/rune-pin.toml"
url=$(sed -n 's/^url = "\(.*\)"$/\1/p' "$pin")
revision=$(sed -n 's/^revision = "\(.*\)"$/\1/p' "$pin")
destination="$root/rune"

if [ -z "$url" ] || [ -z "$revision" ]; then
  echo "Invalid rune-pin.toml" >&2
  exit 1
fi

if [ ! -d "$destination/.git" ]; then
  rm -rf "$destination"
  git clone "$url" "$destination"
else
  git -C "$destination" remote set-url origin "$url"
fi

git -C "$destination" fetch --force origin "$revision"
git -C "$destination" checkout --force --detach "$revision"

actual=$(git -C "$destination" rev-parse HEAD)
if [ "$actual" != "$revision" ]; then
  echo "Rune checkout is $actual, expected $revision" >&2
  exit 1
fi
