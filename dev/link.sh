#!/usr/bin/env bash
# Opt-in local link between this madar-shared checkout and the consumers
# (MadarRust, madar/rust-core, and any worktree of them), so a change to a shared
# crate builds in the backend and the core without pushing a tag.
#
#   dev/link.sh on  [consumer ...]   # link (default: every sibling consumer)
#   dev/link.sh off [consumer ...]   # unlink and put Cargo.lock back on the tag
#   dev/link.sh status
#
# A "consumer" is a Cargo workspace whose Cargo.toml depends on
# https://github.com/Shawket4/madar-shared. By default this script finds them
# beside this checkout: ../MadarRust, ../madar/rust-core, and ../wt-*/ (plus
# ../wt-*/rust-core).
#
# How: `on` writes <consumer>/.cargo/config.toml with a [patch] to this
# checkout's crates (marked as ours, and git-ignored through the clone's
# info/exclude, never committed). `off` removes it and re-resolves, so
# Cargo.lock returns to the pinned `git+…?tag=` lines.
#
# While linked, Cargo.lock in the consumer shows the crates as local paths:
# NEVER commit it in that state (CI runs --locked and would fail). Unlink first.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
PARENT="$(dirname "$HERE")"
URL="https://github.com/Shawket4/madar-shared"
MARK="# madar-shared dev/link.sh — local link, do not commit"

crates() {
  for toml in "$HERE"/crates/*/Cargo.toml; do
    local name
    name="$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$toml" | head -1)"
    printf '%s = { path = "%s" }\n' "$name" "$(dirname "$toml")"
  done
}

consumers() {
  if [ $# -gt 0 ]; then
    for c in "$@"; do (cd "$c" && pwd); done
    return
  fi
  for c in "$PARENT/MadarRust" "$PARENT/madar/rust-core" "$PARENT"/wt-*/ "$PARENT"/wt-*/rust-core; do
    c="${c%/}"
    [ -f "$c/Cargo.toml" ] || continue
    # The dependency may sit in the root or in a member crate (madar-core).
    grep -qs "$URL" "$c/Cargo.toml" "$c"/crates/*/Cargo.toml && echo "$c"
  done
}

exclude() {  # git-ignore .cargo/config.toml in this clone only
  local c="$1" ex
  ex="$(git -C "$c" rev-parse --git-common-dir 2>/dev/null)/info/exclude" || return 0
  case "$ex" in /*) ;; *) ex="$c/$ex" ;; esac
  mkdir -p "$(dirname "$ex")"
  grep -qx '.cargo/config.toml' "$ex" 2>/dev/null || echo '.cargo/config.toml' >> "$ex"
  grep -qx 'rust-core/.cargo/config.toml' "$ex" 2>/dev/null || echo 'rust-core/.cargo/config.toml' >> "$ex"
}

on() {
  for c in $(consumers "$@"); do
    local cfg="$c/.cargo/config.toml"
    if [ -f "$cfg" ] && ! grep -qF "$MARK" "$cfg"; then
      echo "skip $c: it has its own .cargo/config.toml (add the [patch] by hand)"; continue
    fi
    mkdir -p "$c/.cargo"
    { echo "$MARK"; echo "[patch.\"$URL\"]"; crates; } > "$cfg"
    exclude "$c"
    (cd "$c" && cargo metadata --format-version 1 >/dev/null)
    echo "linked   $c"
  done
}

off() {
  for c in $(consumers "$@"); do
    local cfg="$c/.cargo/config.toml"
    if [ -f "$cfg" ] && grep -qF "$MARK" "$cfg"; then
      rm "$cfg"
      rmdir "$c/.cargo" 2>/dev/null || true
      # Re-resolve against the tag in Cargo.toml: the lock's git lines return.
      (cd "$c" && cargo metadata --format-version 1 >/dev/null)
      if git -C "$c" diff --quiet -- Cargo.lock 2>/dev/null; then
        echo "unlinked $c (Cargo.lock clean)"
      else
        echo "unlinked $c — Cargo.lock still differs from git: check 'git -C $c diff Cargo.lock'"
      fi
    fi
  done
}

status() {
  for c in $(consumers "$@"); do
    if grep -qsF "$MARK" "$c/.cargo/config.toml"; then echo "linked   $c"; else echo "on tag   $c"; fi
  done
}

cmd="${1:-status}"; shift || true
case "$cmd" in
  on) on "$@" ;;
  off) off "$@" ;;
  status) status "$@" ;;
  *) echo "usage: dev/link.sh on|off|status [consumer ...]"; exit 2 ;;
esac
