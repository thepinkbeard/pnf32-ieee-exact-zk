# 0009 — Protótipo da prova dupla L2+L3 (PNF32 + CBD num passo Nova)

**Data:** 2026-10-16 · **Estado:** concluído. A composição custa a soma das
partes + 6 constraints. A decisão robusta nunca contradiz a referência exata e
sinaliza 100% das decisões binary32 erradas.

## 1. Objetivo e estado
Construir a "defesa" do plano ataque+defesa da tese: num único passo Nova,
provar (a) o passo logístico binary32 bit-exato (PNF32), (b) um enclosure
certificado da trajetória em matemática exata (CBD de `proof-numeric-zk`) e
(c) uma decisão trivalorada combinada. Todos os entregáveis estão prontos:

- crate `dual-proof` com 11 testes;
- breakdown de constraints;
- Nova com 10 folds e compressão;
- cobertura em 1.000 sementes nos horizontes 10/30/100;
- [docs/08](../08-prova-dupla-prototipo.md) com enunciado, soundness, custos e
  limitações.

## 2. Arquivos
- `crates/dual-proof/` (**crate novo**):
  - `src/lib.rs`: `DualCircuit<K>` (`StepCircuit`), `combine` (gadget de 6
    constraints), `Verdict`, `check_z0`, `f32_ratio`/`f32_to_rational`,
    simulação fora do circuito (`simulate`, `record`) e `breakdown`;
  - `src/main.rs`: binário `dual-bench` (`constraints`, `nova`, `coverage`);
  - `tests/dual.rs`: 11 testes.
- `Cargo.toml`: membro novo. `Cargo.lock` ganhou as dependências de
  `nova-circuit`/`numeric-reference` (rug/GMP).
- `results/dual-20261016/`:
  - `constraints.json`;
  - `nova_k2_n10.json`, `nova_k2_model_n10.json`, `nova_k4_n10.json` e
    `nova.log`;
  - `nova_pnf32_only_n10.json` e `nova_pnf32_only.log`, baseline
    contemporâneo pelo `pnf-bench` sem mudança;
  - `coverage.json`, `coverage.csv` (12.000 linhas) e `coverage.log`;
  - `tests.log` e `MANIFEST.md`.
- `docs/08-prova-dupla-prototipo.md` e este log.
- **Nada** em `pnf-circuit`, `pnf-reference`, `pnf-bench` ou `spf-baselines`
  foi alterado: tudo de que a composição precisava já era público. Nada em
  `../proof-numeric-zk` foi alterado.

## 3. Decisões e hipóteses
- **Reuso como caixa-preta.** `LogisticCircuit::synthesize` e
  `CbdLogisticStepCircuit::synthesize` são chamados em namespaces próprios
  sobre fatias do estado. Nenhum gadget foi copiado, e o embutido conta
  exatamente o mesmo que o isolado (conferido por `assert`).
- **Contador compartilhado.** A CBD recebe a mesma variável de contador da
  PNF32, e a saída de contador da CBD é descartada. Isso fixa que L2 e L3
  falam do mesmo índice sem nenhuma constraint extra. Custo: 1 constraint
  redundante dentro da CBD.
- **Qual `r` a CBD usa.** O padrão é o mesmo número que a PNF32 usa,
  `val(fl(3,9)) = 8178893/2^21` ("mesma entrada"), de modo que o enclosure
  isola o erro de arredondamento. A variante `--model-r 39/10` ("modelo") mede
  também o erro de representação de `r` e reproduz o r do baseline CBD-32
  (709). Os custos diferem: 847 contra 709.
- **Bound de raio:** `LocalTaylor`, o mesmo do CBD-32 de 709 constraints.
- **Combinação:** `d_cbd = a + 2b`, `RT = a·d`, `RF = (1−a−b)(1−d)`,
  `v = 2 − RT − 2RF`. O código de veredito é o mesmo da CBD (0/1/2).
- **`z0` checado fora do circuito** (`check_z0` exige o estado honesto), na
  mesma linha da T2 da PNF32.
- **Sementes:** `x0 = u/2^24` (exatas em binary32 e na grade CBD, raio
  inicial 0), via splitmix64 com semente 20261016.
- **Referência exata:** racional exato até n = 12 (o denominador dobra a cada
  passo) e intervalo dirigido MPFR de 4.096 bits até n = 100.

## 4. Comandos
```bash
cargo test --workspace --release                  # tests.log
D=results/dual-20261016; B=target/release/dual-bench
$B constraints --output $D/constraints.json
$B nova --k 2 --steps 10 --compress --reps 3 --output $D/nova_k2_n10.json
$B nova --k 2 --steps 10 --model-r 39/10 --compress --reps 3 --output $D/nova_k2_model_n10.json
$B nova --k 4 --steps 10 --compress --reps 3 --output $D/nova_k4_n10.json
target/release/pnf-bench nova --steps 10 --r 3.9 --x0 0.3 --compress --reps 3 --output $D/nova_pnf32_only_n10.json
$B coverage --seeds 1000 --output $D/coverage.json   # grava também coverage.csv
```
Todos os subcomandos recusam sobrescrever arquivos.

## 5. Testes, métricas e evidências
**Testes** (`crates/dual-proof/tests/dual.rs`, 11, todos verdes; workspace
inteiro verde em `tests.log`):

- `ieee_output_is_bit_exact_k2`/`_k4`: 6 casos × 120 folds (`CheckCS`
  encadeado). O estado PNF32 bate bit a bit com o f32 nativo, e `d_ieee`, o
  estado CBD e o veredito batem com a simulação. Os casos incluem x0 inexato
  na grade (1e−12) e r = 4 com `is_one`.
- `enclosure_contains_exact_rational_trajectory`: a trajetória racional exata
  (`numeric_reference::logistic_trajectory`) fica dentro de `[c−R, c+R]` até
  n = 12, com K = 2 e K = 4. Vereditos robustos batem com o sinal exato.
- `enclosure_contains_directed_reference_long_horizon`: o intervalo dirigido
  de 4.096 bits fica dentro do enclosure até n = 100; os três vereditos são
  exercitados.
- `model_variant_encloses_decimal_r_trajectory`: o mesmo, com `r* = 39/10`.
- `combination_is_unique_and_correct`: para cada `(d_ieee, d_cbd)`, das 6^5
  atribuições adulteradas de `(a, b, RT, RF, v)` em {0, 1, 2, 3, −1, 1/2}, só
  a honesta satisfaz as constraints.
- `breakdown_adds_up` (total = PNF + CBD + 6; PNF = 415),
  `test_constraint_system_agrees`, `z0_check_rejects_inconsistent_initial_state`,
  `r_and_x0_are_the_same_numbers_in_both_parts` e
  `nova_proves_dual_trajectory` (prova Nova real com `z_n` igual ao
  encadeamento `CheckCS`).

**Constraints por fold:**

| CBD r | K | total | PNF + CBD + combinação |
|---|---:|---:|---|
| fl(3,9) | 2 | 1.268 | 415 + 847 + 6 |
| fl(3,9) | 4 | 1.852 | 415 + 1.431 + 6 |
| 39/10 | 2 | 1.130 | 415 + 709 + 6 |
| 39/10 | 4 | 1.714 | 415 + 1.293 + 6 |

**Nova, 10 folds:**

- Constraints primárias: 12.653 (K = 2) e 14.793 (K = 4), contra 11.794 da
  PNF32 sozinha.
- Prova comprimida: 11.008 / 11.072 B, contra 10.816 B.
- Todas as provas verificaram (recursiva e comprimida), e o estado final foi
  conferido contra o f32 nativo e a simulação.
- Tempos empatados, mas sob load ≈ 53 (ver §7).

**Cobertura** (mesma entrada; robusto / IEEE errada / sinalizada / robusto
errado):

| K | n = 10 | n = 30 | n = 100 |
|---:|---|---|---|
| 2 | 100% / 0 / 0 / 0 | 79,5% / 197 / 197 / 0 | 0% / 488 / 488 / 0 |
| 4 | 100% / 0 / 0 / 0 | 80,3% / 197 / 197 / 0 | 0,8% / 488 / 488 / 0 |

Com K = 4 e n = 30, todos os indeterminados são conflitos IEEE × enclosure
decisivo, ou seja, certificados de que o binary32 erra. O modo modelo (39/10)
é análogo (206 erradas, todas sinalizadas). O veredito foi reproduzido no
circuito nas 20 primeiras sementes de cada (modo, K) até n = 100.

## 6. Tentativas malsucedidas
- O primeiro `breakdown_adds_up` supôs que a forma não dependia de `r` e
  falhou (847 contra 855): o custo da CBD depende da largura de `r_num`/`r_den`.
  O teste foi corrigido para comparar formas com o mesmo `r`. Isso também
  revelou que `r = fl(3,9)` custa +138 constraints em relação a 39/10.
- A primeira medição Nova saiu ~4× mais lenta que no 0005 (0,19 contra
  0,046 s/passo da PNF32). A causa é carga externa (load average 53), não o
  circuito: o baseline PNF32 medido no mesmo momento teve a mesma lentidão.

## 7. Limitações
- Tempos sob carga pesada, com 3 repetições e sem pareamento; só constraints
  e bytes são firmes.
- Apenas o veredito do último passo é exposto (não há acumulador
  "todos robustos"); um passo por fold; `r` e o limiar 1/2 são constantes.
- `K ≤ 4` limita o horizonte útil a ≲ 80 passos com r = 3,9 (saturação
  estimada em n ≈ 85).
- A soundness da CBD é herdada de `proof-numeric-zk`, com evidência empírica
  aqui; `z0` é checado fora do circuito.
- Um único `r` e uma família de sementes.

## 8. Próximo passo
1. Medição pareada em máquina ociosa (30 repetições, como no 0005): PNF32
   sozinha × dual K = 2/K = 4 × FP32 exato.
2. Acumulador de veredito ("todos os passos robustos" e "primeiro passo
   indeterminado") e `steps_per_fold` na prova dupla.
3. Horizonte 100: escalada CDAD (K > 4 ou degraus ligados) e medição do custo
   para manter o veredito robusto.
4. Cruzar com o ataque (sessão irmã): mostrar que um provador Garg-style com
   erro relaxado pode virar a decisão, enquanto a prova dupla devolve
   `Indeterminate` ou recusa.

## 9. Continuidade para outra IA
Comece por `crates/dual-proof/src/lib.rs`, cuja doc do módulo traz o layout
do estado, e por [docs/08](../08-prova-dupla-prototipo.md) §1–2. O ponto
sutil é que o custo da CBD depende de `r` (constante da forma). Use sempre
`DualCircuit::with_model_r` com o mesmo `r` ao comparar formas. Para novas
medições, crie um diretório `results/dual-AAAAMMDD-*` novo: os binários
recusam sobrescrever.
