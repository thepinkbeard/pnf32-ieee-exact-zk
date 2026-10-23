# 0013 — Ponto fixo diádico fundido

**Data:** 2026-10-23 · **Estado:** concluído

## 1. Objetivo e estado
Testar se uma representação co-desenhada para R1CS (escala 2^-p, um
arredondamento por passo, decisão como bit) dá mais bits que a PNF32 e o WAD
com menos constraints, e conferir as decisões contra o h1. Resultado completo em
[11](../11-ponto-fixo-diadico-fundido.md).

## 2. Arquivos
- `crates/dyadic-fused/` (novo): `src/lib.rs` (perfil, referência, circuito),
  `src/main.rs` (`dyadic-bench constraints|decisions|nova`), `tests/dyadic.rs`.
- `Cargo.toml`: membro `crates/dyadic-fused`.
- `scripts/run_dyadic_fused.sh`, `scripts/compare_dyadic_h1.py` (novos).
- `docs/11-ponto-fixo-diadico-fundido.md`, `results/dyadic-fused-20261023/`.

## 3. Decisões
- s = 22 bits para `R`: `fl32(r)` com r ∈ {3,5; 3,7; 3,9; 4,0} é exato (testado).
  x0 das sementes (binary32 em [0,01; 0,99]) é exato para p ≥ 30. Então o perfil
  parte do **mesmo** modelo exato que a referência do h1.
- Arredondamento por piso (divisão euclidiana), como o WAD: é o que deixa o
  witness único com o menor custo.
- Bits de Y alocados com p+1 de largura. `Y ≤ 2^p` sai da unicidade, não de um
  range extra.
- Aridade 3 (contador, estado, decisão), igual à do WAD, para manter a comparação
  justa.

## 4. Comandos
```bash
bash scripts/run_dyadic_fused.sh results/dyadic-fused-20261023
```

## 5. Evidências
- Constraints/passo: p=32 → 91, p=64 → 155, p=96 → 219, p=110 → 247
  (= 2p+s+5). PNF32 415, WAD 431, PNF64 677.
- Testes (`tests.log`): 6 novos passam, incluindo a **enumeração exaustiva** de
  todos os witnesses `(Y, rem)` nos perfis mini (p,s) ∈ {(2,1), (3,2), (4,2)},
  com 673.088 witnesses no total para todo X e todo R ≤ 4·2^s: só o honesto é
  aceito. Adulteração rejeitada nos quatro perfis reais, TestCS concorda e
  trajetórias de 500 passos batem com a referência. Os 5 testes do WAD continuam
  passando.
- 3.219.216 decisões do circuito iguais a uma referência Python independente
  (0 diferenças).
- Primeiro passo divergente mediano contra a órbita exata, r=3,9: 43 / 87 / 133 /
  152 (previsão `p·ln2/λ`: 45 / 89 / 134 / 154).
- Nova (16 folds): 9.975 (p=64) e 10.067 (p=110) constraints primárias,
  verificação comprimida ~20 ms, prova de 10.656 B, estado final conferido.

## 6. Tentativas malsucedidas
- O teste de conversão tinha o significando de `fl32(3.9)` errado na constante
  esperada (16.356.148). O valor correto é 0xF9999A = 16.357.786. Era erro do
  teste, não do código.

## 7. Limitações
Ver 11 §5: só o passo logístico, p ≤ 114 num elemento, domínio limitado com
decisão absoluta, sem G2/L3 neste perfil, Nova smoke.

## 8. Próximo passo
G2 (certificado HYG) sobre o perfil diádico, para medir o custo do sombreamento
quando o estado já está numa grade binária fina, e comparar com os 1.035 da
PNF32 + G2.

## 9. Continuidade para outra IA
Alegação segura: no passo logístico com decisão por limiar, o diádico fundido
p=64 custa 155 constraints contra 431 do WAD e 415 da PNF32, com mais bits e
divergência mais tardia em relação à órbita exata. **Não** afirmar
compatibilidade com float nem generalizar para mapas com vários produtos sem
medir.
