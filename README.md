# PNF32: IEEE 754 binary32 bit-exato com menos constraints

Experimento derivado do paper *Impact of Numeric Representation on Verifiable
Semantics in Zero-Knowledge Blockchain Systems*. O paper mostra que
representações baratas divergem do FP32 no mapa logístico caótico. Aqui
investigamos se é possível provar **o próprio IEEE 754**, bit a bit, gastando
menos constraints que as alternativas aproximadas.

## Resultado em uma tabela

Mesmo backend (Nova 0.73, Pallas/Vesta, R1CS), passo do mapa logístico
(`1-x`, `x·c`, `p·r`) + decisão `x >= 0,5`:

| representação | fidelidade a FP32 | escopo | constraints/passo |
|---|---|---|---:|
| FP32 exato (baseline `proof-numeric-zk`) | bit-exata | só positivo-normal | 1.721 |
| CFD v1 | enclosure, não bit-exata | positivo | 1.003 |
| CBD-32 fusionado | enclosure, não bit-exata | [0,1] | 709 |
| **PNF32 (este trabalho)** | **bit-exata** | **binary32 geral** (±0, subnormais, ±inf, NaN) | **415** |

Operações isoladas (binary32 geral): **mul 134**, **add/sub 145**, `1-x` 141,
`x·r` 127, decisão 11. O FP32 do baseline custava 474 (mul) e 924 (`1-x`).

Fidelidade: trajetórias de 1.000 passos em r=3,5 e r=3,9 **bit-idênticas** ao
f32 nativo (0 flips do predicado, `ε_max = 0`). Para comparar: no paper,
Q16.16, Q8.8, quantização e Approx-FP têm de 444 a 491 flips em r=3,9.

Soundness: para todo par de entradas nos formatos mini `(3,3)` e `(4,4)`, a
enumeração de **todos** os witnesses (cerca de 18 milhões) aceita exatamente uma
saída, a correta. Essa enumeração achou e permitiu corrigir um bug real de
soundness que os testes amostrados não pegavam (registro 0003).

Detalhes, ressalvas e o que **não** melhorou (tempo de prova em 1 passo/fold,
dominado pelo overhead fixo do Nova): [docs/06-resultados.md](docs/06-resultados.md).

## Como a representação funciona (resumo)

A semântica é exatamente a do binary32. O que muda é a codificação e as
relações provadas:

- **Estado desempacotado** `(s, e-emin, m com bit implícito, z, inf, nan)`. O word
  de 32 bits só existe nas fronteiras.
- **Validade como invariante indutivo do IVC**: saídas são válidas por
  construção, e só `z0` é checado. Entradas herdadas não são re-checadas.
- **Quociente com corte fixo e shift testemunhado**: `P·2^k = Q·2^K + R`. Um
  único mecanismo cobre normalização, subnormais e underflow; o bit de topo
  de `Q` fixa o caminho.
- **Alinhamento limitado (far/near path)** na soma: diferença de expoentes
  `>= p+3` vira um bit sticky. Nada de alinhar à escala `2^-149`.
- **RNE por guard/sticky/LSB** saídos da mesma decomposição; exceções como
  álgebra de flags.

Especificação completa: [docs/02-especificacao-pnf32.md](docs/02-especificacao-pnf32.md).

## Mapa da pasta

```
crates/pnf-reference/   valor PNF genérico (ebits,p), bijeção com bits, oráculo exato
crates/pnf-circuit/     gadgets R1CS, mul/add/sub/decisão, StepCircuit Nova
  tests/ops_binary32.rs           bit-exatidão contra oráculo/nativo, adversarial
  tests/exhaustive_uniqueness.rs  unicidade de witness em formatos mini
crates/pnf-bench/       medições: constraints, nova, trajectory, sweep
tools/berkeley-validation/  validação contra Berkeley SoftFloat (C), via Docker
scripts/                run_baselines.sh, run_all.sh, run_berkeley_validation.sh
results/                uma pasta datada por campanha (nunca sobrescrita)
docs/                   00 pergunta · 01 literatura · 02 spec · 03 custo ·
                        04 protocolo · 05 validação · 06 resultados · progress/
```

## Comandos

```bash
cargo test --workspace --release                       # tudo menos o exaustivo grande
cargo test -p pnf-circuit --release --test exhaustive_uniqueness -- --include-ignored
bash scripts/run_all.sh results/pnf32-AAAAMMDD          # campanha completa num diretório novo
bash scripts/run_baselines.sh results/baselines-AAAAMMDD  # FP32/CBD/CFD do repo original
bash scripts/run_berkeley_validation.sh                # requer Docker/OrbStack ativo
```

## Para continuar

Leia [docs/progress/README.md](docs/progress/README.md) e o último registro. O
primeiro item da fila é o tempo de prova com vários passos por fold e a
validação Berkeley. A lista completa está em
[docs/06-resultados.md](docs/06-resultados.md#próximos-passos).

## Dependências externas

Os crates `dual-proof` e `spf-baselines` e o script `scripts/run_baselines.sh`
usam o repositório irmão `proof-numeric-zk` (CBD/CDAD e o baseline FP32)
por caminho relativo: `../proof-numeric-zk`. Para compilar o workspace inteiro,
clone os dois lado a lado. `pnf-reference`, `pnf-circuit`, `pnf-bench` e
`shadow-proof` não dependem dele. Os PDFs lidos no original não são
versionados; veja [`docs/refs/README.md`](docs/refs/README.md).
