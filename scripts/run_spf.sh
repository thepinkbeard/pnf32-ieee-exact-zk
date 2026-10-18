#!/usr/bin/env bash
# Campanha steps-per-fold (log 0005) num diretório NOVO (nunca sobrescreve).
#
# 1. testes (workspace + unicidade exaustiva) — aborta se algum falhar;
# 2. desenho pareado: em cada repetição, as 12 configurações
#    {pnf32, fp32, cbd32} × S ∈ {1,4,16,64} rodam em ordem aleatória
#    (semente fixa), um processo por medição, mesmo total de passos logísticos;
# 3. análise (scripts/analyze_spf.py) -> summary.json / summary.md.
#
# uso: scripts/run_spf.sh results/pnf32-spf-AAAAMMDD   (REPS=30 STEPS=512 SEED=5)
#   SPF="1 4 16 64 256 1024"  valores de S (padrão: 1 4 16 64, como no 0005)
#   FOLDS=9                   folds fixos por configuração (passos = FOLDS·S),
#                             em vez de STEPS fixo (log 0006)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: run_spf.sh results/pnf32-spf-AAAAMMDD}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
REPS="${REPS:-30}"; STEPS="${STEPS:-512}"; SEED="${SEED:-5}"
read -r -a SPF <<< "${SPF:-1 4 16 64}"
FOLDS="${FOLDS:-}"
mkdir -p "$OUT/raw"

{
  echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m), $(sysctl -n hw.ncpu 2>/dev/null) cpus"
  echo "rustc: $(rustc --version)"
  if [ -n "$FOLDS" ]; then steps_desc="FOLDS*S (FOLDS=$FOLDS)"; else steps_desc="$STEPS"; fi
  echo "reps: $REPS  steps: $steps_desc  steps_per_fold: ${SPF[*]}  seed: $SEED"
  echo "r: 3.7 (pnf32/fp32 f32; cbd32 37/10)  x0: 0.1 (cbd32 1/10)  threshold: 1/2"
} > "$OUT/manifest.txt"

if [ "${SKIP_TESTS:-0}" = 1 ]; then echo "SKIP_TESTS=1 (apenas smoke test)" | tee -a "$OUT/manifest.txt" > "$OUT/tests.log"; else
echo "== testes" | tee "$OUT/tests.log"
cargo test --workspace --release 2>&1 | grep -E "^test |test result|panicked|error" | tee -a "$OUT/tests.log"
cargo test -p pnf-circuit --release --test exhaustive_uniqueness -- --include-ignored 2>&1 \
  | grep -E "^test |test result|panicked|error" | tee -a "$OUT/tests.log"
if grep -qE "FAILED|panicked|^error" "$OUT/tests.log"; then echo "testes falharam" >&2; exit 1; fi
fi

cargo build --release -p pnf-bench -p spf-baselines
# Cópias congeladas: outra sessão pode recompilar target/ durante a campanha.
mkdir -p "$OUT/bin"
cp ./target/release/pnf-bench ./target/release/spf-baselines "$OUT/bin/"
PNF="$OUT/bin/pnf-bench"
BASE="$OUT/bin/spf-baselines"
shasum -a 256 "$PNF" "$BASE" >> "$OUT/manifest.txt"

echo "rep,slot,repr,steps_per_fold,start_utc" > "$OUT/order.csv"
for rep in $(seq 0 $((REPS - 1))); do
  configs=()
  for s in "${SPF[@]}"; do for r in pnf32 fp32 cbd32; do configs+=("$r:$s"); done; done
  # Ordem aleatória reprodutível dentro do bloco (repetição).
  order=$(python3 -c "import random,sys; c=sys.argv[3:]; random.Random(int(sys.argv[1])*1000+int(sys.argv[2])).shuffle(c); print(' '.join(c))" "$SEED" "$rep" "${configs[@]}")
  slot=0
  for c in $order; do
    r="${c%%:*}"; s="${c##*:}"
    f="$OUT/raw/${r}_s${s}_rep${rep}.json"
    n="${FOLDS:+$((FOLDS * s))}"; n="${n:-$STEPS}"
    echo "$rep,$slot,$r,$s,$(date -u +%H:%M:%S)" >> "$OUT/order.csv"
    if [ "$r" = pnf32 ]; then
      "$PNF" nova --steps "$n" --steps-per-fold "$s" --compress --output "$f" > /dev/null 2>> "$OUT/run.log"
    else
      "$BASE" --repr "$r" --steps "$n" --steps-per-fold "$s" --compress --output "$f" > /dev/null 2>> "$OUT/run.log"
    fi
    slot=$((slot + 1))
  done
  echo "rep $rep ok $(date -u +%H:%M:%S)"
done

python3 scripts/analyze_spf.py "$OUT"
echo "ok: $OUT"
