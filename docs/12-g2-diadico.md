# 12 — G2 (sombreamento) sobre o perfil diádico fundido

**Data:** 2026-10-23 · Código: [`crates/dyadic-fused/src/shadow.rs`](../crates/dyadic-fused/src/shadow.rs) ·
Campanha: `results/dyadic-g2-20261023/` · Registro: [0014](progress/0014-g2-diadico.md)

Pergunta: qual é a combinação mais barata de **trajetória sem liberdade para o
provador** (L2 ao perfil) com **verdade para qualquer N** (sombreamento: existe
uma órbita verdadeira do mapa real a ≤ ε, partindo de x0' a ≤ ε de x0)? E qual
é o menor p cujo ε fica abaixo do ruído da entrada do oráculo?

## 1. O fold

Estado `[contador, X, d, A, B, veredito]`. O fold junta:

- o passo diádico fundido (doc 11): uma constraint de produto e os bits de Y e do
  resto;
- a checagem local do certificado de Hammel–Yorke–Grebogi, a mesma do
  `shadow-proof` (doc 09): lado de 1/2, próximo intervalo, aninhamento
  `f(I_n) ⊇ I_{n+1}` e distância `|p_n − x| ≤ ε`;
- o veredito: robusto se o lado certificado coincide com a decisão do estado.

Diferença em relação à PNF + G2: `p_n` já está na grade binária
(`X·2^(K−p)`, linear). Por isso **não** há `pow2` nem checagens de
classe/expoente para levar o número à grade.

O certificado usa o mesmo `r` do perfil (`R = r·2^22`, valor exato de `fl32(r)`)
e o mesmo construtor exato do `shadow-proof` (`build_grid`, refatoração sem
mudança de comportamento: os testes do `shadow-proof` continuam passando).

**Duas larguras de range check:**

- **`full`:** as mesmas larguras do G2 da PNF. Serve para isolar o efeito da
  representação.
- **`tight`:** cada range check com teto igual à folga **honesta**. A
  desigualdade imposta é a mesma (valor ≥ 0), então a soundness não muda:
  - aninhamento: o construtor de HYG escolhe o extremo mais justo, e a folga é
    menor que um degrau de f, `rn·2^K`. Largura `bits(rn)+K+1` em vez de
    `2K+25`.
  - distância e largura do intervalo: ficam em `[0, 2ε]`. Largura `bits(2ε)+1`
    em vez de `K+2`.
  - `B'` não precisa de range próprio.

  Esse aperto vale igualmente para a PNF + G2, que não foi remedido aqui.

**Escolha de K:** K = p+8 já certifica 100% (varredura exploratória, p de 12 a
110). A conta do corpo é `2K + bits(r) + 2 ≤ 253`, então K ≤ 113. Isso corrige a
estimativa anterior de "p ≲ 83", que usava a largura errada.

## 2. Custo por fold

| configuração | constraints/fold | contra PNF32 + G2 (1.035) |
|---|---:|---:|
| PNF32 + G2 (K=48, `full`) — referência | 1.035 | 1,00× |
| diádico p=24, K=48, `full` (mesma grade e mesmos 24 bits) | 678 | 0,66× |
| diádico p=24, K=48, `tight` | 471 | 0,46× |
| diádico p=20, K=28, `tight` | **309** | 0,30× |
| diádico p=24, K=32, `tight` | **343** | 0,33× |
| diádico p=32, K=40, `full` | 606 | 0,59× |
| **diádico p=32, K=40, `tight`** | **393** | **0,38×** |
| diádico p=48, K=56, `tight` | 508 | 0,49× |
| diádico p=64, K=72, `tight` | 617 | 0,60× |
| diádico p=96, K=104, `tight` | 844 | 0,82× |
| diádico p=110, K=113, `tight` | 902 | 0,87× |

Decomposição do efeito, com a mesma grade K=48 e os mesmos 24 bits:

- **representação:** de 1.035 para 678 (−34%), porque não há mais o passo IEEE
  nem o alinhamento à grade;
- **larguras estreitas:** de 678 para 471 (−31%).

Com K = p+8, o perfil p=32 custa **393**, 38% da PNF32 + G2.

Nova (16 folds, uma rodada, smoke): ~11.8–12,0 mil constraints primárias (o
overhead fixo do Nova domina), prover ~0,7 s, compressão ~1,5 s, verificação
comprimida ~30 ms, prova de 10.752 B. O estado final confere com a referência,
e o veredito final é robusto.

## 3. Certificação e ε

1.001 sementes do ataque, r ∈ {3,7; 3,9; 4,0}, N = 2^4 … 2^12 em 7 perfis, e
N até 2^16 = 65.536 em p=32 com r=3,9. **Todos os 202.202 certificados foram
emitidos, sem nenhum breakdown.** Tabela completa em `summary.md`, onde a
linha p=32, r=3,9, N ≤ 4.096 soma as duas varreduras e por isso mostra
2002/2002.

Pior erro retroativo total em x0 (ε máx em todos os r e N, mais o
arredondamento da entrada para p < 30), comparado com o ruído do preço do
oráculo, `δx = 1,5·ρ`:

| p | K | pior ε total | ρ = 0,05% (δx = 7,5·10⁻⁴) | ρ = 0,5% |
|---:|---:|---:|---|---|
| 20 | 28 | 4,0·10⁻⁴ | ok, folga de 2× | ok, 19× |
| 24 | 32 | 1,2·10⁻⁴ | ok, 6× | ok, 60× |
| **32** | **40** | **7,3·10⁻⁶** | **ok, 103×** | ok, 1.028× |
| 48 | 56 | 7,4·10⁻⁹ | ok, 10⁵× | |
| 64 | 72 | 5,1·10⁻¹⁴ | ok, 10¹⁰× | |

ε mediano em p=32, r=3,9: 2,4·10⁻⁸ em N=1.024 e 8,5·10⁻⁷ em N=65.536. Ele
satura, como no G2 da PNF.

**Observação (p pequeno):** com p ≤ 20, a pseudo-órbita diádica cai em ciclos
curtos (a órbita de precisão finita é periódica) e o ε satura cedo
(p=20: mediana 1,5·10⁻⁴ a partir de N≈1.024). O certificado continua válido:
existe uma órbita verdadeira perto do ciclo. Mas é exatamente o regime em que
as **estatísticas** de longo prazo da simulação ficam erradas (Boghosian et al.,
G4). O sombreamento garante a trajetória, não a estatística.

## 4. Leitura

1. **A combinação mais barata medida de "L2 + verdade para qualquer N" é o
   diádico p=32 + G2 estreito: 393 constraints por fold.** Isso é 38% da PNF32 +
   G2 (1.035) e menos que a PNF32 sozinha sem sombreamento (415). O ε fica 100×
   abaixo do menor ruído considerado, em 65.536 passos.
2. **Com G2, p deixa de definir o horizonte e passa a definir só ε.** A escolha
   de p é pelo ruído da entrada: p=24 já basta para ρ=0,05% (6× de folga), p=32
   dá 100×.
3. **Custo Θ(N) confirmado na prática:** o custo por fold não depende de N, e
   a certificação funciona igual de N=16 a N=65.536.
4. O ganho vem de duas fontes separáveis: a representação (−34%, mesma grade)
   e o aperto das larguras (−31%). O segundo também se aplica à PNF.

## 5. Limitações

- Só o mapa logístico em 1D e o limiar 1/2, que é o ponto crítico: o veredito
  é sempre robusto. Para outros limiares a regra é `|p_n − t| > ε`, ainda não
  medida aqui.
- As larguras estreitas assumem o certificado justo produzido pelo construtor
  de HYG. Um certificado válido, mas frouxo, pode ser rejeitado. Isso afeta a
  completude, não a soundness.
- O argumento de soundness do G2 é manual (o mesmo do doc 09). A unicidade do
  passo diádico é exaustiva (doc 11). A parte do certificado não tem
  enumeração exaustiva.
- A semântica é de **erro retroativo** (verdade para x0' a ≤ ε de x0). Ela só é
  aceitável quando ε fica abaixo do ruído da entrada (§3).
- Nova medido em uma rodada só, com 1 passo por fold.

## 6. Comandos

```bash
bash scripts/run_dyadic_g2.sh results/dyadic-g2-AAAAMMDD
./target/release/dyadic-bench shadow-constraints --profiles 32:40 --output <arquivo novo>
./target/release/dyadic-bench shadow-scan --profiles 32:40 --r 3.9 --seeds 1000 --nexp 16 --output <arquivo novo>
./target/release/dyadic-bench shadow-nova --p 32 --k 40 --widths tight --output <arquivo novo>
```
