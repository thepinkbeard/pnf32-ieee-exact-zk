# 0002 — Referência PNF genérica e oráculo exato

**Data:** 2026-10-16 · **Estado:** concluído

## 1. Objetivo e estado
Implementar o valor canônico PNF genérico em `(ebits, p)`, a bijeção com
padrões de bits e um **oráculo de arredondamento único** (valor exato em
inteiros grandes, RNE uma vez), independente do gerador de witness do
circuito.

## 2. Arquivos
`crates/pnf-reference/src/lib.rs`.

## 3. Decisões
- NaN canônico `0x7FC00000` (especialização default-NaN). IEEE não
  especifica payload; a comparação trata NaN por classe.
- Oráculo: todo finito é `N·2^qmin`; soma e produto exatos em `BigInt`;
  `round_finite` arredonda uma vez, com carry e overflow para inf.
- Árbitros: **f32 nativo (aarch64)** e o oráculo exato. Berkeley SoftFloat (C)
  fica preparado para rodar em Docker x86_64 (`tools/berkeley-validation`).

## 4. Comandos
```bash
cargo test -p pnf-reference --release -- --nocapture
```

## 5. Evidências
- 6 milhões de operações (add/sub/mul, com viés para subnormais, especiais,
  bordas e cancelamento) bit a bit iguais ao f32 nativo.
- Casos difíceis (22×22×3), trajetórias logísticas de 1.000 passos em
  r=3,5/3,7/3,9/4,0, bijeção exaustiva nos formatos `(3,3)`, `(4,4)` e binary16.

## 6. Tentativas malsucedidas — achado relevante
O crate Rust puro **`softfloat` 1.0.0** (já usado como dev-dependency em
`proof-numeric-zk/crates/numeric-reference`) **está errado em subnormais**:
divergiu do hardware em 12.851 de 6.000.000 operações. Contraexemplo fixo,
travado num teste:
`-1.825465e-39 × 1.8195308e-6`: correto `0x80000002` (nativo e oráculo);
o crate devolve `0x80000003`. **Consequência para o repo original:** qualquer
validação que use esse crate como árbitro em subnormais é suspeita. O baseline
FP32 de lá é positivo-normal e foi validado contra Berkeley em Docker
(registro 0022 de lá), então não é afetado.

## 7. Limitações
Berkeley SoftFloat (C) ainda não rodou neste projeto: o daemon Docker/OrbStack
não estava ativo. O script está pronto: `scripts/run_berkeley_validation.sh`.

## 8. Próximo passo
Gadgets do circuito (0003).

## 9. Continuidade para outra IA
Não use o crate `softfloat` como árbitro. Use o nativo, o oráculo, ou Berkeley via Docker.
