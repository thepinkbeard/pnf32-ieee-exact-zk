# 0001 — Andaime e reprodução dos baselines

**Data:** 2026-10-16 · **Estado:** concluído

## 1. Objetivo e estado
Criar a pasta independente do experimento e reproduzir, no ambiente atual, os
custos de referência do mesmo backend (Nova 0.73). Os quatro números batem
exatamente com os registros do repositório original.

## 2. Arquivos
`Cargo.toml`, `rust-toolchain.toml`, `Cargo.lock` (copiado de `proof-numeric-zk`
para fixar versões), `.gitignore`, `scripts/run_baselines.sh`,
`results/baselines-20261016/*.json`, `results/baselines-20261016.log`.

## 3. Decisões e hipóteses
- Pasta irmã `../pnf32-ieee-exact-zk`, sem modificar `proof-numeric-zk`. Os
  baselines são compilados a partir do código original num target dir próprio
  (`target-baselines/`).
- Workspace leve: o crate novo não depende de `rug`/GMP.

## 4. Comandos
```bash
bash scripts/run_baselines.sh results/baselines-20261016
# CFD exige --k 1 (corrigido no script depois da primeira execução):
target-baselines/release/nova-circuit --steps 10 --compress --k 1 \
  --rounding nearest-even --representation cfd --output results/baselines-20261016/cfd.json
```

## 5. Evidências
| baseline | raw step | Nova primário | prover 10 folds | prova |
|---|---:|---:|---:|---:|
| FP32 exato (positivo-normal) | 1.721 | 13.097 | 0,397 s | 10.720 B |
| CBD-32 fusionado | 709 | 12.086 | 0,446 s | 10.752 B |
| CBD-32 sequencial | 782 | 12.159 | 0,458 s | 10.752 B |
| CFD (p=24, 9 lanes) | 1.003 | — | — | — |

## 6. Tentativas malsucedidas
A primeira execução da CFD falhou com "CFD usa --k 1": o script passava o
`--k 2` padrão. Corrigido.

## 7. Limitações
Tempos de uma rodada só (smoke). As constraints são determinísticas.

## 8. Próximo passo
Referência off-circuit genérica e oráculo exato (0002).

## 9. Continuidade para outra IA
Os baselines são imutáveis. Se quiser recompará-los, rode o script num diretório
de resultados novo.
