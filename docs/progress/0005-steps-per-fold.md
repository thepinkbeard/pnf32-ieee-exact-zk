# 0005 — Vários passos por fold (`steps_per_fold`) e medição pareada

**Data:** 2026-10-16 · **Estado:** concluído. Hipótese de tempo de prover
**refutada**; compressão, verificação comprimida e memória **confirmam** a vantagem.

## 1. Objetivo e estado
Testar se, com S passos logísticos por fold Nova, a redução de constraints da
PNF32 (415 vs 1.721 do FP32 e 709 do CBD-32) vira redução do tempo de prova.
Medido S ∈ {1, 4, 16, 64}, 30 repetições pareadas, mesmo total de 512 passos.

Resultado: o tempo do prover por passo cai ~43× de S = 1 para S = 64 nas três
representações, mas **as três ficam empatadas**. Em S = 64, PNF/FP32 = 1,007
[0,981; 1,034] e PNF/CBD = 1,095 [1,073; 1,117], ou seja, a PNF é 9,5% *mais
lenta* que o CBD-32. A redução de constraints aparece onde o custo escala com o
tamanho do circuito: compressão (PNF/FP32 = 0,610 [0,583; 0,638]),
verificação comprimida (57 vs 94 ms) e pico de RSS (325 vs 635 MiB).

## 2. Arquivos
- `crates/pnf-circuit/src/step.rs`: campo `steps_per_fold` em
  `LogisticCircuit`; `trajectory` avança S passos por fold.
- `crates/pnf-circuit/tests/steps_per_fold.rs`: 5 testes novos.
- `crates/pnf-bench/src/main.rs`: `nova --steps-per-fold`; novas métricas.
- `crates/spf-baselines/` (**crate novo**): `MultiStep<C>` genérico sobre os
  circuitos de `../proof-numeric-zk/crates/nova-circuit` via path dependency
  (FP32 restrito e CBD-32 fundido, local-Taylor, K = 2), binário
  `spf-baselines` e `tests/chaining.rs`. `proof-numeric-zk` não foi modificado.
- `Cargo.toml` (membro novo), `scripts/run_spf.sh`, `scripts/analyze_spf.py`.
- `results/pnf32-spf-20261016/`: `manifest.txt`, `tests.log`, `order.csv`,
  `run.log`, `raw/*.json` (360 medições), `summary.json`, `summary.md`.
  O log do driver fica em `results/pnf32-spf-20261016.log`.

## 3. Decisões e hipóteses
- **Semântica do fold.** Um fold com S passos produz exatamente o estado de S
  folds de um passo: contador `+S`, `x` bit a bit, decisão do último passo.
  Cada passo calcula a decisão (custo "passo + decisão", como no 0004). As
  decisões intermediárias são restritas, mas não ficam expostas no estado.
  A saída de um passo entra direto no seguinte (`PnfVar`, sem `to_nums`). A
  entrada do fold continua sem revalidação (T2).
- **Baselines como caixa-preta.** `MultiStep<C>` encadeia S cópias do
  `StepCircuit` original, cada uma num namespace próprio, e não copia nenhum
  gadget. Assim cada passo mantém as checagens de entrada do baseline, e o
  custo é exatamente `S × raw` (1.721 e 709). Um baseline reescrito poderia
  dispensar essas checagens nos passos internos. Isso favorece levemente a
  PNF, mas não muda a conclusão: o tempo do prover empatou mesmo assim.
- **Métrica de prover corrigida.** Em Nova 0.73, `RecursiveSNARK::new` já
  prova o primeiro fold e o primeiro `prove_step` é no-op. O cronômetro do 0004
  pegava só o laço de `prove_step`, então perdia um fold (com 1 fold mediria
  ≈ 0). A métrica principal agora é `prover_ivc_seconds = new + prove_step*`,
  dividida pelo total de passos. Também é gravado o regime estacionário,
  `prove_step / ((folds-1)·S)`. `prover_total_seconds` foi mantido com o
  significado antigo para comparação.
- **Mesmo total de passos (512) para todo S**, para que S = 64 tenha 8 folds
  (7 estacionários). Com 64 passos, S = 64 seria só o `new`, sem dobra.
- **Desenho pareado.** As 12 configurações (3 representações × 4 valores de S)
  rodam em ordem aleatória dentro de cada repetição (semente 5, `order.csv`),
  um processo por medição. As razões PNF/baseline são calculadas por
  repetição; o resumo é a média geométrica, com IC 95% t sobre o log.
- **Sensibilidade**, com critério fixado antes de ver as razões: descartar
  repetições inteiras cuja soma de setup + prover + compressão passasse de
  1,5× a mediana. Nenhuma passou (faixa de 154 a 169 s; mediana 158 s).

## 4. Comandos
```bash
cargo test --workspace --release
cargo test -p pnf-circuit --release --test exhaustive_uniqueness -- --include-ignored
scripts/run_spf.sh results/pnf32-spf-20261016   # roda os dois acima antes de medir
# medição avulsa:
target/release/pnf-bench nova --steps 512 --steps-per-fold 16 --compress --output <novo>.json
target/release/spf-baselines --repr fp32 --steps 512 --steps-per-fold 16 --compress --output <novo>.json
python3 scripts/analyze_spf.py results/<dir>     # recusa sobrescrever summary.*
```

## 5. Testes, métricas e evidências
**Testes** (`tests.log`): workspace inteiro verde, inclusive os testes novos
(`steps_per_fold`: 5; `chaining`: 3). A unicidade exaustiva com
`--include-ignored` passou 5/5 (`add_mini8` em 177 s).

Os testes novos verificam:
- fold de S passos = S folds de um passo, para S ∈ {1, 2, 4, 8} e entradas
  normal, divergente para inf/NaN, subnormal, ±0, inf e NaN;
- estado final igual ao f32 nativo após 320 passos;
- constraints lineares em S;
- `TestConstraintSystem` concorda com `CheckCS`;
- prova Nova real com S = 4 × 2 folds dando o mesmo `z` que S = 1 × 8;
- o mesmo para os baselines, com custo `S × raw` exato.

Toda medição conferiu o estado final: na PNF32 contra o f32 nativo (bits,
contador e decisão); no FP32 contra a referência e o f32 nativo; no CBD-32
contra a referência fora do circuito.

**Tempo do prover por passo logístico** (ms; `new + prove_step*` / 512;
média [IC 95%]; n = 30 pareadas):

| S | PNF32 | FP32 | CBD-32 | PNF/FP32 | PNF/CBD |
|---:|---|---|---|---|---|
| 1 | 46,12 [45,39; 46,85] | 45,60 [44,88; 46,31] | 45,40 [44,41; 46,40] | 1,011 [0,996; 1,027] | 1,016 [0,994; 1,039] |
| 4 | 11,96 [11,73; 12,19] | 11,98 [11,77; 12,19] | 12,06 [11,77; 12,35] | 0,998 [0,981; 1,015] | 0,992 [0,967; 1,019] |
| 16 | 3,212 [3,161; 3,263] | 3,378 [3,281; 3,476] | 3,296 [3,192; 3,400] | **0,952** [0,926; 0,980] | 0,977 [0,946; 1,008] |
| 64 | 1,059 [1,047; 1,071] | 1,054 [1,023; 1,084] | 0,968 [0,949; 0,986] | 1,007 [0,981; 1,034] | **1,095** [1,073; 1,117] |

No regime estacionário (sem `new`), em S = 64: PNF/FP32 = 0,978 [0,952; 1,006]
e PNF/CBD = 1,061 [1,039; 1,084]. Ganho de S = 64 sobre S = 1 (razão
pareada): PNF 0,023, FP32 0,023, CBD 0,021.

**Custo de um fold** (prover estacionário por fold ≈ tempo por passo × S): de
~46 ms (12–13 mil constraints primárias, S = 1) para ~71 ms (PNF, 37.813), ~73 ms
(FP32, 121.520) e ~67 ms (CBD, 56.753) em S = 64. O fold quase não depende do
número de constraints nessa faixa: o FP32 tem 3,2× as constraints da PNF e
custa o mesmo.

**Compressão, verificação e memória** (S = 64; média ± meia-largura do IC):

| | PNF32 | FP32 | CBD-32 |
|---|---:|---:|---:|
| constraints primárias | 37.813 | 121.520 | 56.753 |
| compressão (s) | 4,14 ± 0,12 | 6,79 ± 0,21 | 3,98 ± 0,12 |
| verificação comprimida (ms) | 57,4 ± 1,5 | 94,3 ± 2,2 | 58,7 ± 3,7 |
| verificação recursiva (ms) | 51,5 ± 2,2 | 116,5 ± 6,1 | 84,9 ± 2,2 |
| setup (s) | 3,36 ± 0,17 | 5,93 ± 0,23 | 3,36 ± 0,13 |
| prova comprimida (B) | 11.440 | 11.656 | 11.376 |
| pico de RSS (MiB) | 325 | 635 | 367 |

Compressão pareada PNF/FP32: 1,093 (S=1), 0,718 (4), 0,657 (16), 0,610 (64).
PNF/CBD: entre 1,025 e 1,098. As tabelas completas estão em `summary.md`.

## 6. Tentativas malsucedidas
- No piloto, a métrica antiga (só `prove_step`) com 64 passos e S = 64 media
  um fold só pelo `new` e daria tempo ≈ 0. Isso levou à correção da métrica
  e ao total de 512 passos.
- No smoke (2 repetições, 64 passos), `analyze_spf.py` quebrava sem regime
  estacionário (1 fold). Corrigido para imprimir "—".
- Durante a repetição 13 (21:15–21:40 UTC), o relógio de parede do driver
  parou ~24 min. Isso coincidiu com a carga alta de outra sessão na máquina
  (load 28 em 15 núcleos) e com a interrupção de uma ferramenta de espera
  nesta sessão. Os tempos internos dessa repetição ficaram normais (soma
  dentro da faixa), exceto `fp32_s64_rep13`, levemente mais lento (setup 7,0 s
  contra 5,9 s de média). Pelo critério pré-fixado, nenhuma repetição foi
  descartada.

## 7. Limitações
- Uma carga só (mapa logístico, r = 3,7, x0 = 0,1) e uma máquina (Apple M5
  Pro, 15 núcleos, macOS), sem isolamento de CPU. Outras sessões rodavam na
  mesma máquina; o pareamento e a ordem aleatória mitigam o efeito, mas não o
  eliminam.
- *Por que* o fold não escala com as constraints é **hipótese**, não medida:
  quase todo o witness são bits (range checks), e a commitment de valores
  pequenos no Nova 0.73 é barata, então o custo seguiria o número de valores
  "largos" e a síntese, não a contagem de constraints. Não foi feito profiling.
- Os baselines encadeados refazem a checagem de entrada a cada passo (§3). O
  viés é a favor da PNF e não altera a conclusão.
- O tempo de prover por passo, com S fixo, ainda inclui um setup que não é
  amortizado nas métricas por passo. O setup foi medido à parte.
- S > 64 não foi medido. Com folds ainda maiores, o custo por fold deve
  começar a escalar com o tamanho, e a ordem entre as representações pode
  mudar.

## 8. Próximo passo
1. Profiling de um fold (síntese vs. MSM vs. NIFS vs. circuito secundário) e
   contagem de valores de witness "largos" vs. bits, por representação, para
   testar a hipótese do §7.
2. Estender para S ∈ {256, 1024} (com 2.048+ passos) e ver onde o fold passa
   a escalar com as constraints.
3. Reformular a alegação: a vantagem da PNF32 em Nova está na compressão,
   na verificação e na memória, não no folding. Para o prover, testar backends
   cujo custo é proporcional ao tamanho do circuito (Spartan/Plonkish direto,
   HyperNova/lookups).

## 9. Continuidade para outra IA
Os números estão em `results/pnf32-spf-20261016/summary.md` (e
`summary.json`). Para refazer, use `scripts/run_spf.sh` com um diretório novo:
ele roda os testes, aborta se falharem, e nunca sobrescreve. `SKIP_TESTS=1` só
serve para smoke (fica registrado no `manifest.txt`). Os baselines multi-passo
estão em `crates/spf-baselines` e envolvem o `nova-circuit` original por
path dependency. Nunca edite `../proof-numeric-zk`.
