# 02 — Especificação da PNF32

PNF (*Proof-Native Float*) é uma **codificação no circuito** do binary32 IEEE
754 com arredondamento *round-to-nearest-ties-to-even* (RNE). A semântica é a do
IEEE. A novidade está na forma como os valores são guardados no estado
IVC e nas **relações verificadas** (o circuito não calcula o resultado, ele
*confere* uma dica do provador com range checks de largura fixa).

Tudo é genérico no formato `(ebits, p)`: `binary32 = (8, 24)`. Os formatos mini
`(3,3)` e `(4,4)` usam exatamente o mesmo código e servem à verificação
exaustiva. Notação: `emin = 1 - bias`, `emax = bias`, `qmin = emin - (p-1)`
(quantum subnormal; em binary32, `emin=-126`, `emax=127`, `qmin=-149`).

Código: [`crates/pnf-circuit/src/ops.rs`](../crates/pnf-circuit/src/ops.rs),
[`pnf.rs`](../crates/pnf-circuit/src/pnf.rs),
[`step.rs`](../crates/pnf-circuit/src/step.rs). Oráculo independente:
[`crates/pnf-reference/src/lib.rs`](../crates/pnf-reference/src/lib.rs).

## 1. Valores

Um valor é a tupla `(s, eb, m, z, inf, nan)` de elementos do corpo:

| campo | significado | domínio válido |
|---|---|---|
| `s` | sinal | bit |
| `eb` | `e - emin`, expoente desenviesado e deslocado | `[0, emax-emin]` |
| `m` | significando com o bit implícito **explícito** | `[0, 2^p)` |
| `z, inf, nan` | flags de classe | bits, no máximo um ligado |

Concretização de um finito (`fnz = 1 - z - inf - nan`, ou zero):
`valor = (-1)^s · m · 2^(e - (p-1))`.

**Invariante de validade `Valid(v)`:**

1. flags booleanas e mutuamente exclusivas;
2. `m < 2^p`, `0 <= eb <= emax - emin`;
3. `m < 2^(p-1)` (bit implícito 0) só se `eb = 0` (subnormais vivem no menor expoente);
4. `fnz = 1  <=>  m ≠ 0`;
5. classes especiais e zero são canônicas: `m = 0`, `eb = 0`; NaN tem `s = 0`.

**Bijeção.** Para padrões não-NaN, `Pnf::from_bits`/`to_bits` é uma bijeção com
os padrões binary32. Todos os NaN colapsam no NaN quieto canônico
`0x7FC00000`. Essa é a especialização *default-NaN* (RISC-V, ARM com DN): o IEEE
754 não especifica payloads, então a fidelidade numérica é idêntica. Ela foi
verificada nos 2^32 padrões (ver [05-validacao.md](05-validacao.md)).

Diferença em relação ao plano original: o plano previa expoente estendido com
subnormais normalizados. A implementação guarda os campos **IEEE nativos**
(subnormal com `m < 2^(p-1)`, `e = emin`), porque o quociente com shift
testemunhado (abaixo) já absorve entradas não normalizadas sem custo extra, e
a saída subnormal deixa de precisar de renormalização (economiza ~30
constraints por operação). Ver o registro [0003](progress/0003-circuito-pnf32.md).

## 2. Técnicas

| | técnica | onde aparece |
|---|---|---|
| T1 | estado desempacotado: o word de 32 bits não é montado a cada fold | estado IVC |
| T2 | elisão de range checks de entrada por invariante indutivo | §6 |
| T3 | alinhamento com shift limitado: `d >= p+3` vira sticky | add §4 |
| T4 | normalização por shift testemunhado; unicidade vem do bit de topo de `Q` | mul §3, add §4 |
| T5 | RNE por guard/sticky/LSB tirados da mesma decomposição | §5 |
| T6 | exceções como álgebra de flags, sem ramos por expoente | mul/add, `finish` |
| T7 | potências de 2 só sobre shifts limitados (≤ 7 bits) | `pow2` |
| T8 | dobra de constantes (`1`, `r`) e range check fundido ao produto | `gadgets.rs` |

## 3. Multiplicação

Entradas válidas `a, b`. `P = m_a · m_b < 2^(2p)`. Corte fixo `K = 2p+1`.

Hints do provador: `k ∈ [0, 2^kb)` (`kb = bitlen(3p)`, 7 em binary32) e o bit
`ovf`. Define-se `dexp = K - k`, `F = 2^k` (T7) e verifica-se

```
P · F = Q · 2^K + R,    Q ∈ [0, 2^p),  R ∈ [0, 2^K)        (decomposição em bits)
```

ou seja `Q = floor(P / 2^dexp)` e `R` carrega os bits descartados alinhados
no topo. `t = Q[p-1]` (bit de topo) decide o caminho:

- `w = (emin + p - 1) - e_a - e_b + (1 - fnz_a·fnz_b) · C`, com
  `C = K - (emin + p - 1 - 2·emax)`. O termo `C` empurra `w` para `>= K`
  quando algum operando é zero ou especial.
- **Normal (`t = 1`):** `dexp - w >= 0` (o quantum do resultado é ≥ `2^qmin`).
  Como `Q ∈ [2^(p-1), 2^p)`, `dexp` fica determinado por `P`.
- **Subnormal (`t = 0`):** `dexp - w <= 0` e `(dexp - w)·(K - dexp) = 0`. Ou seja,
  `dexp = w` (quantum exatamente `2^qmin`) ou `dexp = K < w` (resultado
  necessariamente zero, porque `P < 2^(2p) <= 2^(K-1)`).

As duas condições viram **um** range check: `(2t-1)·(dexp - w) ∈ [0, 2^yb)`,
mais `m1 = (1-t)·(dexp-w)` e `m1·k = 0`. Expoente de saída:
`e_out = e_a + e_b - p + 1 + dexp + carry`.

Overflow: `(2·ovf-1)·(e_out - emax - 1) + ovf - 1 ∈ [0, 2^ob)`, ou seja
`ovf = [e_out > emax]`, mascarado por `ovf·(1 - zf)`.

Classes: `nan = (nan_a ∨ nan_b) + inf_a·z_b + z_a·inf_b` (termos disjuntos),
`inf = ¬nan ∧ (inf_a ∨ inf_b ∨ ovf)`, sinal `s_a ⊕ s_b`.

## 4. Adição

Hints: `swap, big, neg, k ∈ [0, 2^kb2)` (`kb2 = bitlen(2p+2)`, 6 em binary32) e
`ovf`.

1. **Ordenação:** `(A, B) = swap ? (b, a) : (a, b)` por seleção de 4 campos;
   `d = e_A - e_B ∈ [0, 2^db)`.
2. **Alinhamento limitado (T3):** `big ∈ {0,1}` com
   `(2·big-1)·(d - (p+3)) + big - 1 ∈ [0, 2^bigb)`. Para `big = 0`, os bits
   baixos desse mesmo valor são `p+2-d` e dão `2^(p+2-d)` (T7). Então
   `Y = (1-big)·m_B·2^(p+2-d) + big·fnz_B`. Se `d >= p+3`, a contribuição de `B` é
   menor que um quarto do quantum do resultado, e só o fato de ser não nula
   (sticky) importa. Ver o lema A1.
3. **Soma exata:** `X = (1-2s_A)·m_A·2^(p+2) + (1-2s_B)·Y`; `|X| = (1-2·neg)·X`.
4. **Normalização (T4):** `|X|·2^k = Q·2^K2 + R`, `K2 = p+3`, `Q < 2^p`, `R < 2^K2`.
   `t = Q[p-1]`.
5. **Expoente:** `u = dexp + e_A - (emin + p + 2) ∈ [0, 2^ub)` (quantum ≥ `2^qmin`).
   Se `t = 0` e o resultado não é zero, `u = 0`: `(1-t)·u·(1-zf) = 0`.
6. **Zero só se exato:** `zf · |X| = 0`. A adição IEEE com subnormais graduais
   nunca arredonda para zero (`x + y = 0 ⟺ x = -y`), então resultado zero exige
   soma exata nula. **Esta constraint faltava na primeira versão**: a
   enumeração exaustiva achou o contraexemplo (registro 0003).
7. `e_out = e_A - p - 2 + dexp + carry`; overflow como na multiplicação.
8. Classes e sinal: `nan = (nan_a ∨ nan_b) + inf_a·inf_b·(s_a ⊕ s_b)`. O sinal de
   um zero exato é `s_a ∧ s_b` (RNE), senão `neg`; com infinito, herda o sinal
   do infinito.

`sub(a, b) = add(a, -b)`.

## 5. Arredondamento comum

Com os bits de `Q` e `R` já alocados pela decomposição:

```
guard = R[top]     sticky = (R sem o topo ≠ 0)     lsb = Q[0]
up    = guard ∧ (sticky ∨ lsb)                         -- RNE
Qr    = Q + up
carry = up ∧ (Q = 2^p - 1)       m = Qr - carry·2^(p-1)
zf    = (Qr = 0)
```

Saída: `m_out = fnz_out · m`, `eb_out = fnz_out · (e_out - emin)`; especiais e
zero ficam canônicos.

## 6. Decisão `x >= 2^k`

Para `emin < k <= emax`, com o bit `b` e
`(2b-1)·(e - k) + b - 1 ∈ [0, 2^n)`, `dec = (1 - s - nan)·(b + inf)`. Para normais
positivos, `x >= 2^k ⟺ e >= k`. Subnormais e zero têm `e = emin < k`. O caso
`+inf` é tratado pela flag. Custo: 11 constraints.

## 7. Lemas

**A1 (sticky na adição).** Se `d >= p+3`, então `A` é normal, `|X|` tem bit de
topo em 2p ou 2p+1 (quantum ≥ `2^(p+1)`), e a contribuição exata de `B`,
`|m_B·2^(p+2-d)| < 2^(p-1)`, é menor que `quantum/4`. Como `m_A·2^(p+2)` é
múltiplo do quantum, `X` e `X' = m_A·2^(p+2) ± 1` estão no mesmo semi-intervalo
aberto de arredondamento e não podem ser empate: o RNE coincide. ∎

**U (unicidade / determinismo).** Fixadas as entradas e os hints, toda
variável restante é determinada: bits de range check (decomposição binária
única abaixo de `p`), `Q` e `R` (divisão euclidiana única, já que
`Q·2^K + R < 2^(p+K) ≪ |F|`), acumuladores de `pow2`, produtos e as flags de
`is_zero` (o inverso é livre só quando o argumento é 0, e não é usado em mais
nada). Logo, o conjunto de witnesses satisfatórios é enumerado por completo
quando se enumeram os hints. O teste
[`exhaustive_uniqueness.rs`](../crates/pnf-circuit/tests/exhaustive_uniqueness.rs)
faz isso para todo par de entradas nos formatos `(3,3)` e `(4,4)` e checa que
**exatamente uma** saída é aceita, que ela é válida e que é igual ao oráculo.

**I (invariante indutivo, T2).** Se as entradas satisfazem `Valid`, toda saída
aceita satisfaz `Valid`: `m` vem de `Q` (já limitado a `p` bits) mais o carry,
o expoente é limitado inferiormente pela regra de quantum e superiormente pela
de overflow, e as flags são produtos de bits disjuntos. (Isto também é checado
nos testes: toda saída passa por `validate`.) Como `z0` é checado pelo
verificador fora do circuito, `Valid(z_i)` vale em todo fold por indução, e as
entradas não precisam ser re-checadas. **Ressalva:** a garantia exige que o
verificador realmente cheque `z0` (ou que o primeiro fold use `validate`).

**Ressalva de escopo da prova.** O lema U é um argumento manual,
complementado pela enumeração exaustiva *nos formatos mini*. Para binary32, a
unicidade segue do mesmo código paramétrico e de testes adversariais
amostrados (todos os 128×2 hints de mul e 64×16 de add para 300 pares
aleatórios). Uma prova mecanizada (por exemplo em Lean/Coq, ou um SMT sobre as
constraints) é trabalho futuro e está registrada em
[06-resultados.md](06-resultados.md).
