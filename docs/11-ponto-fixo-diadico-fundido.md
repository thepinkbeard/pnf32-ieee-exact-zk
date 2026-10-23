# 11 — Ponto fixo diádico fundido

**Data:** 2026-10-23 · Código: [`crates/dyadic-fused`](../crates/dyadic-fused) ·
Campanha: `results/dyadic-fused-20261023/` · Registro: [0013](progress/0013-ponto-fixo-diadico-fundido.md)

Pergunta: dá para ter **mais precisão em relação à matemática exata** e
**menos constraints** que a PNF32 e o WAD? A hipótese, de
[`../../tese/representacao-caminho-2026-10-22.md`](../../tese/representacao-caminho-2026-10-22.md),
é que a precisão depende só do número de bits (`N* ≈ p·ln2/λ`) e que a
representação só define quanto custa cada bit.

## 1. O perfil

Estado `X = x·2^p` com `0 ≤ X ≤ 2^p`. O parâmetro é `R = r·2^s` constante,
com `s = 22` (`fl32(r)` em [2, 4) é exato). Há **um** arredondamento (piso)
por passo:

```
t = X·(2^p − X)·R;   Y = floor(t / 2^(p+s));   decisão = [Y ≥ 2^(p−1)]
```

O circuito é **uma** constraint de produto,
`X · (R·(2^p − X)) = Y·2^(p+s) + rem`. O produto por R é linear porque R é
constante, e o lado direito é a recomposição dos bits de `Y` (p+1) e de `rem`
(p+s). A decisão é `bit_{p−1}(Y) + bit_p(Y)`, linear e sem constraint. `2^p − X`
não tem range check porque o invariante `0 ≤ X ≤ 2^p` é herdado do fold
anterior. O custo é `(p+1) + (p+s)` booleanidades, mais 1 produto e 3 saídas,
ou seja, **2p + s + 5**.

**Soundness** (argumento completo no topo de `src/lib.rs`): com
`2p+s+2 ≤ 253`, os dois lados ficam abaixo do módulo de Pallas, a igualdade vale
nos inteiros, e a divisão euclidiana com `0 ≤ rem < 2^(p+s)` é única. Então o
witness é o piso honesto, `Y ≤ 2^p` (o invariante passa adiante) e a decisão é
booleana. **O provador não tem graus de liberdade**, assim como na PNF e no WAD.
O limite do corpo é `p ≤ 114` com s = 22. O maior perfil medido é p = 110.

## 2. Constraints por passo (mapa logístico + decisão, Pallas)

| representação | bits em [0,1] | constraints/passo | constraints por bit | 1º passo divergente, r=3,9 (mediana) | semântica |
|---|---:|---:|---:|---:|---|
| PNF32 | 24 | 415 | 17,3 | 31 | IEEE binary32 bit-exato |
| WAD (`mulWad` piso) | 59,8 | 431 | 7,2 | 78 | ponto fixo decimal do Solidity |
| PNF64 | 53 | 677 | 12,8 | 72 | IEEE binary64 bit-exato |
| **diádico fundido p=32** | 32 | **91** | 2,8 | 43 | perfil declarado (2^-32, piso) |
| **diádico fundido p=64** | 64 | **155** | 2,4 | 87 | perfil declarado (2^-64, piso) |
| **diádico fundido p=96** | 96 | **219** | 2,3 | 133 | perfil declarado (2^-96, piso) |
| **diádico fundido p=110** | 110 | **247** | 2,2 | 152 | perfil declarado (2^-110, piso) |

O breakdown (`dyadic_constraints.json`) é `range` 2p+s+1, `arith` 1, `io` 3,
exatamente a estimativa feita antes da implementação.

Comparações diretas:

- **p=64 contra WAD:** 0,36× as constraints (155 contra 431), com 4 bits a mais
  e divergência 9 passos mais tarde (87 contra 78).
- **p=64 contra PNF32:** 0,37× as constraints, com 2,8× o horizonte (87 contra
  31).
- **p=110 contra PNF32:** 0,60× as constraints, com 4,9× o horizonte (152 contra
  31).
- **p=96 contra Q64.96 do h1** (mesma escala, mas dois arredondamentos por
  passo): divergência em 133 contra 129. A fusão rende ~4 passos, ou seja, cerca
  de 2 bits.

Nova (16 folds, 1 passo/fold, uma rodada, smoke): 9.975 constraints primárias
em p=64 e 10.067 em p=110 (o overhead fixo do Nova domina), prover ~0,5 s,
compressão ~0,95 s, verificação comprimida ~20 ms, prova de 10.656 B. Os dois
estados finais conferem com a referência.

## 3. Decisões contra o h1

`scripts/compare_dyadic_h1.py`:

1. **Correção:** uma trajetória diádica fundida recalculada em inteiros Python,
   de forma independente do Rust, confere com o circuito em **3.219.216
   decisões** (4 perfis × 4 r × 1.001 sementes × 201 passos): **0 diferenças**.
   Cada fold do circuito também confere estado e decisão com a referência Rust
   (`decisions` aborta em qualquer divergência).
2. **Divergência em relação à órbita exata certificada** (coluna `reference` do
   h1). Tabela completa em `results/dyadic-fused-20261023/summary.md`. Primeiro
   passo divergente mediano, comparado com a previsão `p·ln2/λ`:

| r (λ) | p=32 | p=64 | p=96 | p=110 |
|---|---|---|---|---|
| 3,5 (−0,873) | nenhuma divergência | nenhuma | nenhuma | nenhuma |
| 3,7 (0,355) | prev. 62 · obs. 58 | 125 · 120 | 187 · 181* | 215 · 195* |
| 3,9 (0,496) | 45 · 43 | 89 · 87 | 134 · 133 | 154 · 152 |
| 4,0 (0,693) | 32 · 32 | 64 · 65 | 96 · 98 | 110 · 112 |

\* Horizonte de 200 passos: em r=3,7 só 905 (p=96) e 213 (p=110) das 1.001
sementes divergem até N=200, então a mediana é das que divergiram e fica
**subestimada**.

A lei `N* ≈ p·ln2/λ` vale também para o perfil novo, com o mesmo viés de poucos
passos dos outros formatos. Em r=3,5 (λ<0) nenhum formato diverge.

## 4. Leitura

1. **Sim: mais precisão e menos constraints ao mesmo tempo.** O diádico fundido
   entrega de 4,6 a 7,7× mais bits por constraint que a PNF32 e cerca de 3× mais
   que o WAD. A hipótese de que o custo é dominado por range checks proporcionais
   aos bits, e não pelo formato, se confirma: o perfil gasta quase só os
   `2p+s` bits da divisão.
2. **A precisão segue a lei do horizonte.** Nenhum perfil "escapa" do caos:
   p=110 leva o horizonte a ~150 passos em r=3,9, e a partir daí a decisão
   volta a ser 50/50 em relação à matemática exata. Para N maior é preciso mais
   bits (custo por passo ∝ N, total Θ(N²)) ou sombreamento (G2).
3. **O que se perde:** a compatibilidade bit-exata com código float (numpy). O
   L2 passa a ser "exato ao perfil declarado". Para um oráculo DeFi isso não é
   perda, porque a semântica nativa on-chain já é ponto fixo.

## 5. Limitações

- Só o passo logístico, que é polinomial de grau 2 com um único termo variável
  × variável. Mapas com mais produtos precisam de um corte por produto ou de
  acumulação no corpo (limite de 253 bits).
- `p ≤ 114` com s = 22, num único elemento de corpo. Acima disso, é preciso
  limbs.
- Ponto fixo **não** serve para estados com faixa dinâmica larga, em que o erro
  relativo perto de 0 importa. Aqui a decisão é por limiar absoluto em [0,1].
- Sem G2 nem envelope (L3) neste perfil ainda. A grade é binária, então o
  certificado HYG deve se encaixar. Fica para o próximo passo.
- Nova medido em uma rodada, com 1 passo por fold (overhead fixo domina).

## 6. Comandos

```bash
bash scripts/run_dyadic_fused.sh results/dyadic-fused-AAAAMMDD
./target/release/dyadic-bench constraints --ps 32,64,96,110 --output <arquivo novo>
./target/release/dyadic-bench decisions --p 64 --r 3.9 --seeds 1000 --nmax 200 --output <arquivo novo>
./target/release/dyadic-bench nova --p 64 --steps 16 --output <arquivo novo>
```
