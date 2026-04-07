#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/release-helper.sh <x.y.z>

Example:
  ./scripts/release-helper.sh 0.2.0
EOF
}

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

version="$1"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: version must follow SemVer format X.Y.Z" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo_toml="$repo_root/Cargo.toml"

for file in "$cargo_toml"; do
  if [[ ! -f "$file" ]]; then
    echo "Error: missing file $file" >&2
    exit 1
  fi
done

sed -E -i "s/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"$/version = \"$version\"/" "$cargo_toml"

cat <<EOF
Release helper complete for version $version.

Next steps:
1. Review diff.
2. Commit changes:
   git add -A
   git commit -m "chore: release v$version"
3. Tag and push (SemVer tag convention with v-prefix):
   git tag -a v$version -m "Release v$version"
   git push && git push --tags
4. Verify GitHub release artifacts.
EOF
