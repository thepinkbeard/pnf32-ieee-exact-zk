# Diário de progresso

Ponto de retomada para pessoas e agentes automatizados. Segue o mesmo modelo de
9 seções do diário de `proof-numeric-zk`.

## Como continuar

1. Leia [`../00-pergunta-e-hipoteses.md`](../00-pergunta-e-hipoteses.md) e
   [`../06-resultados.md`](../06-resultados.md).
2. Leia o último registro abaixo.
3. Confirme o estado: `cargo test --workspace --release`.
4. Crie um registro novo e numerado; não altere registros concluídos.

## Registros

| ID | Data | Estado | Avanço |
|---:|---|---|---|
| [0001](0001-andaime-e-baselines.md) | 2026-10-16 | concluído | Pasta, workspace e baselines FP32/CBD/CFD reproduzidos |
| [0002](0002-referencia-e-oraculo.md) | 2026-10-16 | concluído | Referência genérica, oráculo exato; bug do crate `softfloat` em subnormais |
| [0003](0003-circuito-pnf32.md) | 2026-10-16 | concluído | Circuito PNF32, unicidade exaustiva, bug de soundness achado e corrigido |
| [0004](0004-medicoes.md) | 2026-10-16 | concluído, go | Constraints, Nova, trajetórias, varredura 2^32 |
| [0005](0005-steps-per-fold.md) | 2026-10-16 | concluído | `steps_per_fold`, baselines multi-passo, 30 rodadas pareadas: prover empata; compressão −39% vs FP32 |
| [0006](0006-busca-sistematica-rodada1.md) | 2026-10-16 | concluído, provisório | Literatura: ZKLP já faz IEEE barato; prova dupla L2+L3 aparece como brecha aberta |
| [0007](0007-busca-sistematica-rodada2.md) | 2026-10-16 | concluído | Rodada 2: ZIP lido, PANDA, snowballing; nenhum trabalho faz L2+L3; B2 vira formulação candidata |
| [0008](0008-garg2022-original.md) | 2026-10-16 | concluído | Garg 2022 no original: só erro por porta, admite pior arredondamento, sem decisão certificada; B2 confirmada |
| [0009](0009-prova-dupla-prototipo.md) | 2026-10-16 | concluído | Prova dupla L2+L3 (crate `dual-proof`): 415 + CBD + 6 constraints; robusto 100%/80%/≈0% em n=10/30/100; toda decisão IEEE errada sinalizada |
| [0010](0010-perfil-e-s1024.md) | 2026-10-17 | concluído | Perfil por fase; duas correções na PNF (LC crescente, modo só-witness); S até 1024: prover 2,1× mais rápido que FP32 e 1,4× que CBD-32 (revisa o 0005) |
| [0011](0011-prova-de-sombreamento.md) | 2026-10-18 | concluído (protótipo) | G2: PNF32 + sombreamento HYG, 1.035 constraints/fold, 100% das sementes certificadas até 65.536 passos; sombreamento sozinho não barra o ataque |

## Modelo obrigatório

1. objetivo e estado; 2. arquivos; 3. decisões e hipóteses; 4. comandos;
5. testes, métricas e evidências; 6. tentativas malsucedidas; 7. limitações;
8. próximo passo; 9. continuidade para outra IA.
