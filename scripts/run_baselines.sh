#!/usr/bin/env bash
# Reproduz os baselines de proof-numeric-zk (FP32 restrito, CBD, CFD) no mesmo
# backend Nova 0.73, gravando os JSONs num diretório NOVO desta pasta.
# Não modifica proof-numeric-zk: o build vai para um target dir próprio.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:?uso: run_baselines.sh results/baselines-AAAAMMDD}"
OUT="$HERE/$OUT"
if [ -e "$OUT" ]; then echo "diretório já existe: $OUT" >&2; exit 1; fi
mkdir -p "$OUT"
export CARGO_TARGET_DIR="$HERE/target-baselines"
cd "$HERE/../proof-numeric-zk"
cargo build --release -p nova-circuit
BIN="$CARGO_TARGET_DIR/release/nova-circuit"
STEPS="${STEPS:-10}"
run() { local name="$1"; shift; echo "== $name"; "$BIN" --steps "$STEPS" --compress --output "$OUT/$name.json" "$@"; }
run fp32            --k 2 --rounding nearest-even --representation fp32
run cbd32-fused     --k 2 --rounding nearest-even --representation cbd
run cbd32-seq       --k 2 --rounding nearest-even --representation cbd-sequential
run cfd             --k 1 --rounding nearest-even --representation cfd
echo "ok: $OUT"
