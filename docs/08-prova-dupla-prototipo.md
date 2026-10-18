# 08 — Prova dupla L2+L3: protótipo

Crate `crates/dual-proof` · resultados em `results/dual-20261016/` · log
[0009](progress/0009-prova-dupla-prototipo.md).

## Resumo

Num único passo Nova (nova-snark 0.73, Pallas/Vesta, IPA), o circuito
`DualCircuit<K>` prova três coisas sobre o mesmo par público `(r, x0)`:

1. **L2**: o passo logístico `y = (x·(1−x))·r` em binary32, bit-exato
   (PNF32, `pnf_circuit::step::LogisticCircuit`, reaproveitado sem mudança);
2. **L3**: um enclosure certificado `[c−R, c+R]` da trajetória em
   matemática exata (CBD com bound de Taylor local, `CbdLogisticStepCircuit<K>`
   de `../proof-numeric-zk`, reaproveitado como caixa-preta);
3. **decisão combinada**: `RobustTrue`, `RobustFalse` ou `Indeterminate`.

Custo por fold, com `r = fl(3,9)` e K = 2: **415 + 847 + 6 = 1.268
constraints**. Com `r* = 39/10`, o mesmo `r` do baseline CBD, são 415 + 709 +
6 = 1.130. Isso é 66% do FP32 exato sozinho (1.721), que só entrega L2. A
composição custa exatamente a soma das partes mais 6 constraints.

Em 1.000 sementes com r = 3,9, a decisão sai robusta em 100% dos casos no
horizonte 10, em ~80% no horizonte 30 e em ≈0% no horizonte 100. **Toda**
decisão binary32 errada em relação à matemática exata (197 de 1.000 no
horizonte 30; 488 no 100) foi marcada como indeterminada, e nenhum veredito
robusto contradisse a referência exata.

## 1. Enunciado provado

**Parâmetros públicos** (constantes da forma R1CS, ligados ao digest dos
parâmetros públicos Nova):

- `r_f`, o binary32 usado pela parte IEEE;
- `r* = r_num/r_den`, o racional do modelo exato. O padrão é
  `r* = val(r_f)`, o mesmo número; a variante "modelo" usa, por exemplo, 39/10;
- `K ∈ {2, 4}`: a grade CBD é `2^(−16K)`, com escala `S = 2^(16K)`;
- o limiar `t = 1/2`.

**Entrada pública:** `z0`, que o verificador checa fora do circuito com
`check_z0`. Ele contém o contador 0, `x̂0` (binary32 válido em [0,1]) e o
estado CBD inicial canônico `initial_cbd_state(val(x̂0))`, com raio 0 se
`val(x̂0)` está na grade e 1 ULP caso contrário.

**Estado** (aridade 12 + K):
`[i, s, eb, m, z, inf, nan, d_ieee, limbs[K], is_one, R, d_cbd, v]`.

**Enunciado.** Seja `π` uma prova Nova (recursiva ou comprimida) que verifica
para `(n, z0, z_n)`. Então, sob a knowledge soundness de Nova:

- **(L2)** `x̂_n = fl∘…∘fl(x̂0)` (n vezes), onde `fl(x) = RNE(RNE(RNE(1−x)·x)·r_f)`
  em binary32, com igualdade de bits. Além disso, `d_ieee = [x̂_n ≥ 1/2]`.
- **(L3)** Com `x*_0 = val(x̂0)` e `x*_{k+1} = r*·x*_k·(1−x*_k)` em ℚ, vale
  `|x*_n − c_n/S| ≤ R_n/S`. Também `d_cbd = 1 ⇒ (c_n−R_n)/S ≥ 1/2` e
  `d_cbd = 0 ⇒ (c_n+R_n)/S < 1/2`.
- **(Decisão)** `v_n = 1 ⇒ d_ieee = 1 ∧ x*_n ≥ 1/2`;
  `v_n = 0 ⇒ d_ieee = 0 ∧ x*_n < 1/2`. Logo, **um veredito robusto implica que
  a decisão IEEE é igual à decisão exata.** Corolário útil: `v_n = 2` com
  `d_cbd ∈ {0, 1}` é um **certificado de que a decisão binary32 está errada**
  em relação ao modelo exato.

O estado só expõe os vereditos do último passo. Os intermediários são
restritos em cada fold, mas sobrescritos, como na PNF32.

## 2. Argumento de soundness

**Invariante `I(z_i)`:**

- (a) a parte PNF32 de `z_i` é a codificação válida e canônica de `x̂_i`;
- (b) os limbs CBD estão em `[0, 2^16)`, `is_one` é booleano e canônico,
  `R_i ≤ S` e `|x*_i − c_i/S| ≤ R_i/S`;
- (c) o contador vale `i`.

**Base.** `check_z0` exige `z0` igual ao estado honesto. Daí saem (a), (b) e
(c), e a contenção inicial é conferida explicitamente.

**Passo** (um fold satisfeito com `I(z_i)` ⇒ `I(z_{i+1})`):

- **(a)** Com a entrada válida, os lemas de unicidade da PNF32
  ([02](02-especificacao-pnf32.md)) fixam a saída como o único binary32 RNE
  correto e válido por construção. Por isso a entrada não é revalidada (T2).
  Os lemas são checados exaustivamente em formatos pequenos e na varredura
  2^32 (ver [05](05-validacao.md)).
- **(b)** O circuito CBD revalida a entrada (range checks de limbs e raio).
  Ele restringe `c' = round(r*·N(S−N)/(r_den·S))` com resto e gap, e
  `R' ≥ ⌈r*(R·|S−2N| + R²)/S⌉ + [resto≠0]`, saturando em S. Pela identidade de
  Taylor exata `f(c+δ) − f(c) = r*·δ·(1−2c) − r*·δ²`, temos
  `|f(x*_i) − f(c_i)| ≤ r*(R|1−2c| + R²)` (na escala). O arredondamento do
  centro acrescenta no máximo 1 ULP, então a contenção se mantém. Com
  saturação, `[c−S, c+S] ⊇ [0,1] ∋ x*_{i+1}` porque `r* ≤ 4` (o construtor
  exige isso). A soundness do gadget em si é a de `proof-numeric-zk`
  (`docs/mathematical-specification.md` lá). Aqui ela não é re-provada, só
  testada contra a trajetória exata e a referência dirigida.
- **(c)** Os dois sub-circuitos recebem a mesma variável de contador. O
  contador de saída usado é o da PNF32 (`i+1`); a cópia da CBD (também `i+1`)
  é descartada.
- **Combinação.** Seis constraints:
  - `a, b` booleanos;
  - `d_cbd = a + 2b`;
  - `RT = a·d_ieee`;
  - `RF = (1−a−b)(1−d_ieee)`;
  - `v = 2 − RT − 2RF`.

  A CBD já força `d_cbd ∈ {0,1,2}` (one-hot + range de 2 bits), e
  `d_ieee ∈ {0,1}` vem de (a) e de `ge_pow2`. Então `(a, b)` é único, porque
  `a+2b = 3` está excluído, e `RT`, `RF` e `v` ficam determinados. O teste
  `combination_is_unique_and_correct` confirma isso com witnesses adulterados.

**Composição entre folds.** Nova garante que, se `π` verifica, existe uma
cadeia `z_0 → z_1 → … → z_n` em que cada transição satisfaz `F`. Por indução
sobre `I` sai o enunciado. **O que compõe:** as partes L2 e L3 só se acoplam
por três canais:

1. o contador compartilhado, que garante que as duas afirmações falam do mesmo
   índice `i`;
2. as constantes públicas `r_f` e `r*`, presas à forma do circuito e, portanto,
   ao digest dos parâmetros públicos;
3. o `z0` comum, checado pelo verificador.

Nenhum estado cruzado além disso é necessário, porque L2 e L3 são
invariantes independentes e a combinação é local a cada fold. **O que não
compõe sem trabalho extra:** "todos os passos foram robustos" exige um
acumulador (`v_acc ← v_acc ∧ robusto(v_i)`, 2–3 constraints). O protótipo
expõe só o último passo.

**Tamanhos no campo.** O maior produto intermediário é
`N(S−N)·r_num ≤ 2^(32K+23)`, ou seja, 2^151 com K = 4. Isso fica bem abaixo
de `p ≈ 2^254`, sem wrap-around.

## 3. Custos

### 3.1 Constraints por fold (determinístico; `constraints.json`)

| CBD r | K | PNF32 | CBD | combinação | **total** | CBD sozinha | aridade |
|---|---:|---:|---:|---:|---:|---:|---:|
| fl(3,9) = 8178893/2^21 | 2 | 415 | 847 | 6 | **1.268** | 847 | 14 |
| fl(3,9) | 4 | 415 | 1.431 | 6 | **1.852** | 1.431 | 16 |
| 39/10 | 2 | 415 | 709 | 6 | **1.130** | 709 | 14 |
| 39/10 | 4 | 415 | 1.293 | 6 | **1.714** | 1.293 | 16 |

- A CBD embutida conta exatamente o mesmo que a CBD sozinha (o programa
  verifica isso por `assert`). 709 reproduz o CBD-32 de `proof-numeric-zk`.
- **O custo da CBD depende de `r`**, não de `x0`. O `r = fl(3,9)` exato tem
  numerador de 23 bits e denominador 2^21, o que alarga os range checks do
  resto (+138 constraints em relação a 39/10).
- Comparação: o FP32 exato sozinho (só L2, sem L3) custa 1.721. A prova dupla
  com K = 2 custa 1.130 a 1.268.

### 3.2 Nova, 10 folds, com compressão (3 repetições; medianas)

| | PNF32 sozinha | dual K=2, fl(r) | dual K=2, 39/10 | dual K=4, fl(r) |
|---|---:|---:|---:|---:|
| constraints primárias | 11.794 | 12.653 | 12.515 | 14.793 |
| constraints secundárias | 10.349 | 10.349 | 10.349 | 10.349 |
| prover IVC, 10 folds (s) | 1,94 | 1,97 | 1,74 | 1,56 |
| verificação recursiva (ms) | 50 | 77 | 84 | 130 |
| compressão (s) | 3,20 | 3,04 | 3,04 | 3,22 |
| verificação comprimida (ms) | 127 | 135 | 113 | 94 |
| prova comprimida (B, bincode) | 10.816 | 11.008 | 11.008 | 11.072 |
| pico de RSS (MiB) | 189 | 165 | 213 | 225 |

A prova dupla acrescenta 192–256 B à prova comprimida e de 721 a 2.999
constraints primárias, sobre um piso de ~11,4 mil da verificação recursiva.
**Os tempos não são conclusivos.** A máquina estava com load average ≈ 53
durante a medição (outras sessões compilando e medindo), e a PNF32 sozinha
levou 0,19 s/passo contra 0,046 s/passo no 0005. As três variantes duais
empatam com a PNF32 dentro desse ruído, o que é coerente com o 0005: nessa
faixa, o fold Nova quase não depende do número de constraints. Uma medição
pareada em máquina ociosa fica como próximo passo.

## 4. Cobertura da decisão robusta (`coverage.json`, `coverage.csv`)

Configuração:

- 1.000 sementes `x0 = u/2^24`, com `u = splitmix64(20261016) >> 40` e `u ≠ 0`.
  São binary32 exatos e exatos na grade CBD, logo o raio inicial é 0.
- r = 3,9 e horizontes 10, 30 e 100.
- A decisão exata vem do intervalo dirigido de 4.096 bits
  (`numeric_reference::directed_dynamic_trajectory`), decisivo em 100% dos
  casos.
- Nas 20 primeiras sementes de cada (modo, K), o veredito também foi
  reproduzido no circuito (`CheckCS`), passo a passo até n = 100.

**Modo "mesma entrada"** (`r* = val(fl(3,9))`):

| K | n | RT | RF | **robusto** | indet. (enclosure cruza 1/2) | indet. (IEEE contradiz o enclosure) | IEEE errada vs. exato | errada e sinalizada | robusto errado | raio mediano |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2 | 10 | 569 | 431 | **100,0%** | 0 | 0 | 0 | 0 | 0 | 1,1e−7 |
| 2 | 30 | 471 | 324 | **79,5%** | 12 | 193 | 197 | 197 | 0 | 3,0e−3 |
| 2 | 100 | 0 | 0 | **0,0%** | 1.000 | 0 | 488 | 488 | 0 | 1 (saturado) |
| 4 | 10 | 569 | 431 | **100,0%** | 0 | 0 | 0 | 0 | 0 | 2,3e−17 |
| 4 | 30 | 476 | 327 | **80,3%** | 0 | 197 | 197 | 197 | 0 | 6,4e−13 |
| 4 | 100 | 4 | 4 | **0,8%** | 985 | 7 | 488 | 488 | 0 | 1 (saturado) |

**Modo "modelo"** (`r* = 39/10`; a parte IEEE continua com fl(3,9)):

| K | n | robusto | indet. cruza | indet. conflito | IEEE errada | sinalizada | robusto errado |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 2 | 10 | 100,0% | 0 | 0 | 0 | 0 | 0 |
| 2 | 30 | 78,3% | 18 | 199 | 206 | 206 | 0 |
| 2 | 100 | 0,0% | 1.000 | 0 | 529 | 529 | 0 |
| 4 | 10 | 100,0% | 0 | 0 | 0 | 0 | 0 |
| 4 | 30 | 79,4% | 0 | 206 | 206 | 206 | 0 |
| 4 | 100 | 1,2% | 980 | 8 | 529 | 529 | 0 |

**Leitura.**

- **Soundness empírica:** 0 vereditos robustos errados em 12 mil pares
  (semente, horizonte, K, modo). Toda decisão binary32 errada foi marcada.
- **No horizonte 30, a falha é do binary32, não do enclosure.** Com K = 4,
  todos os 197 indeterminados são conflitos: o enclosure é decisivo (raio
  ~1e−12) e a decisão IEEE está do lado errado. A prova dupla, portanto,
  certifica que a resposta IEEE está errada, coisa que nem L2 sozinho (ZKLP,
  PNF32) nem L3 sozinho conseguem afirmar. Com K = 2, 12 casos ainda cruzam o
  limiar por falta de precisão.
- **No horizonte 100, o enclosure satura.** O raio cresce cerca de 1,67× por
  passo (expoente de Lyapunov estimado ≈ 0,51, tirado da razão entre os raios
  medianos em n = 10 e n = 30), ou seja, ~0,74 bit por passo. A saturação
  (raio = 1) estimada fica em n ≈ 41 (K = 2) e n ≈ 85 (K = 4). Para n = 100
  seriam precisos ≳ 80 bits fracionários, acima dos 64 do `K ≤ 4` do circuito
  CBD atual. A escalada CDAD (outra instância Nova por degrau) é o caminho
  natural.
- **"Mesma entrada" × "modelo":** com 39/10, a IEEE erra um pouco mais (206
  contra 197 no horizonte 30), porque ao erro de arredondamento soma-se o erro
  de representação de `r`. Os dois modos custam diferente (§3.1).

## 5. Limitações

- `r_f` e `r*` são constantes do circuito: trocar `r` exige novos parâmetros
  públicos. O limiar é fixo em 1/2, que é potência de 2 e por isso cabe no
  `ge_pow2` da PNF32.
- Só o veredito do último passo é exposto; não há acumulador de "todos os
  passos robustos".
- Um passo por fold (`steps_per_fold` não foi levado à prova dupla). A cópia
  CBD do contador gasta 1 constraint redundante, preço de reaproveitar o
  circuito como caixa-preta.
- `K ≤ 4` (limite do crate CBD), o que dá horizonte útil ≲ 80 passos com
  r = 3,9.
- A soundness da parte CBD é herdada de `proof-numeric-zk`. Aqui só há
  evidência empírica: contenção da trajetória racional exata até n = 12, do
  intervalo dirigido de 4.096 bits até n = 100 em 6 casos de borda e 1.000
  sementes, e unicidade da combinação.
- A checagem de `z0` é feita fora do circuito, como na T2 da PNF32.
- Os tempos foram medidos sob carga pesada, com 3 repetições e sem
  pareamento. Só constraints e tamanhos de prova são números firmes.
- Uma única família de sementes e um único `r`.
