# 0003 — Circuito PNF32 e unicidade exaustiva

**Data:** 2026-10-16 · **Estado:** concluído; H4 confirmada após correção

## 1. Objetivo e estado
Implementar mul, add/sub, decisão e o StepCircuit Nova com binary32 geral, e
verificar soundness por enumeração do espaço inteiro de witnesses nos formatos
mini.

## 2. Arquivos
`crates/pnf-circuit/src/{gadgets,pnf,ops,step,lib}.rs`,
`crates/pnf-circuit/tests/{ops_binary32,exhaustive_uniqueness}.rs`.

## 3. Decisões
- **Mudança em relação ao plano:** em vez de expoente estendido com
  subnormais normalizados, a implementação guarda **os campos IEEE nativos**
  (bit implícito explícito). O quociente com shift testemunhado
  `P·2^k = Q·2^K + R` já absorve entradas subnormais, e a saída subnormal não
  precisa de renormalização (cerca de 30 constraints a menos por operação). A
  decodificação para bits fica linear.
- Hints = variáveis de escolha (`k`, `ovf`; na soma, `swap, big, neg, k, ovf`).
  Todo o resto é determinístico (lema U da spec). Isso torna a enumeração
  exaustiva uma verificação completa, e não uma amostragem.
- `CheckCS`: constraint system próprio que avalia cada constraint na hora
  (rápido o bastante para cerca de 18 M sínteses). É cruzado com o
  `TestConstraintSystem` do Nova nos testes.

## 4. Comandos
```bash
cargo test -p pnf-circuit --release --test ops_binary32
cargo test -p pnf-circuit --release --test exhaustive_uniqueness -- --include-ignored --nocapture
```

## 5. Evidências
- binary32: 200 mil pares aleatórios × 3 ops, 200 mil pares de near path, casos
  difíceis, 300 trajetórias logísticas: bit a bit iguais ao oráculo/nativo,
  e toda saída passa por `validate`.
- Adversarial binary32: para 300 pares, todos os 256 hints de mul e os 1.024
  de add. Todo hint aceito produz a saída correta.
- **Exaustivo:** `(3,3)` mul 111.392 e add 891.136 witnesses; `(4,4)` mul
  1.889.568 e add 15.116.544 witnesses; decisão em todos os valores e
  limiares. Unicidade OK.

## 6. Tentativas malsucedidas — bug de soundness achado
A primeira versão do `add` passou em **todos** os testes binary32 (inclusive o
adversarial amostrado) e **falhou** na enumeração exaustiva `(3,3)`:
`8 + (-6) = 2` aceitava também a saída `+0`. Mecanismo: o provador escolhia um
`dexp` grande, `Q` virava 0, `zf = 1` liberava a exceção de zero da regra de
quantum, e um resultado não nulo era "arredondado" para zero. Correção (1
constraint): `zf·|X| = 0`, porque a soma IEEE com underflow gradual nunca
arredonda para zero. A mul não tem o problema: `dexp = K` com `t = 0` exige
`w >= K` pela regra `(2t-1)(dexp-w) >= 0`.

Lição metodológica: testes amostrados, mesmo adversariais, não acharam o bug,
e a enumeração completa num formato pequeno achou em segundos. Isso reforça a
contribuição 3 de 01-literatura.

## 7. Limitações
Unicidade exaustiva só nos formatos mini. Para binary32, vale pelo mesmo
código paramétrico mais o argumento manual (lema U) e os testes amostrados.
Falta uma prova mecanizada.

## 8. Próximo passo
Medições (0004).

## 9. Continuidade para outra IA
Qualquer mudança em `ops.rs` deve rodar o exaustivo `--include-ignored` antes
de qualquer medição.
