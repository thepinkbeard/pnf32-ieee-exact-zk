#!/usr/bin/env bash
# Ponto fixo diádico fundido: testes, constraints por passo, decisões no
# circuito (sementes do ataque) e comparação com o h1, num diretório NOVO.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_dyadic_fused.sh results/dyadic-fused-AAAAMMDD}"
H1="${H1:-../oracle-case-study/results/h1-20261022/decisions.csv}"
PS="${PS:-32 64 96 110}"
RS="${RS:-3.5 3.7 3.9 4.0}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
mkdir -p "$OUT"
cargo build --release -p dyadic-fused -p wad-baseline -p pnf-bench
cargo test --release -p dyadic-fused -p wad-baseline 2>&1 | grep -E "^test |test result" > "$OUT/tests.log"
./target/release/dyadic-bench constraints --ps "$(echo $PS | tr ' ' ,)" --output "$OUT/dyadic_constraints.json"
for p in $PS; do
  for r in $RS; do
    ./target/release/dyadic-bench decisions --p "$p" --r "$r" --seeds "${SEEDS:-1000}" --nmax 200 \
      --output "$OUT/dyadic_p${p}_r${r}.csv"
  done
done
./target/release/dyadic-bench nova --p 64 --steps 16 --output "$OUT/dyadic_p64_nova_n16.json" > /dev/null
./target/release/dyadic-bench nova --p 110 --steps 16 --output "$OUT/dyadic_p110_nova_n16.json" > /dev/null
python3 scripts/compare_dyadic_h1.py "$H1" "$OUT/summary.md" "$OUT"/dyadic_p*_r*.csv
echo "ok: $OUT"
