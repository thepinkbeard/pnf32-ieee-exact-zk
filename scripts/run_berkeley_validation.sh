#!/usr/bin/env bash
# Valida pnf-reference e o circuito PNF32 contra Berkeley SoftFloat (C) num host
# x86_64 Linux emulado (Docker/OrbStack com --platform linux/amd64).
# A compilação sob emulação leva ~8 min; os testes, segundos.
set -euo pipefail
cd "$(dirname "$0")/.."
exec docker run --rm --platform linux/amd64 -v "$PWD":/work -w /work rust:1.85-bookworm bash -c '
  set -e
  apt-get update -qq >/dev/null && apt-get install -y -qq cmake >/dev/null
  export CARGO_TARGET_DIR=/tmp/target
  cargo test -p berkeley-validation --release -- --nocapture
'
