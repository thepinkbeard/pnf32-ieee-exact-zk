#!/usr/bin/env bash
# Campanha completa da PNF32 num diretório NOVO (nunca sobrescreve).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_all.sh results/pnf32-AAAAMMDD}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
cargo build --release -p pnf-bench
B=./target/release/pnf-bench
$B constraints --output "$OUT/constraints.json"
for r in 3.5 3.9; do $B trajectory --r $r --n 1000 --output "$OUT/trajectory_r$r.csv" | tee -a "$OUT/trajectory_summary.jsonl"; done
$B nova --steps 10 --compress --reps 3 --output "$OUT/nova_n10.json" > /dev/null
$B nova --steps 10 --compress --validate-input --output "$OUT/nova_n10_validate.json" > /dev/null
$B sweep --output "$OUT/sweep_2pow32.json"
cargo test --workspace --release 2>&1 | grep -E "^test |test result" | tee "$OUT/tests.log"
echo "ok: $OUT"
