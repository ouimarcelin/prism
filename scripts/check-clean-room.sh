#!/usr/bin/env bash
# Fails when third-party brand names or proprietary file formats appear in
# source code, assets or tracked files. See docs/CLEAN_ROOM.md, section 6.
# A line may opt out with the marker "clean-room-allow" (non-affiliation notice only).
set -euo pipefail
cd "$(dirname "$0")/.."

status=0
pattern='resolume|avenue|dxv|\.avc([^a-z0-9]|$)'

paths=()
for p in crates apps assets src; do
  [ -e "$p" ] && paths+=("$p")
done

if [ ${#paths[@]} -gt 0 ]; then
  if matches=$(grep -rniE --binary-files=without-match "$pattern" "${paths[@]}" | grep -v 'clean-room-allow'); then
    echo "clean-room: forbidden terms found in source or assets:"
    echo "$matches"
    status=1
  fi
fi

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if files=$(git ls-files | grep -iE '\.(avc|dxv)$|resolume'); then
    echo "clean-room: forbidden files are tracked:"
    echo "$files"
    status=1
  fi
fi

[ "$status" -eq 0 ] && echo "clean-room: OK"
exit "$status"
