# 0012 — PNF64 e baseline WAD

**Data:** 2026-10-22 · **Estado:** concluído

## 1. Objetivo e estado
Medir o custo de mais bits em IEEE (PNF64) e do ponto fixo WAD do Solidity no
mesmo backend, e levar o sombreamento (G2) para binary64 no circuito.
Motivação: no estudo de caso do oráculo, a PNF32 diverge do exato antes do
WAD. Resultado completo em [10](../10-pnf64-e-wad.md).

## 2. Arquivos
- `crates/pnf-reference/src/lib.rs`: `Format::BINARY64`, `from_f64`/`to_f64`, testes binary64.
- `crates/pnf-circuit/src/ops.rs`: `FIELD_SAFE_BITS`; largura de `k` da mul limitada quando
  a margem do binary32 daria wrap; `split_shifted` no hint da mul (o `P << k` passava de 128 bits).
- `crates/pnf-circuit/src/step.rs`: `LogisticCircuit::new_f64`.
- `crates/pnf-circuit/tests/ops_binary64.rs` (novo).
- `crates/pnf-bench/src/main.rs`: `constraints --format b32|b64`.
- `crates/wad-baseline/` (novo): circuito WAD, `wad-bench constraints|decisions`, testes.
- `crates/shadow-proof/src/{circuit,bench,main}.rs`: circuito e bench genéricos no formato
  (`--format b64`); `tests/circuit.rs` com casos binary64.
- `crates/dual-proof/src/main.rs`: `coverage --attack-seeds` (feito para o estudo de caso).
- `scripts/run_pnf64_wad.sh`, `docs/10-pnf64-e-wad.md`, `results/pnf64-wad-20261022/`.

## 3. Decisões
- WAD com piso (`mulWad` de Solady/PRBMath), divisão testemunhada com resto em [0, 10^18).
- Binary32 intocado: toda mudança na PNF é condicionada ao formato; contagens conferidas
  antes e depois (415 e 1.035, idênticas).
- Grade do G2 em binary64: 2^-96 (a mesma do scan off-circuit de 0011).

## 4. Comandos
```bash
bash scripts/run_pnf64_wad.sh results/pnf64-wad-20261022
```

## 5. Evidências
- Constraints/passo: PNF32 415, WAD 431, PNF64 677; com G2: 1.035 (b32), 1.885 (b64).
- PNF64 bit-exata ao oráculo e ao f64 nativo (testes de 2); adulteração de hints rejeitada.
- Circuito WAD = `mulWad` de referência; 201.201 decisões iguais ao Python do estudo de caso.
- G2 binary64: 1.001/1.001 sementes certificadas até N = 4.096, ε mediano 6,5·10⁻¹⁴.
- 51 testes passam (`tests.log`), incluindo os antigos.

## 6. Tentativas malsucedidas
- `prod << k` em `u128` estourava no binary64 (overflow-checks pegou); trocado por `split_shifted`.
- A largura de `k` herdada do binary32 (`bitlen(K+p−1)` = 8 bits no binary64) permitiria
  `P·2^k` acima do corpo; reduzida a `bitlen(K)` só quando necessário.
- Teste de `to_wad(0.1)` esperava 10^17; o piso correto é 100000000000000005 (0,1 em f64
  é 0,1000000000000000055…).

## 7. Limitações
Ver 10 §5 (WAD + G2 não feito, só logístico, Nova smoke, largura de `k` argumentada,
scan b64 até 4.096).

## 8. Próximo passo
Tempo de prova com S passos por fold para PNF32, PNF64 e WAD, pareado (H5 do estudo de
caso); decidir se vale um G2 em grade decimal para o WAD.

## 9. Continuidade para outra IA
Não afirmar que a PNF é mais precisa ou mais barata que o WAD: não é. O argumento é
compatibilidade bit-exata com código float existente (binary64 em numpy).
