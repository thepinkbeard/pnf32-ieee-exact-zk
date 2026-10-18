#!/usr/bin/env bash
# Perfil por fase de um fold Nova (log 0006), com a cópia INSTRUMENTADA de
# nova-snark (vendor/nova-snark-0.73.0-prof). Diretório NOVO, nunca sobrescreve.
# Em cada repetição, as configurações {pnf32, fp32, cbd32} × S rodam em ordem
# aleatória (semente fixa); um processo por medição.
#
# uso: profiling/run_profile.sh results/profile-AAAAMMDD
#   REPS=10 FOLDS=8 SEED=6 SPF="1 4 16 64 256 1024"
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:?uso: profiling/run_profile.sh results/profile-AAAAMMDD}"
[ -e "$OUT" ] && { echo "já existe: $OUT" >&2; exit 1; }
REPS="${REPS:-10}"; FOLDS="${FOLDS:-8}"; SEED="${SEED:-6}"
read -r -a SPF <<< "${SPF:-1 4 16 64 256 1024}"
mkdir -p "$OUT/raw" "$OUT/bin"
(cd profiling && cargo build --release)
cp profiling/target/release/spf-profile "$OUT/bin/"
BIN="$OUT/bin/spf-profile"
{
  echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m), $(sysctl -n hw.ncpu 2>/dev/null) cpus"
  echo "backend: nova-snark 0.73.0 instrumentado (vendor/nova-snark-0.73.0-prof)"
  echo "reps: $REPS  timed folds: $FOLDS  steps_per_fold: ${SPF[*]}  seed: $SEED"
  shasum -a 256 "$BIN"
} > "$OUT/manifest.txt"
echo "rep,slot,repr,steps_per_fold,start_utc" > "$OUT/order.csv"
for rep in $(seq 0 $((REPS - 1))); do
  configs=()
  for s in "${SPF[@]}"; do for r in pnf32 fp32 cbd32; do configs+=("$r:$s"); done; done
  order=$(python3 -c "import random,sys; c=sys.argv[3:]; random.Random(int(sys.argv[1])*1000+int(sys.argv[2])).shuffle(c); print(' '.join(c))" "$SEED" "$rep" "${configs[@]}")
  slot=0
  for c in $order; do
    r="${c%%:*}"; s="${c##*:}"
    echo "$rep,$slot,$r,$s,$(date -u +%H:%M:%S)" >> "$OUT/order.csv"
    "$BIN" --repr "$r" --steps-per-fold "$s" --folds "$FOLDS" \
      --output "$OUT/raw/${r}_s${s}_rep${rep}.json" > /dev/null 2>> "$OUT/run.log"
    slot=$((slot + 1))
  done
  echo "rep $rep ok $(date -u +%H:%M:%S)"
done
python3 scripts/analyze_profile.py "$OUT"
echo "ok: $OUT"
