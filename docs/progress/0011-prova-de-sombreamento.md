# 0011 — Protótipo G2: IEEE exato + sombreamento de HYG

**Data:** 2026-10-18 · **Estado:** concluído (protótipo); hipótese G2 sustentada

## 1. Objetivo e estado
Implementar e medir a "defesa além do horizonte": PNF32 bit-exata mais o
certificado de intervalos aninhados de Hammel, Yorke e Grebogi (1987),
checado localmente em cada fold Nova. Também verificar se o sombreamento sozinho
derrota o ataque (não derrota).

## 2. Arquivos
`crates/shadow-proof/{Cargo.toml, src/{lib,cert,circuit,bench,main}.rs, tests/circuit.rs}`,
`scripts/run_shadow_campaign.sh`, `scripts/summarize_shadow.py`,
`docs/09-prova-de-sombreamento.md`, `results/shadow-20261018/`, workspace `Cargo.toml` (membro novo).

## 3. Decisões
- Certificado de HYG (intervalos para trás), em vez do de Chow & Palmer: a checagem
  por passo é só aninhamento + distância, que cabe em comparações inteiras exatas.
- Grade 2^-48 (binary32) e 2^-96 (binary64) fora do circuito; no circuito, só binary32 e K=48.
- `r` exato do float (diádico); limiar 1/2 (ponto crítico).

## 4. Comandos
```bash
cargo test -p shadow-proof --release
bash scripts/run_shadow_campaign.sh results/shadow-20261018
```

## 5. Evidências
- 1.035 constraints/fold (424 PNF32 + 611 sombreamento).
- 201/201 sementes binary32 e 101/101 binary64 certificadas até 65.536 passos
  em r ∈ {3,7; 3,8; 3,9; 4,0}, com 1 breakdown isolado. ε ~10⁻⁵ (b32), ~10⁻¹² (b64).
- Os witnesses adversariais do ataque também são sombreados, o que confirma que o
  sombreamento sem L2 exato não impede a condução.
- Testes: aceitação honesta encadeada (300 folds, bit-exata ao f32), rejeição
  de adulterações (alargar A'/B', trocar o lado, ε=0), TestCS = CheckCS.
- Nova 16 folds: prova comprimida de 10.880 B, verificação de 23 ms.

## 6. Tentativas malsucedidas
- `debug_assert` da checagem não roda em release; a checagem passou a ser
  explícita no scan.
- r = 4 (inteiro) quebrava o `Map::from_f64` (expoente positivo); corrigido.
- binary64 com grade de 72 bits: alguns pontos perto de 0 (r=4) não eram
  representáveis; passou para 96 bits.
- TestConstraintSystem: nomes duplicados na alocação de `z`; corrigido com namespaces.

## 7. Limitações
Ver 09 §6 (1D, limiar 1/2, N fixo, soundness manual, Nova smoke).

## 8. Próximo passo
Enumeração exaustiva do circuito de sombreamento em grade reduzida; limiares
gerais; S passos por fold; estudo 2D (Hénon).

## 9. Continuidade para outra IA
Não afirmar "decisão exata em x0" depois do horizonte: a afirmação de G2 é
retroativa (x0' a ≤ ε). Sempre combinar com L2 exato; sem ele, o ataque continua.
