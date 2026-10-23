# 0014 — G2 sobre o perfil diádico fundido

**Data:** 2026-10-23 · **Estado:** concluído

## 1. Objetivo e estado
Combinar o passo diádico fundido (0013) com o certificado de sombreamento de
HYG (0011), medir o custo por fold e achar o menor p cujo ε fica abaixo do
ruído da entrada do oráculo. Resultado completo em [12](../12-g2-diadico.md).

## 2. Arquivos
- `crates/dyadic-fused/src/shadow.rs` (novo): `DyadicShadowCircuit`, `Widths`, mapa, órbita, certificado.
- `crates/dyadic-fused/src/main.rs`: `shadow-scan`, `shadow-constraints`, `shadow-nova`; `synth` genérico.
- `crates/dyadic-fused/tests/shadow.rs` (novo); `Cargo.toml` (dependência `shadow-proof`); `src/lib.rs` (`pub mod shadow`).
- `crates/shadow-proof/src/cert.rs`: `build_grid` e `check_grid` (pseudo-órbita já na grade). `build`/`check` delegam sem mudar o comportamento.
- `scripts/run_dyadic_g2.sh`, `scripts/summarize_dyadic_g2.py`, `docs/12-g2-diadico.md`, `results/dyadic-g2-20261023/`.

## 3. Decisões
- K = p+8 (com teto de 113, pelo limite do corpo `2K+bits(r)+2 ≤ 253`). A
  varredura exploratória (200 sementes, K ∈ {p+4, p+8, p+16, p+24}) certificou
  100% já com K = p+4.
- Duas larguras: `full`, igual à PNF + G2, para isolar a representação, e
  `tight`, com teto igual à folga honesta e a mesma soundness.
- A decisão de entrada `d` vai no estado (o verificador checa `d_0`), porque os
  bits do estado atual não existem no fold.
- p < 30 usa x0 arredondado para a grade. O erro retroativo total soma 2^-p.

## 4. Comandos
```bash
bash scripts/run_dyadic_g2.sh results/dyadic-g2-20261023
```

## 5. Evidências
- Testes: 6 do diádico + 4 do G2 diádico + 3 + 6 do `shadow-proof` passam
  (`tests.log`).
- Constraints por fold (`tight`): 309 / 343 / **393** / 508 / 617 / 844 / 902 para
  p = 20 / 24 / 32 / 48 / 64 / 96 / 110. PNF32 + G2 = 1.035. Com a mesma grade
  K=48 e p=24: 678 (`full`) e 471 (`tight`).
- 202.202 certificados emitidos, sem nenhum breakdown (7 perfis × 3 r × 1.001
  sementes × 9 horizontes, mais p=32 até N=65.536).
- Pior ε total: p=24 → 1,2·10⁻⁴; p=32 → 7,3·10⁻⁶ (103× abaixo de δx com
  ρ=0,05%).
- Nova (16 folds): estado e veredito finais conferidos, prova de 10.752 B,
  verificação ~30 ms.

## 6. Tentativas malsucedidas
- O teste de adulteração "A' − 1 deve ser rejeitado" falhou em p=32, K=40. Não
  era bug: com a grade mais grossa, a folga honesta pode passar de um degrau, e
  o intervalo alargado continua contido em f(I_n), ou seja, é um certificado
  válido. O teste passou a adulterar **além** da imagem exata de f e a aceitar
  a borda exata.

## 7. Limitações
Ver 12 §5: 1D, limiar no ponto crítico, larguras estreitas assumem certificado
justo, argumento de soundness do G2 manual, semântica retroativa, Nova smoke.

## 8. Próximo passo
Generalização para 2D (Hénon): perfil diádico com vários produtos por passo e
certificado com parallelogramos (HYG 1988). Em paralelo, aplicar as larguras
estreitas à PNF + G2 para a comparação ficar completa.

## 9. Continuidade para outra IA
Alegação segura: no mapa logístico com limiar 1/2, diádico p=32 + G2 estreito
custa 393 constraints por fold (contra 1.035 da PNF32 + G2), certifica 100% das
sementes até 65.536 passos, com ε ≤ 7,3·10⁻⁶. **Não** atribuir todo o ganho à
representação: com a mesma grade, ela responde por −34%, e as larguras por −31%.
