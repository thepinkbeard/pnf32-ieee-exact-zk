#!/usr/bin/env bash
# Campanha do protótipo de sombreamento (G2) num diretório NOVO.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_shadow_campaign.sh results/shadow-AAAAMMDD}"
cargo build --release -p shadow-proof
B=./target/release/shadow-bench
ATK=../rounding-steering-attack/results/campaign-20261016
for r in 3.7 3.8 3.9 4.0; do
  $B scan --format b32 --r $r --seeds "${SEEDS32:-200}" --nexp 16 --out "$OUT/scan_b32_r$r.csv"
  $B scan --format b64 --r $r --seeds "${SEEDS64:-100}" --nexp 16 --out "$OUT/scan_b64_r$r.csv"
done
for w in witness_f32_d24_r3.9 witness_f32_d23_r3.9 witness_f32_d24_r4.0 witness_f64_d53_r3.9 witness_f64_d52_r3.9; do
  $B attack --witness "$ATK/$w.json" --out "$OUT/attack_$w.json" > /dev/null
done
$B nova --steps 16 --out "$OUT/nova_n16.json" > /dev/null
python3 scripts/summarize_shadow.py "$OUT" | tee "$OUT/summary.md"
echo "ok: $OUT"
