#!/usr/bin/env bash
# Copies the latest Claude Code session transcript of this project to an archive
# folder outside the repository and prints its sha256 for docs/journal/.
# Usage: scripts/archive-session.sh [session-id]
# Archive folder: $PRISM_ARCHIVE_DIR (default: ~/Documents/prism-archives/sessions).
set -euo pipefail
cd "$(dirname "$0")/.."

project_dir="$HOME/.claude/projects/$(pwd | tr '/' '-')"
archive_dir="${PRISM_ARCHIVE_DIR:-$HOME/Documents/prism-archives/sessions}"

if [ $# -ge 1 ]; then
  transcript="$project_dir/$1.jsonl"
else
  transcript=$(ls -t "$project_dir"/*.jsonl 2>/dev/null | head -n 1 || true)
fi

if [ -z "${transcript:-}" ] || [ ! -f "$transcript" ]; then
  echo "archive-session: no transcript found in $project_dir" >&2
  exit 1
fi

mkdir -p "$archive_dir"
name="$(date +%Y-%m-%d)-$(basename "$transcript")"
# A session archived again the same day keeps its earlier copies, which are
# read-only: the new copy gets the time in its name.
if [ -e "$archive_dir/$name" ]; then
  name="$(date +%Y-%m-%d-%H%M%S)-$(basename "$transcript")"
fi
cp "$transcript" "$archive_dir/$name"
chmod 444 "$archive_dir/$name"
hash=$(shasum -a 256 "$archive_dir/$name" | cut -d' ' -f1)

echo "Archived: $archive_dir/$name"
echo "Line for the journal:"
echo "- Transcription : \`$name\`, sha256 \`$hash\`"
