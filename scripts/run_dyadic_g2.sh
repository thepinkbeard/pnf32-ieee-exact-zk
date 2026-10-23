#!/usr/bin/env bash
# G2 sobre o perfil diádico fundido: testes, constraints por fold, scan de
# certificação nas sementes do ataque, horizonte longo e Nova, num diretório NOVO.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_dyadic_g2.sh results/dyadic-g2-AAAAMMDD}"
PROFILES="${PROFILES:-20:28,24:32,32:40,48:56,64:72,96:104,110:113}"
SAME_GRID="24:48,32:48"
RS="${RS:-3.7 3.9 4.0}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
mkdir -p "$OUT"
cargo build --release -p dyadic-fused -p shadow-proof
cargo test --release -p dyadic-fused -p shadow-proof 2>&1 | grep -E "^test |test result" > "$OUT/tests.log"
./target/release/dyadic-bench shadow-constraints --profiles "$PROFILES,$SAME_GRID" --output "$OUT/g2_constraints.json"
for r in $RS; do
  ./target/release/dyadic-bench shadow-scan --profiles "$PROFILES" --r "$r" --seeds "${SEEDS:-1000}" \
    --nexp "${NEXP:-12}" --output "$OUT/g2_scan_r${r}.csv"
done
# horizonte longo no perfil candidato
./target/release/dyadic-bench shadow-scan --profiles 32:40 --r 3.9 --seeds "${SEEDS:-1000}" --nexp 16 \
  --output "$OUT/g2_scan_long_p32_r3.9.csv"
./target/release/dyadic-bench shadow-nova --p 32 --k 40 --widths tight --output "$OUT/g2_p32_k40_tight_nova_n16.json" > /dev/null
./target/release/dyadic-bench shadow-nova --p 32 --k 40 --widths full --output "$OUT/g2_p32_k40_full_nova_n16.json" > /dev/null
./target/release/dyadic-bench shadow-nova --p 64 --k 72 --widths tight --output "$OUT/g2_p64_k72_tight_nova_n16.json" > /dev/null
python3 scripts/summarize_dyadic_g2.py "$OUT"
echo "ok: $OUT"
