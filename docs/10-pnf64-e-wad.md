# 10 — PNF64 e baseline WAD

**Data:** 2026-10-22 · Campanha: `results/pnf64-wad-20261022/` ·
Registro: [0012](progress/0012-pnf64-e-wad.md)

Pergunta: o estudo de caso do oráculo (`../oracle-case-study`) mostrou que a
PNF32 diverge da matemática exata antes do WAD (24 contra ~60 bits). Quanto
custa ter mais bits em IEEE (PNF64), quanto custa o WAD no mesmo backend, e o
sombreamento (G2) funciona em binary64 no circuito?

## 1. Constraints por passo (mapa logístico + decisão, Nova 0.73, Pallas)

| representação | bits de precisão em [0,1] | constraints/passo | 1º passo divergente (r=3,9, mediana) | garantia retroativa (ε mediano, N=4.096) | semântica |
|---|---:|---:|---:|---:|---|
| PNF32 | 24 | **415** | 31 | nenhuma | IEEE binary32 bit-exato |
| WAD (`mulWad` piso) | ~60 | **431** | 78 | nenhuma | ponto fixo decimal do Solidity |
| PNF64 | 53 | **677** | 72 | nenhuma | IEEE binary64 bit-exato |
| FP32 exato (baseline antigo) | 24 | 1.721 | 31 | nenhuma | IEEE binary32, só positivo-normal |
| PNF32 + G2 | 24 | 1.035 | 31 (igual à PNF32) | 8·10⁻⁶ | L2 + sombreamento, grade 2^-48 |
| PNF64 + G2 | 53 | **1.885** | 72 (igual à PNF64) | 6,5·10⁻¹⁴ | L2 + sombreamento, grade 2^-96 |

O G2 não muda a trajetória calculada, então a divergência em relação à
órbita exata de x0 começa no mesmo passo. O que ele acrescenta é a prova de
que a trajetória é a de uma órbita verdadeira partindo de x0' a ≤ ε de x0
(ε da PNF32 + G2 medido no estudo de caso, phase2-20261022).

Operações isoladas PNF64: mul 227, add/sub 218, decisão 14 (PNF32: 134, 145,
11). O custo é dominado por range checks (566 dos 677), cujas larguras crescem
com p; por isso PNF64 custa 1,63× a PNF32, não 2,2× (53/24).

A última coluna de divergência vem de `oracle-case-study/results/h1-20261022`.

## 2. Bit-exatidão

- **Referência** (`pnf-reference`): oráculo exato em binary64 contra o f64
  nativo em 500 mil pares de bits aleatórios, 500 mil na mesma binade
  (cancelamento) e 500 mil no intervalo unitário (incluindo `1 − a` e
  `a·3,9`), cada par nas três operações; casos difíceis (subnormais, ±0,
  ±inf, NaN, MAX, vizinhos de 1) e trajetórias logísticas de 1.000 passos
  em r ∈ {3,5; 3,7; 3,9; 4,0}. Bijeção bits ↔ PNF em 1 milhão de padrões.
- **Circuito** (`pnf-circuit/tests/ops_binary64.rs`): 50 mil pares de bits
  aleatórios e 50 mil no intervalo unitário, cada um nas três operações,
  50 mil pares de near path,
  casos difíceis, todos iguais ao oráculo **e** ao f64 nativo, com CheckCS
  satisfeito e saída válida; TestCS concorda; 4.000 passos logísticos
  encadeados bit-exatos ao f64 nativo; StepCircuit satisfeito.
- **Adulteração:** em 120 pares (metade aleatórios, metade em [0,1]),
  todos os hints possíveis da mul (`k` em 0..127, `ovf`) e da add (16
  combinações de bits × `k` em 0..127) foram tentados: todo witness aceito
  dá a saída honesta.
- **WAD** (`wad-baseline`): 1.000 folds encadeados por r ∈ {3,5; 3,7; 3,9;
  4,0} iguais ao `mulWad` de referência; extremos (0, W/2±1, W, r = 4);
  quocientes deslocados (±1, ±2, 2^40, com resto ajustado) e decisão
  invertida são rejeitados. Nas 1.001 sementes do ataque × 200 passos
  (r = 3,9), as **201.201 decisões do circuito WAD são idênticas** às do
  script Python da fase 1 do estudo de caso.
- Nenhuma regressão: as contagens do binary32 (415 por passo, 1.035 com
  sombreamento) são idênticas às de antes, e todos os testes antigos passam
  (`tests.log`).

## 3. G2 em binary64

Circuito (`shadow-proof`, agora genérico no formato): grade 2^-96,
aninhamento com range checks de 2K + p + 2 = 247 bits (cabe no corpo de
~2^254). 1.885 constraints/fold. Testes: 300 folds encadeados bit-exatos ao
f64 com veredito robusto em todos; alargar `A'`/`B'`, trocar o lado ou ε = 0
é rejeitado; TestCS concorda. Nova (16 folds, uma rodada): prover 0,55 s,
compressão 1,1 s, verificação comprimida 21 ms, prova de 10.880 B.

Scan nas 1.001 sementes do ataque, r = 3,9:

| N | certificadas | ε mediano | ε p90 | ε máx |
|---:|---:|---:|---:|---:|
| 64 | 1.001/1.001 | 1,0·10⁻¹⁵ | 6,9·10⁻¹⁵ | 5,9·10⁻¹³ |
| 1.024 | 1.001/1.001 | 1,6·10⁻¹⁴ | 1,0·10⁻¹³ | 1,2·10⁻¹¹ |
| 4.096 | 1.001/1.001 | 6,5·10⁻¹⁴ | 4,1·10⁻¹³ | 3,8·10⁻¹¹ |

Em preço (`P = 1500 + 1000·x`), a órbita verdadeira parte a ~10⁻¹⁰ US$ do
preço informado.

## 4. Leitura

1. **WAD dá mais bits por constraint.** Com custo praticamente igual ao da
   PNF32 (431 contra 415), o WAD tem ~60 bits e adia a divergência de 31
   para 78 passos. A PNF64 chega a 72 passos e custa 677. Se o critério fosse
   só precisão por custo, o WAD ganharia.
2. **O argumento para IEEE é compatibilidade, não custo nem precisão.**
   Simuladores e modelos em Python/numpy usam binary64 por padrão. A PNF64
   prova exatamente o número que esse código produz, a 1,57× o custo do
   WAD. Um circuito WAD prova outra conta, que discorda do float em ~50% das
   decisões depois do horizonte.
3. **O sombreamento escala para binary64** com 1,8× o custo do G2 em
   binary32 e ε cerca de 10⁸ vezes menor.
4. **IEEE bit-exato não é proibitivo nem em binary64:** PNF64 (677) custa
   menos que o FP32 exato do baseline antigo (1.721).

## 5. Limitações

- WAD + G2 não foi feito: a grade decimal (10⁻¹⁸) não é diádica, e o
  certificado precisaria de outra grade ou de conversão no circuito.
- Só o passo logístico. A variante A (EWMA, raiz) ainda não existe em
  nenhum formato.
- Nova medido em uma rodada só e com 16 folds. Em 1 passo por fold o tempo é
  dominado pelo overhead fixo do Nova; custo em tempo precisa de S passos
  por fold e rodadas pareadas.
- A largura de `k` na mul da PNF64 foi reduzida para `bitlen(K)` (7 bits)
  para evitar wrap no corpo; o argumento é que o `k` honesto fica em [0, K].
  Os testes de adulteração cobrem 0..127, mas a enumeração exaustiva só
  existe para os formatos mini.
- O scan G2 em binary64 foi até N = 4.096 (não 65.536) para caber no tempo.

## 6. Comandos

```bash
bash scripts/run_pnf64_wad.sh results/pnf64-wad-AAAAMMDD
./target/release/pnf-bench constraints --format b64 --output <arquivo novo>
./target/release/wad-bench constraints --output <arquivo novo>
./target/release/shadow-bench constraints --format b64 --out <arquivo novo>
./target/release/shadow-bench nova --format b64 --steps 16 --out <arquivo novo>
```
