#!/usr/bin/env bash
# PNF64 e baseline WAD: constraints, testes, decisões WAD no circuito, G2 em
# binary64 (constraints, Nova curto e scan nas sementes do ataque), num
# diretório NOVO.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_pnf64_wad.sh results/pnf64-wad-AAAAMMDD}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
mkdir -p "$OUT"
cargo build --release -p pnf-bench -p shadow-proof -p wad-baseline
cargo test --release -p pnf-reference -p pnf-circuit -p shadow-proof -p wad-baseline 2>&1 \
  | grep -E "^test |test result" > "$OUT/tests.log"
./target/release/pnf-bench constraints --format b32 --output "$OUT/pnf32_constraints.json" > /dev/null
./target/release/pnf-bench constraints --format b64 --output "$OUT/pnf64_constraints.json" > /dev/null
./target/release/wad-bench constraints --output "$OUT/wad_constraints.json" > /dev/null
./target/release/shadow-bench constraints --format b32 --out "$OUT/shadow_b32_constraints.json" > /dev/null
./target/release/shadow-bench constraints --format b64 --out "$OUT/shadow_b64_constraints.json" > /dev/null
./target/release/shadow-bench nova --format b64 --steps 16 --out "$OUT/shadow_b64_nova_n16.json" > /dev/null
./target/release/wad-bench decisions --r 3.9 --seeds "${SEEDS:-1000}" --nmax 200 --output "$OUT/wad_decisions_r3.9.csv"
./target/release/shadow-bench scan --format b64 --r 3.9 --seeds "${SEEDS:-1000}" --nexp "${NEXP:-12}" \
  --out "$OUT/shadow_scan_b64_r3.9.csv"
echo "ok: $OUT"
