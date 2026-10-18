# 09 — Protótipo G2: IEEE exato + certificado de sombreamento (HYG) em Nova

**Data:** 2026-10-18 · Código: [`crates/shadow-proof`](../crates/shadow-proof) ·
Campanha: `results/shadow-20261018/` · Registro: [0011](progress/0011-prova-de-sombreamento.md)

## 1. O que é provado

Para `r` e `x0` públicos (binary32), horizonte `N` e um ε público, a prova Nova
de `N` folds afirma:

1. **(L2, exato)** `p_0 = x0` e `p_{n+1} = fl(fl(p_n·fl(1−p_n))·r)` em binary32
   RNE, bit a bit (PNF32);
2. **(L3 retroativo, sombreamento)** existem intervalos de grade `I_n` (2^-48)
   com `f(I_n) ⊇ I_{n+1}` e `|p_n − x| ≤ ε` para todo `x ∈ I_n`. Logo **existe uma
   órbita verdadeira** `x_n = f^n(x0')` do mapa real `f(x) = r·x(1−x)` (com o `r`
   exato do float), com `|x_n − p_n| ≤ ε` para `n = 0..N`. Em particular,
   `|x0' − x0| ≤ ε`;
3. **(decisão)** em cada passo, o veredito é *robusto* quando o lado de 1/2
   onde o certificado prende a órbita verdadeira coincide com a decisão IEEE
   `p_n ≥ 1/2`; caso contrário, é *indeterminado*.

Construção do certificado (fora do circuito, pelo provador): HYG 1987 §3,
para trás a partir de `I_N = {p_N}`, em aritmética inteira exata
([`cert.rs`](../crates/shadow-proof/src/cert.rs)). Checagem por fold no
circuito: [`circuit.rs`](../crates/shadow-proof/src/circuit.rs).

**Argumento de soundness.** (a) O fold `n` impõe o aninhamento `f(I_n) ⊇ I_{n+1}`
com comparações inteiras exatas, escolhendo o ramo monótono pelo lado `σ_n`
(ele próprio checado); (b) `0 ≤ A ≤ B ≤ 2^K` em cada intervalo emitido (então
`I_N ≠ ∅`); (c) o verificador checa `z0` (x0 e `I_0`) e a distância final fora
do circuito. Por (a) e (b), escolhendo `x_N ∈ I_N` e andando para trás pelas
pré-imagens (que existem pela contenção), obtém-se `x_n ∈ I_n` para todo `n`,
com `x_{n+1} = f(x_n)`. A distância vem de (6) do circuito. ∎ (Argumento
manual; a unicidade de witness da parte PNF32 já foi verificada
exaustivamente. A parte de sombreamento não tem liberdade de saída: os
intervalos são estado, mas qualquer escolha aceita é um certificado válido,
então a afirmação vale para qualquer witness aceito.)

## 2. Custo

| circuito por passo | constraints | garante |
|---|---:|---|
| FP32 exato (repo original) | 1.721 | L2, só positivo-normal |
| prova dupla G1 (PNF32 + CBD) | 1.268 | L2 + L3 até ~30–100 passos |
| **PNF32 + sombreamento (G2)** | **1.035** | L2 + L3 retroativo, testado até 65.536 passos |
| PNF32 sozinho | ~424 | L2 |

Breakdown (`constraints.json`): ~424 da PNF32 e da decisão; ~611 do sombreamento.
São 7 range checks (dois de 122 bits para o aninhamento, que dominam), 2
produtos para as imagens dos extremos, 2 seleções de ramo e `pow2` para pôr
`p_n` na grade.

Nova (16 folds, r=3,9, x0=0,1, uma rodada, máquina compartilhada): 12.416
constraints primárias, prover 0,56 s, compressão 1,09 s, verificação comprimida
23 ms, prova de 10.880 B.

## 3. Até onde o sombreamento certifica

201 sementes em binary32 e 101 em binary64 (mesmo gerador de sementes do
ataque), r ∈ {3,7; 3,8; 3,9; 4,0}, horizontes 2^4 a 2^16. Tabela completa em
`results/shadow-20261018/summary.md`; recorte:

| formato | r | N | certificadas | ε mediano | ε p90 |
|---|---|---:|---:|---:|---:|
| binary32 | 3,9 | 64 | 201/201 | 5,1·10⁻⁷ | 2,6·10⁻⁶ |
| binary32 | 3,9 | 1.024 | 201/201 | 4,8·10⁻⁶ | 1,9·10⁻⁵ |
| binary32 | 3,9 | **65.536** | **201/201** | **1,1·10⁻⁵** | 3,7·10⁻⁵ |
| binary32 | 4,0 | 65.536 | 201/201 | 2,9·10⁻⁵ | 3,3·10⁻⁵ |
| binary64 | 3,9 | **65.536** | **101/101** | **8,9·10⁻¹³** | 5,1·10⁻¹² |

- **Em todas as configurações, 100% das sementes foram certificadas até 65.536
  passos**, exceto um único *breakdown* (binary32, r=3,9, semente 192, N=256,
  no passo 254: o intervalo final cai perto do máximo de `f`). A mesma semente é
  certificada em N=1.024. É o fenômeno de separação de HYG; o certificado
  depende do ponto final.
- ε cresce devagar com N e satura (~10⁻⁵ em binary32, ~10⁻¹² em binary64). Isso
  é bem melhor que a conjectura média de HYG (√δp ≈ 2·10⁻⁴ em binary32), em
  linha com o exemplo de Chow & Palmer.
- **Comparação com a prova dupla G1** (mesmo r=3,9): a decisão robusta caía de
  100% (N=10) para ~80% (N=30) e ~0% (N=100). Com sombreamento, **todo passo
  até 65.536 tem veredito robusto** no sentido retroativo: o lado de 1/2 coincide
  entre a órbita IEEE e a órbita verdadeira certificada. Isso é garantido por
  construção, porque o limiar 1/2 é o ponto crítico e o certificado mantém cada
  `I_n` do lado de `p_n`. Para outros limiares, a regra passa a ser
  `|p_n − t| > ε`.

## 4. O ataque contra o sombreamento (o insight confirmado)

Os witnesses adversariais do ataque (decisão invertida em N=24–76) também são
**sombreados** (`attack_*.json`):

| witness | N | honesto: ε | adversarial: ε | x0' do adversário |
|---|---:|---:|---:|---|
| binary32, δ=2^-24, r=3,9 | 26 | 1,0·10⁻⁷ | 1,5·10⁻⁷ | 0,1000000389 (x0 = 0,1000000015) |
| binary64, δ=2^-53, r=3,9 | 76 | 3,1·10⁻¹⁵ | 4,3·10⁻¹⁵ | 0,09999999999999994 |

A trajetória honesta e a adversarial têm decisões opostas em N, e **as duas**
são verdadeiras para alguma entrada a ≤ ~4·10⁻⁸ de x0. Conclusões:

1. **O sombreamento sozinho não impede a condução.** O adversário escolhe, entre
   as órbitas verdadeiras vizinhas, a que dá a decisão que ele quer.
2. **Nessa precisão, a decisão em N não é função de x0.** Ela muda dentro de uma
   bola de raio 10⁻⁷ em torno da entrada. Nenhum método de precisão finita
   determina "a" decisão matemática em x0 sem aumentar a precisão com N.
3. **O que o L2 exato acrescenta:** remove a escolha do provador. A decisão vira
   uma função determinística de `(x0, especificação IEEE)`, reproduzível e
   auditável. O sombreamento garante que essa escolha determinística corresponde
   a uma órbita real. **As duas peças juntas** dão uma afirmação que nem
   o provador manipula nem é falsa matematicamente.

## 5. Três regimes (síntese para a tese)

| regime | o que é possível afirmar | mecanismo |
|---|---|---|
| `N < n*` (previsível) | decisão da matemática exata **em x0**, robusta ou indeterminada | prova dupla G1 (PNF32 + envelope) |
| `N ≥ n*` (caótico) | a decisão IEEE é a de uma órbita verdadeira a ≤ ε de x0 (erro retroativo), e o provador não tem escolha | **G2: PNF32 + sombreamento** |
| qualquer N, sem L2 exato | o provador escolhe a decisão (com `x0'` vizinho a tiracolo) | — (é o ataque) |

## 6. Limitações

- Só o mapa logístico 1D com limiar 1/2 (o ponto crítico). Outros limiares
  precisam de `|p_n − t| > ε` no veredito; isso é trivial, mas não está implementado.
- 2D (Hénon, Ikeda) exige os parallelogramos de HYG 1988 ou dicotomias de
  Chow & Palmer: normas e direções estável e instável no circuito.
- O certificado é construído para um N fixo, a partir de `I_N = {p_N}`.
  Breakdowns no fim podem exigir escolher outro N ou reiniciar (HYG).
- A grade de 48 bits exige `p_n ≥ 2^-25`. Órbitas que tocam 0 (r=4) precisam de grade maior.
- Nova medido em uma rodada só e só com 16 folds. Faltam S passos por fold
  e medição pareada.
- O soundness é um argumento manual. Uma enumeração exaustiva em grade
  reduzida (como na PNF32) seria o próximo passo de verificação.
- A semântica retroativa ("verdade para x0' vizinho") precisa ser defendida
  por aplicação: para entradas medidas ou com ruído é natural; para entradas
  exatas por definição, não.

## 7. Comandos

```bash
cargo test -p shadow-proof --release
bash scripts/run_shadow_campaign.sh results/shadow-AAAAMMDD
./target/release/shadow-bench constraints --out results/<dir>/constraints.json
```
