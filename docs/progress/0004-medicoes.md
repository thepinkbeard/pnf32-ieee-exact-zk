# 0004 — Medições e veredito

**Data:** 2026-10-16 · **Estado:** concluído; **go** (H1–H4 confirmadas)

## 1. Objetivo e estado
Medir constraints, Nova IVC, trajetórias e a varredura 2^32; decidir go/no-go.

## 2. Arquivos
`crates/pnf-bench/src/main.rs`, `scripts/run_all.sh`,
`tools/berkeley-validation/`, `scripts/run_berkeley_validation.sh`,
`results/pnf32-20261016/*`, `docs/00`–`06`, `README.md`.

## 3. Decisões
- Nova com r=3,7, 10 folds e compressão, para casar com o baseline 0012.
- `r` constante no circuito, como no baseline. Ablação com `r` variável medida.

## 4. Comandos
```bash
B=target/release/pnf-bench; R=results/pnf32-20261016
$B constraints --output $R/constraints.json
$B trajectory --r 3.5 --n 1000 --output $R/trajectory_r3.5.csv
$B trajectory --r 3.9 --n 1000 --output $R/trajectory_r3.9.csv
$B nova --steps 10 --compress --reps 3 --output $R/nova_n10.json
$B nova --steps 10 --compress --validate-input --output $R/nova_n10_validate.json
$B sweep --output $R/sweep_2pow32.json
cargo test --workspace --release
```

## 5. Evidências
- Passo: **415** (FP32 1.721; CFD 1.003; CBD-32 709). mul 134, add 145.
- Nova primário 11.794 (FP32 13.097). Prover 0,437 s / 10 folds (FP32 0,397 s, 1 rodada).
- Trajetórias r=3,5 e 3,9: 0 bits divergentes, 0 flips.
- Varredura 2^32: decode/encode, `1-x` e `x·3,9`: 0 divergências.
- Suíte completa verde após a correção do 0003 (`sweep_and_tests.log`).

## 6. Tentativas malsucedidas
O resumo das trajetórias saiu só no stdout na primeira execução; ele foi
reconstruído dos CSVs em `trajectory_summary.jsonl`. `run_all.sh` já grava com `tee`.

## 7. Limitações
Ver 06 §5: tempo não melhorou com 1 passo/fold; Berkeley não executado;
ablações T3–T7 isoladas não implementadas; só add/sub/mul.

## 8. Próximo passo
`steps_per_fold` + medição pareada de 30 rodadas; Berkeley em Docker.

## 9. Continuidade para outra IA
Comece por 06 §Próximos passos, item 1. Para medir FP32/CBD com vários
passos por fold seria preciso alterar o código de `proof-numeric-zk`; faça
isso numa cópia ou num crate desta pasta, nunca no original.
