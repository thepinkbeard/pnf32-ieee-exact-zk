# 01 — Literatura e posicionamento

> **Superado em parte por [07-busca-sistematica.md](07-busca-sistematica.md).**
> Correções principais: os autores do Garg 2022 são Garg, Jain, Jin e Zhang. O
> ZKLP já faz IEEE completo em R1CS (gnark + LogUp) com 64 (mul) e 85 (add)
> constraints por operação, menos que o PNF32. A alegação "IEEE exato
> barato" não é nova.

> Estado: levantamento **inicial**, feito a partir das referências já citadas
> no paper e no repositório `proof-numeric-zk`. Itens marcados com **[verificar]**
> precisam de leitura direta da fonte antes de entrar num texto submetido.

## O que já existe

| trabalho | semântica | mecanismo | relevância para a PNF |
|---|---|---|---|
| Garg et al., *Succinct zero knowledge for floating point computations* (CCS 2022) | aproximada (erro relativo) e IEEE bit a bit | R1CS | baseline do "IEEE é caro" (~2.456 constraints por soma bit a bit); 89/108 entradas não nulas na versão aproximada. A PNF mostra que a versão **exata** pode custar ~145. **[verificar]** se o custo bit-exato deles inclui subnormais/especiais e qual métrica exata usam. |
| Ernstberger et al., ZKLP (2025) | IEEE completo (subnormais, ±0, inf, NaN, RNE) | lookups (LogUp), Plonkish | é o concorrente direto em fidelidade. A PNF não usa lookups e roda em R1CS/folding. ~64 constraints por mul amortizado com ≥ 2^15 operações, e a PNF não amortiza. **[verificar]** custo sem amortização, custo de add e backend exato. |
| Riasi et al., ZIP (2025) | FP64 em zkML | — | custo absoluto alto. A PNF é genérica em `(ebits, p)`, e binary64 é extensão direta. **[verificar]** |
| Sridhar et al. (USENIX Security 2026) | ponto fixo com quantização honesta + slack | Noir | já comparado no repo (registro 0031). Semântica diferente (não IEEE). |
| EZKL, zkPyTorch | ponto fixo / quantização | Halo2 | não bit-exatos com FP32 |
| `proof-numeric-zk`: CBD, CFD, CDAD | enclosure certificada / decisão trivalorada | Nova R1CS | mesmos backend e workload, então são baselines homogêneos. Respondem a outra pergunta (fidelidade à matemática exata com abstenção). |

## Técnicas emprestadas (não são contribuição)

- **Far/near path e sticky** em somadores de hardware: com `d >= p+2/p+3`, o
  operando menor só contribui com o sticky. Referência clássica: Muller et al.,
  *Handbook of Floating-Point Arithmetic*. **[verificar edição/capítulo]**
- **Underflow gradual ⇒ soma nunca arredonda para zero**: `x + y = 0 ⟺ x = -y`
  (Hauser 1996, TOPLAS; Kahan). Foi usado na constraint `zf·|X| = 0`.
  **[verificar citação]**
- **Hint-and-verify** (o provador fornece quociente e resto, e o circuito confere)
  é prática padrão em gadgets SNARK (por exemplo xJsnark, circomlib).
- **Range check fundido ao produto** (`a·b = Σ bits + offset`): truque padrão
  de R1CS.

## O que parece novo (a confirmar com busca sistemática)

1. **Codificação IEEE desempacotada como estado IVC** (T1+T2): validade como
   invariante indutivo do folding, checada uma vez em `z0`. Elimina a
   revalidação de entradas a cada fold, que no baseline do repo é parte
   relevante do custo.
2. **Quociente com corte fixo e shift testemunhado** (`P·2^k = Q·2^K + R`): um
   único mecanismo cobre normalização, desnormalização (subnormais) e
   underflow para zero, com unicidade dada pelo bit de topo de `Q`, sem
   contador de zeros à esquerda nem barrel shifter.
3. **Verificação exaustiva do espaço de witnesses** em formatos mini
   paramétricos: enumera *todas* as atribuições das variáveis de escolha, e
   não só testa o witness honesto. Achou um bug real de soundness que
   testes amostrados no binary32 não acharam (registro 0003).
4. Resultado quantitativo: **IEEE bit-exato geral mais barato que
   representações não-IEEE** no mesmo backend (415 contra 709 do CBD-32 e 1.003
   da CFD), invertendo a premissa de que fidelidade IEEE é proibitiva.

## Próxima ação de literatura

Busca sistemática (Google Scholar/IACR ePrint/DBLP) por: "floating point zkSNARK
R1CS", "IEEE 754 zero knowledge", "zkFloat", "verifiable floating point
folding". Para cada resultado, registrar: semântica, backend, lookup ou não,
custo por op e se cobre subnormais/especiais. Prioridade: confirmar os números
de ZKLP e Garg sem amortização para uma tabela homogênea de "constraints por
op exata".
