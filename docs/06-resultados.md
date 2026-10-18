# 06 — Resultados (campanhas `results/pnf32-20261016`, `results/pnf32-spf-20261016` e `results/pnf32-spf-20261017`)

## Veredito

**Go.** As quatro hipóteses pré-registradas valem, inclusive a forma forte da H2.

| | critério | medido | |
|---|---|---|---|
| H1 | mul ≤ 150, add/sub ≤ 250 | mul **134**, add/sub **145** | ✅ |
| H2 | passo < 1.003 (forte: ≤ 709) | **415** | ✅ forte |
| H3 | 0 divergências / 0 flips, N=1.000, r=3,5 e 3,9 | 0 bits divergentes, 0 flips, `ε_max = 0` | ✅ |
| H4 | 1 witness aceito por entrada (exaustivo mini) | ≈ 18 M witnesses, unicidade OK (após correção) | ✅ |

Tempo de prova ([0010](progress/0010-perfil-e-s1024.md), que revisa o 0005): com
S passos por fold, a vantagem da PNF32 cresce com S. Em S = 1024, o prover é
**2,1× mais rápido que o FP32** e **1,4× mais rápido que o CBD-32**. Compressão,
verificação e memória ficam 3–4× menores que no FP32 (§2.1). O empate do 0005 vinha
de duas falhas de implementação da PNF32, já corrigidas.

## 1. Constraints (mesmo backend, determinístico)

| passo logístico + decisão | fidelidade | escopo | raw | Nova primário |
|---|---|---|---:|---:|
| FP32 exato (repo original) | bit-exata | positivo-normal | 1.721 | 13.097 |
| CFD v1 | enclosure | positivo | 1.003 | — |
| CBD-32 sequencial | enclosure | [0,1] | 782 | 12.159 |
| CBD-32 fusionado | enclosure | [0,1] | 709 | 12.086 |
| **PNF32** | **bit-exata** | **binary32 geral** | **415** | **11.794** |
| PNF32, ablação T2 (revalida a entrada a cada fold) | bit-exata | geral | 468 | 11.847 |
| PNF32, ablação `r` variável validada | bit-exata | geral | 475 | — |

Por operação: FP32 baseline mul 474 → PNF 134 (3,5×); `1-x` 924 → 141 (6,6×);
passo 1.721 → 415 (**4,1×**), e isso cobrindo mais casos que o baseline
(sinais, subnormais, ±0, inf, NaN). Breakdown em
[03-modelo-de-custo.md](03-modelo-de-custo.md). Os range checks ainda são 74%.

Atribuição parcial das técnicas: T2 vale 53 por entrada validada; dobrar `r`
como constante vale 60. A contribuição de T3/T4/T7 aparece na comparação
termo a termo com o baseline (03). **Não** foram implementadas ablações
isoladas de T3–T7 (exigiriam reescrever gadgets alternativos). Isso fica como
pendência declarada.

## 2. Nova

### 2.1 S até 1024, após as correções do 0010 (`results/pnf32-spf-20261017`, log [0010](progress/0010-perfil-e-s1024.md))

S ∈ {1, 4, 16, 64, 256, 1024}, 9 folds por configuração (`new` + 8
estacionários; 9·S passos), r = 3,7, x0 = 0,1, comprimido. São 30 repetições
pareadas: as 18 configurações rodam em ordem aleatória em cada repetição.
Mesmos baselines (`spf-baselines`) e mesma métrica do prover (`new + prove_step*`)
da §2.2.

**Tempo do prover por passo logístico** (ms; média [IC 95% t]; razões
pareadas = média geométrica [IC 95%]):

| S | PNF32 | FP32 | CBD-32 | PNF/FP32 | PNF/CBD |
|---:|---|---|---|---|---|
| 1 | 40,73 [37,52; 43,95] | 40,55 [37,49; 43,61] | 40,14 [36,92; 43,36] | 1,003 [0,984; 1,022] | 1,015 [0,993; 1,038] |
| 4 | 10,20 [9,36; 11,03] | 10,38 [9,54; 11,22] | 10,16 [9,37; 10,94] | 0,982 [0,947; 1,019] | 1,002 [0,973; 1,031] |
| 16 | 2,606 [2,401; 2,811] | 2,803 [2,593; 3,012] | 2,764 [2,524; 3,003] | 0,928 [0,905; 0,951] | 0,947 [0,908; 0,989] |
| 64 | 0,751 [0,693; 0,809] | 0,931 [0,861; 1,001] | 0,830 [0,768; 0,893] | 0,806 [0,783; 0,831] | 0,904 [0,877; 0,931] |
| 256 | 0,244 [0,228; 0,259] | 0,408 [0,378; 0,438] | 0,329 [0,306; 0,351] | 0,599 [0,574; 0,625] | 0,743 [0,726; 0,761] |
| 1024 | **0,134** [0,126; 0,142] | 0,291 [0,268; 0,314] | 0,191 [0,176; 0,206] | **0,466** [0,443; 0,489] | **0,707** [0,688; 0,727] |

**S = 1024, outras métricas:**

| | PNF32 | FP32 | CBD-32 |
|---|---:|---:|---:|
| constraints primárias | 435.316 | 1.773.680 | 737.393 |
| setup | 16,8 s | 62,7 s | 33,9 s |
| compressão | 20,2 s | 73,9 s | 36,5 s |
| verificação comprimida | 260 ms | 937 ms | 490 ms |
| prova comprimida | 12.376 B | 12.904 B | 12.624 B |
| pico de RSS | 1,8 GiB | 6,1 GiB | 3,6 GiB |

**Leitura.** Com 1 passo por fold as três empatam (o overhead do Nova domina).
A partir de S = 16 a PNF32 passa à frente, e a vantagem cresce com S. O
perfil por fase (`results/profile-20261017`) mostra a causa: com folds grandes,
o custo do fold passa a ser dominado pelo commitment do termo cruzado `T`, que
segue o número de entradas largas de `T`, ≈ 1 por constraint. Em S = 1024 esse
commitment custa 44 ms na PNF32, 161 ms no FP32 e 111 ms no CBD-32. A síntese
da PNF32 iguala a do FP32, e o circuito secundário é fixo (~18 ms). A sensibilidade (sem 8 repetições sob
carga externa, n = 22) dá PNF/FP32 = 0,479 [0,452; 0,508] e PNF/CBD = 0,723
[0,704; 0,744] em S = 1024. O perfil por fase está no 0010
(`results/profile-20261017`).

**Por que o 0005 empatou.** (1) Ao encadear passos, a combinação linear do
flag `nan` crescia 6 termos por passo, deixando a síntese O(S²) e as linhas
densas. (2) A PNF montava combinações lineares que o gerador de witness do Nova
descarta, e a síntese ficava ~4× mais lenta que a do FP32. As correções não
mudam a relação: o witness é idêntico entrada a entrada, e as constraints só
ganham +1 por passo encadeado.

### 2.2 Vários passos por fold, S ≤ 64, antes das correções (`results/pnf32-spf-20261016`, log [0005](progress/0005-steps-per-fold.md))

> **Superado pela §2.1.** Os tempos de prover da PNF32 aqui, para S ≥ 16,
> estão penalizados pelas duas falhas descritas na §2.1. Mantido como registro.

512 passos logísticos, S passos por fold (S ∈ {1, 4, 16, 64}), r = 3,7,
x0 = 0,1, comprimido. São 30 repetições pareadas: as 12 configurações rodam
em ordem aleatória em cada repetição. FP32 restrito e CBD-32 fundido vêm do
crate `spf-baselines`, que encadeia os circuitos originais de
`proof-numeric-zk` sem modificá-los. O tempo do prover é `new + prove_step*`
(o primeiro `prove_step` é no-op no Nova 0.73).

**Tempo do prover por passo logístico** (ms; média [IC 95% t]; razões
pareadas = média geométrica [IC 95%]):

| S | PNF32 | FP32 | CBD-32 | PNF/FP32 | PNF/CBD |
|---:|---|---|---|---|---|
| 1 | 46,12 [45,39; 46,85] | 45,60 [44,88; 46,31] | 45,40 [44,41; 46,40] | 1,011 [0,996; 1,027] | 1,016 [0,994; 1,039] |
| 4 | 11,96 [11,73; 12,19] | 11,98 [11,77; 12,19] | 12,06 [11,77; 12,35] | 0,998 [0,981; 1,015] | 0,992 [0,967; 1,019] |
| 16 | 3,212 [3,161; 3,263] | 3,378 [3,281; 3,476] | 3,296 [3,192; 3,400] | 0,952 [0,926; 0,980] | 0,977 [0,946; 1,008] |
| 64 | 1,059 [1,047; 1,071] | 1,054 [1,023; 1,084] | 0,968 [0,949; 0,986] | 1,007 [0,981; 1,034] | 1,095 [1,073; 1,117] |

**Onde as constraints contam** (S = 64):

| | PNF32 | FP32 | CBD-32 |
|---|---:|---:|---:|
| constraints primárias | 37.813 | 121.520 | 56.753 |
| compressão | 4,14 s | 6,79 s | 3,98 s |
| verificação comprimida | 57 ms | 94 ms | 59 ms |
| prova comprimida | 11.440 B | 11.656 B | 11.376 B |
| pico de RSS | 325 MiB | 635 MiB | 367 MiB |

Compressão pareada PNF/FP32: 1,09 (S=1), 0,72 (4), 0,66 (16), 0,61 (64)
[IC em 0,583–0,638 em S = 64]. Contra o CBD, a PNF fica entre 1,03 e 1,10.

**Leitura honesta.** Agrupar passos amortiza o overhead do Nova: o tempo por
passo cai ~43× de S = 1 para S = 64. Mas cai igual para as três
representações. A hipótese "menos constraints ⇒ prover mais rápido" foi
**refutada** para o folding. O custo de um fold quase não depende do tamanho
do circuito nessa faixa: ~71 ms para a PNF com 38 mil constraints contra
~73 ms para o FP32 com 122 mil. A PNF só ganha do FP32 em S = 16 (−5%) e perde
9,5% para o CBD-32 em S = 64.

A redução de constraints vira ganho onde o custo escala com o tamanho do
circuito: compressão (−39% vs FP32), verificação (comprimida −39%, recursiva
−56%), setup (−43%) e memória (−49%). Contra o CBD-32 (não bit-exato), a PNF
empata nessas métricas, mas é IEEE bit-exata. A hipótese de por que o fold
não escala (witness dominado por bits, commitment barata de valores pequenos)
ainda precisa de profiling.

### 2.3 Smoke de 10 folds, 1 passo por fold (campanha `results/pnf32-20261016`)

Mantido como registro do 0004. Os tempos de prover desta tabela usam a métrica
antiga, que só cronometra o laço de `prove_step` e perde o primeiro fold (ver
2.2).


| | FP32 | CBD-32 fus. | PNF32 |
|---|---:|---:|---:|
| prover 10 folds (média) | 0,397 s (1 rodada) | 0,446 s (1) | 0,437 s (3) |
| compressão | 1,18 s | 1,63 s | 1,58 s |
| verificação comprimida | 29,8 ms | 30,4 ms | ~30 ms |
| prova | 10.720 B | 10.752 B | 10.816 B |
| estado (elementos) | 5 | 6 | 8 |

**Leitura honesta:** com 1 passo por fold, o overhead fixo do circuito
aumentado do Nova (~11,4 mil constraints) domina. A redução de 1.306
constraints é ~10% do circuito aumentado, e o tempo de prova **não** melhorou de
forma mensurável (smoke de 1–3 rodadas; as diferenças estão no ruído). A prova é
96 B maior porque o estado tem 8 elementos. A vantagem em tempo só deve
aparecer com muitas operações por fold (vários passos por fold, workloads com
dezenas de ops por passo), o que ainda **não foi medido**.

O estado final de cada execução Nova foi conferido contra o f32 nativo
(bits e decisão).

## 3. Fidelidade (H3)

A tabela do paper ganha uma linha (`x0 = 0,1`, N = 1.000, `x_n >= 0,5`):

| r | codificação | `ε_max` | 1º flip | flips |
|---|---|---:|---:|---:|
| 3,9 | Q16.16 | 0,878 | 17 | 477 |
| 3,9 | Q8.8 | 0,831 | 11 | 444 |
| 3,9 | Approx-FP | 0,877 | 26 | 491 |
| 3,9 | 8-bit quant. | 0,896 | 9 | 464 |
| 3,9 | **PNF32** | **0** | **—** | **0** |
| 3,5 | **PNF32** | **0** | **—** | **0** |

Gerado passo a passo **pelo circuito** (CheckCS, todas as constraints
satisfeitas) e comparado bit a bit com o f32 nativo: `trajectory_r3.5.csv` e
`trajectory_r3.9.csv`.

Ressalva conceitual (vem do repo original): bit-exatidão com FP32 é fidelidade
à *referência FP32 da aplicação* (nível L2 do paper). Não é fidelidade à
matemática exata. Em caos, o próprio FP32 diverge da trajetória real; para essa
pergunta, a resposta continua sendo CBD/CDAD com abstenção.

## 4. Validação (detalhes em [05-validacao.md](05-validacao.md))

- Oráculo × f32 nativo: 6 M ops amostradas + **varredura completa de 2^32**
  (decode/encode, `1-x`, `x·3,9`).
- Circuito × oráculo: 600 mil ops binary32 + near path + casos difíceis + adversarial.
- Unicidade exaustiva nos formatos `(3,3)` e `(4,4)`.
- **Achados:** (i) bug de soundness no `add`, achado pelo exaustivo e corrigido;
  (ii) o crate Rust `softfloat` 1.0 erra em subnormais (12.851/6 M).

## 5. Limitações

1. Unicidade exaustiva só em formatos mini; para binary32 vale argumento
   manual + testes. Não há prova mecanizada.
2. Berkeley SoftFloat (C) ainda não rodou neste projeto (Docker inativo); os
   árbitros usados foram o hardware aarch64 e o oráculo exato.
3. Os tempos da §2.3 são smoke (1–3 rodadas). As §2.1 e §2.2 usam 30 rodadas
   pareadas, mas uma carga e uma máquina só, sem isolamento de CPU.
4. Ablações isoladas de T3–T7 não implementadas.
5. Só add/sub/mul/decisão. Div, sqrt, fma e conversões ficam fora da v1, assim
   como modos de arredondamento além do RNE e as flags IEEE (inexact etc.).
6. NaN canônico (sem payload). Para bater com x86 (que propaga payload), seria
   preciso carregar o payload no estado.
7. A literatura ([01](01-literatura.md)) ainda precisa de busca sistemática
   antes de qualquer alegação de originalidade.

## Próximos passos

1. ~~Vários passos por fold~~, ~~profiling e S ≥ 256~~ feitos no
   [0005](progress/0005-steps-per-fold.md) e no [0010](progress/0010-perfil-e-s1024.md):
   o prover da PNF32 fica 2,1× mais rápido que o FP32 em S = 1024.
2. Rodar `scripts/run_berkeley_validation.sh` (ligar o OrbStack).
3. Workloads do repo original: recorrência afim estável, score Adult e ECEF
   (somas de produtos), com mais ops por passo.
4. **Lookups**: portar as mesmas relações para Plonkish/LogUp (Halo2) ou para
   folding com lookups. Como 74% são range checks, o esperado é mais 3–5×.
5. Reduzir o resto de 49 bits da mul (corte variável) e estudar fma (um
   arredondamento só).
6. binary64 (a parametrização já é genérica; checar larguras no corpo).
7. Verificação formal das constraints (SMT/Lean) para binary32.
8. Texto: a contribuição para a tese/paper seguinte é "IEEE bit-exato não é
   proibitivo: 415 constraints/passo, abaixo de representações não-IEEE".
