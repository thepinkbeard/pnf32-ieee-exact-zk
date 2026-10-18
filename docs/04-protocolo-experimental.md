# 04 — Protocolo experimental

Fixado antes de rodar a campanha `results/pnf32-20261016/`.

## Ambiente

- macOS, Apple Silicon (aarch64), Rust 1.85.0 (`rust-toolchain.toml`);
- `nova-snark = 0.73.0`, Pallas/Vesta, IPA, `RelaxedR1CSSNARK` (idênticos ao
  baseline de `proof-numeric-zk`);
- f32 nativo do aarch64 como árbitro de hardware (IEEE 754 RNE, sem FTZ por
  padrão no macOS).

## Workload

Mapa logístico na ordem do baseline FP32: `c = 1 - x; p = x·c; y = p·r`, todos
os produtos e somas arredondados (três arredondamentos por passo). Decisão
`y >= 0,5`. Parâmetros: `x0 = 0,1f`, `r ∈ {3,5; 3,7; 3,9}`. As medições Nova
usam `r = 3,7` e 10 folds, para casar com o baseline (registro 0012 do repo
original).

## Medições

| id | comando | métrica |
|---|---|---|
| M-C | `pnf-bench constraints` | constraints por op e por passo, por categoria |
| M-N | `pnf-bench nova --steps 10 --compress --reps 3` | constraints Nova, prover, verificação, compressão, bytes, RSS |
| M-N' | idem `--validate-input` | ablação T2 |
| M-T | `pnf-bench trajectory --r {3.5,3.9} --n 1000` | divergência de bits, flips do predicado, `ε_max` |
| M-S | `pnf-bench sweep` | oráculo contra f32 nativo nos 2^32 padrões: decode/encode, `1-x`, `x·3,9` |
| M-B | `scripts/run_baselines.sh` | FP32, CBD-32 (fusionado/sequencial), CFD no mesmo backend |

Baselines: código intocado de `../proof-numeric-zk`, compilado em
`target-baselines/` e executado com os comandos dos registros 0012/0016/0018.

## Regras

- Diretórios de resultado são novos e datados; o código recusa sobrescrever.
- Constraints são determinísticas. Tempos são médias de poucas rodadas
  (smoke benchmark, sem desenho pareado de 30 rodadas). Esta é uma limitação
  declarada em 06.
- Não se comparam tempos entre máquinas ou backends diferentes.
