# 03 — Modelo de custo (analítico × medido)

Convenção de R1CS no frontend do Nova: um range check de `n` bits custa `n`
booleanidades + 1 recomposição (a recomposição é fundida a um produto quando
o valor checado é um produto). Produto por constante e checagem de constante
custam zero.

**Proveniência honesta.** As estimativas abaixo ("modelo") foram calculadas
durante o desenho, antes da implementação (mul ≈ 133–148, add ≈ 144–148,
passo ≈ 415–460). Este documento foi escrito *depois* da medição e confere
termo a termo com o código. Os limiares H1/H2 é que estavam congelados no
plano aprovado antes de qualquer código.

## Multiplicação (binary32, var × var): 134

| item | constraints |
|---|---:|
| `fab`, `s_a·s_b`, `P = m_a·m_b`, `P·F` | 4 |
| bits de `k` (7) | 7 |
| `F = 2^k` (7 fatores, o 1º linear) | 6 |
| decomposição `R` (49) + `Q` (24) + recomposição | 74 |
| regra de quantum `(2t-1)(dexp-w)` em 9 bits (fundido) | 10 |
| `m1`, `m1·k = 0` | 2 |
| guard/sticky/RNE/carry (`sl`, `up`, `carry`) | 3 |
| 3 testes de zero (sticky, `Q = 2^p-1`, `Qr = 0`) | 6 |
| overflow: bit + range 9 (fundido) + máscara | 12 |
| classes e seleção final | 10 |
| **total** | **134** |

O termo dominante é a decomposição de `R` em `2p+1 = 49` bits: o produto
exato tem 48 bits e o sticky precisa de todos os descartados. Com `r`
constante (`x·r`), `P` é linear e o custo cai para **127**.

## Adição (var × var): 145

| item | constraints |
|---|---:|
| `swap` + 4 seleções | 5 |
| `d ∈ [0,2^8)` | 9 |
| `big` + range 8 (fundido) | 10 |
| `2^(p+2-d)` (5 bits, 4 produtos) | 4 |
| `Y` (3 produtos) | 3 |
| `X`, `neg`, `|X|`, `|X|·F` | 5 |
| bits de `k` (6) + `F` (5 produtos) | 11 |
| decomposição `R` (27) + `Q` (24) + recomposição | 52 |
| `u ∈ [0,2^8)`, `v`, `v·(1-zf)`, `zf·|X|` | 12 |
| RNE + testes de zero | 9 |
| overflow | 12 |
| classes, sinal de zero/inf, seleção | 13 |
| **total** | **145** |

`1 - x` (constante 1 dobrada): **141**.

## Passo logístico + decisão: 415

`141 (1-x) + 134 (x·c) + 127 (p·r) + 11 (decisão) + 2 (io: contador e flag NaN)`.

| categoria | constraints | % |
|---|---:|---:|
| range (booleanidades + recomposições) | 306 | 73,7 |
| zero-test | 18 | 4,3 |
| class | 25 | 6,0 |
| shift (`pow2`) | 21 | 5,1 |
| exponent | 13 | 3,1 |
| arith, align, rounding, decision, io | 32 | 7,7 |

## Por que o baseline FP32 custava 1.721

| fonte de custo no baseline | PNF |
|---|---|
| `1-x` alinhado à escala absoluta `2^-149`: diferença de 150 bits range-checada (924 no total) | alinhamento relativo com shift limitado: resto de 27 bits (141 no total) |
| toda alocação refaz range de expoente (8+8+8 bits) e significando (24) | entradas herdadas não são re-checadas (T2); saídas válidas por construção |
| `pow2` sobre os 8 bits do expoente | `pow2` só sobre shifts limitados (≤ 7 bits) |
| produto de 48 bits range-checado para extrair o bit alto | o bit de topo de `Q` decide o caminho; não há range check de `P` |

## Onde ainda há gordura (próximos 2–3×)

1. **Range checks são 74%**. Em Plonkish com lookups (tabela de 8–16 bits), os
   ~300 bits do passo viram ~20–40 lookups. É a alavanca do ZKLP e é ortogonal
   à PNF (a codificação e as relações continuam valendo).
2. **`R` de 49 bits na mul**: os `k` bits baixos de `R` são zero por
   construção. Uma variante com corte variável testemunhado pode baixar para
   ~25 bits, mas exige um range check de largura variável (estudar).
3. **Vários passos por fold**: com o overhead fixo do Nova (~11,4 mil
   constraints), a vantagem de prover só aparece com muitas operações por
   fold. Ver [06-resultados.md](06-resultados.md).
