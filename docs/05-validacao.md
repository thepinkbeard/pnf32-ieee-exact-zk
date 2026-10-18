# 05 — Validação de bit-exatidão e soundness

Três árbitros independentes do gerador de witness do circuito:

1. **f32 nativo** (aarch64, IEEE 754 RNE, subnormais ligados);
2. **oráculo exato** `pnf-reference` (valor exato em `BigInt`, RNE único);
3. **Berkeley SoftFloat 3 (C)**: ferramenta pronta em `tools/berkeley-validation`,
   **ainda não executada** (requer Docker x86_64).

## Cadeia de evidência

| alvo | contra | escopo | resultado |
|---|---|---|---|
| decode/encode | definição IEEE | **todos os 2^32 padrões** | 0 divergências (`sweep_2pow32.json`, 587 s) |
| oráculo `1-x`, `x·3,9` | f32 nativo | **todos os 2^32 padrões** | 0 divergências |
| oráculo add/sub/mul | f32 nativo | 6 M ops com viés (subnormais, especiais, bordas, cancelamento) + 22² casos difíceis | 0 divergências |
| oráculo logístico | f32 nativo | 1.000 passos, r ∈ {3,5; 3,7; 3,9; 4,0} | 0 divergências |
| circuito add/sub/mul | oráculo | 200 mil pares aleatórios × 3 ops; 200 mil de near path; 27² × 3 casos difíceis | 0 divergências; todas as saídas válidas |
| circuito × `TestConstraintSystem` | CheckCS | 200 pares × 2 ops | satisfeito nos dois |
| circuito adversarial binary32 | oráculo | 300 pares × todos os hints (256 mul, 1.024 add) | todo hint aceito dá a saída correta |
| circuito logístico | f32 nativo | 1.000 passos, r=3,5 e 3,9 | 0 divergências, 0 flips |
| Nova IVC | f32 nativo | 10 folds, estado final e decisão | iguais |
| **unicidade exaustiva** | oráculo | todos os pares, todos os hints: `(3,3)` e `(4,4)` | exatamente 1 saída aceita |
| decisão exaustiva | oráculo | todos os valores × limiares em `(4,4)` | exatamente 1 resposta aceita |

## Por que o exaustivo é uma verificação, e não uma amostra

O lema U (spec §7) mostra que toda variável que não é hint é determinada pelas
entradas e pelos hints. Logo, enumerar os hints enumera o conjunto inteiro de
witnesses satisfatórios. Num formato pequeno, isso cobre **todas** as entradas.
A afirmação para binary32 depende de o código ser paramétrico (é o mesmo) e de
as larguras escalarem corretamente (fórmulas em `Widths::of`).

## Achados

- **Bug de soundness (corrigido):** veja [0003](progress/0003-circuito-pnf32.md).
  Os testes binary32 amostrados, inclusive os adversariais, não o acharam. O
  exaustivo `(3,3)` achou.
- **Crate `softfloat` 1.0 (Rust) incorreto em subnormais:** veja
  [0002](progress/0002-referencia-e-oraculo.md).
