#!/usr/bin/env bash
# Build madar-web to WebAssembly: dist/public and dist/full, each the wasm, its
# JS glue (`wasm-bindgen --target web`) and its .d.ts, plus dist/SHA256SUMS.
#
# The recipe (SHARED_RULES_PLAN.md Step 3, each step measured): a pinned
# nightly rebuilding std for size, panics that abort at once, the `wasm`
# profile (opt-level z, fat LTO, one codegen unit; native builds never use it),
# chrono-tz cut to scripts/tz-filter.txt, then wasm-opt.
#
# Needs: the pinned nightly with rust-src and the wasm32-unknown-unknown
# target, wasm-bindgen-cli 0.2.129 (the crate's pin), and wasm-opt (binaryen).
# MADAR_WASM_TOOLCHAIN names a toolchain other than nightly-2026-06-16 (a local
# `nightly`, say); it must be the same compiler, which the commit check
# enforces.
set -euo pipefail
cd "$(dirname "$0")/.."

PINNED=nightly-2026-06-16 # rustc 1.98.0-nightly (01dfd7924 2026-06-15)
COMMIT=01dfd79246f1b2d5f146616deff08223a840a9ae
TOOLCHAIN=${MADAR_WASM_TOOLCHAIN:-$PINNED}

got=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^commit-hash: //p')
if [ "$got" != "$COMMIT" ]; then
  echo "build-wasm: $TOOLCHAIN is rustc $got; the recipe is pinned to $PINNED ($COMMIT)" >&2
  exit 1
fi

export CHRONO_TZ_TIMEZONE_FILTER="$(cat scripts/tz-filter.txt)"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target/wasm}
# With --target, RUSTFLAGS reach only the wasm, not build scripts or macros.
export RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort"

rm -rf dist
for pkg in public full; do
  cargo +"$TOOLCHAIN" build --locked -p madar-web --features "$pkg" \
    --profile wasm --target wasm32-unknown-unknown \
    -Z build-std=std,panic_abort -Z build-std-features=optimize_for_size
  wasm-bindgen --target web --out-dir "dist/$pkg" \
    "$CARGO_TARGET_DIR/wasm32-unknown-unknown/wasm/madar_web.wasm"
  wasm-opt -Oz --converge --strip-debug --strip-producers \
    "dist/$pkg/madar_web_bg.wasm" -o "dist/$pkg/madar_web_bg.wasm"
done

(cd dist && shasum -a 256 public/* full/* >SHA256SUMS)

# What a browser downloads: wasm + glue, brotli as served.
if command -v brotli >/dev/null; then
  for pkg in public full; do
    raw=0 br=0
    for f in "dist/$pkg/madar_web_bg.wasm" "dist/$pkg/madar_web.js"; do
      raw=$((raw + $(wc -c <"$f")))
      br=$((br + $(brotli -q 11 -c "$f" | wc -c)))
    done
    echo "$pkg: wasm + glue $((raw / 1024)) KB raw, $((br / 1024)) KB brotli"
  done
fi
